import { ChangeDetectionStrategy, Component, computed, inject, input, signal } from '@angular/core';
import { toObservable, toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { switchMap } from 'rxjs';
import { ATTACK_CLASSES, TimeRange, initialResource, rangeToMs } from '../../core/models/common.model';
import { EthNode, NodeMetricKey } from '../../core/models/node.model';
import { Prediction } from '../../core/models/prediction.model';
import { SecurityAlert } from '../../core/models/alert.model';
import { AlertService } from '../../core/services/alert.service';
import { ClockService } from '../../core/services/clock.service';
import { MetricsHistoryService } from '../../core/services/metrics-history.service';
import { NodeService } from '../../core/services/node.service';
import { PredictionService } from '../../core/services/prediction.service';
import { nodeColor } from '../../core/utils/colors';
import { fmtBytes, fmtMb, fmtPct, fmtPercentValue } from '../../core/utils/format';
import { ATTACK_COLOR, ATTACK_LABEL } from '../../core/utils/labels';
import { timeSeriesOption } from '../../shared/charts/chart-options';
import { EchartDirective } from '../../shared/charts/echart.directive';
import { ChartCardComponent } from '../../shared/components/chart-card/chart-card';
import { ConfidenceComponent, ProbBarComponent } from '../../shared/components/confidence/confidence';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { MetricCardComponent } from '../../shared/components/metric-card/metric-card';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { TimeRangeSelectorComponent } from '../../shared/components/time-range-selector/time-range-selector';
import { AgoPipe, DateTimePipe, PctPipe } from '../../shared/pipes/format.pipes';
import { BEACON_CARDS, GETH_CARDS, MetricDef, VALIDATOR_CARDS } from '../monitoring/metric-defs';

@Component({
  selector: 'app-node-detail-page',
  imports: [
    RouterLink,
    ChartCardComponent,
    EchartDirective,
    MetricCardComponent,
    StatusBadgeComponent,
    ConfidenceComponent,
    ProbBarComponent,
    DataTableComponent,
    CellDefDirective,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    TimeRangeSelectorComponent,
    AgoPipe,
    DateTimePipe,
    PctPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './node-detail.page.html',
  styleUrl: './node-detail.page.scss',
})
export class NodeDetailPage {
  /** Route param bound by withComponentInputBinding(). */
  readonly id = input.required<string>();

  private readonly nodeService = inject(NodeService);
  private readonly predictionService = inject(PredictionService);
  private readonly history = inject(MetricsHistoryService);
  protected readonly clock = inject(ClockService);

  private readonly id$ = toObservable(this.id);
  readonly res = toSignal(this.id$.pipe(switchMap((id) => this.nodeService.node$(id))), { initialValue: initialResource<EthNode>('live') });
  readonly predictions = toSignal(this.id$.pipe(switchMap((id) => this.predictionService.forNode$(id))), {
    initialValue: initialResource<Prediction[]>('live'),
  });
  private readonly allNodes = toSignal(this.nodeService.nodes$, { initialValue: initialResource<EthNode[]>('live') });
  private readonly alertsRes = toSignal(inject(AlertService).alerts$, { initialValue: initialResource<SecurityAlert[]>('live') });

  readonly range = signal<TimeRange>('15m');
  readonly node = computed(() => this.res().data);
  readonly attackLabel = ATTACK_LABEL;

  readonly heroTone = computed(() => {
    switch (this.node()?.status) {
      case 'healthy':
        return 'success';
      case 'critical':
        return 'critical';
      case 'warning':
        return 'warning';
      default:
        return 'offline';
    }
  });

  readonly memPct = computed(() => {
    const m = this.node()?.metrics;
    return m?.memory_used_mb != null && m.memory_total_mb ? (m.memory_used_mb / m.memory_total_mb) * 100 : null;
  });

  // ---------------------------------------------------------- metric cards
  private cards(defs: MetricDef[]) {
    const m = this.node()?.metrics ?? {};
    return defs.map((d) => {
      const v = m[d.key] as number | null | undefined;
      return { def: d, value: v === null || v === undefined ? '—' : d.format(v) };
    });
  }
  readonly gethCards = computed(() =>
    this.cards([
      ...GETH_CARDS,
      { key: 'rpc_latency_ms', title: 'RPC latency', unit: '', group: 'geth', format: (v) => `${v.toFixed(v < 10 ? 2 : 0)} ms`, agg: 'max', hint: 'JSON-RPC round trip' },
      { key: 'rpc_requests_per_sec', title: 'RPC request rate', unit: 'req/s', group: 'geth', format: (v) => v.toFixed(1), agg: 'sum', hint: 'Per second' },
    ]),
  );
  readonly beaconCards = computed(() => this.cards(BEACON_CARDS));
  readonly validatorCards = computed(() => this.cards(VALIDATOR_CARDS));

  // ---------------------------------------------------------- system charts
  private readonly samples = computed(() => {
    this.history.version();
    this.allNodes().updatedAt;
    return this.history.series(this.id(), rangeToMs(this.range()));
  });

  private systemChart(key: NodeMetricKey, label: string, color: string, format: (v: number) => string, yMax?: number) {
    const s = this.samples();
    const data = s.map((x) => [x.t, (x.metrics[key] as number | null | undefined) ?? null] as [number, number | null]);
    const rangeMs = rangeToMs(this.range());
    const end = this.history.windowEnd(rangeMs);
    return {
      empty: data.every(([, v]) => v === null),
      options: timeSeriesOption({ series: [{ name: label, color, data }], format, area: true, yMin: 0, yMax, xMin: end - rangeMs, xMax: end }),
    };
  }

  readonly cpuChart = computed(() => this.systemChart('cpu_usage_percent', 'CPU', nodeColor(this.id()), (v) => fmtPercentValue(v, 0), 100));
  readonly memChart = computed(() => this.systemChart('memory_used_mb', 'Memory', nodeColor(this.id()), fmtMb));

  readonly netChart = computed(() => {
    const s = this.samples();
    const rangeMs = rangeToMs(this.range());
    const end = this.history.windowEnd(rangeMs);
    const rx = s.map((x) => [x.t, x.metrics.network_rx_bytes_per_sec ?? null] as [number, number | null]);
    const tx = s.map((x) => [x.t, x.metrics.network_tx_bytes_per_sec ?? null] as [number, number | null]);
    return {
      empty: [...rx, ...tx].every(([, v]) => v === null),
      options: timeSeriesOption({
        series: [
          { name: 'RX', color: '#2E6FDB', data: rx },
          { name: 'TX', color: '#0B9A8D', data: tx },
        ],
        format: (v) => fmtBytes(v, true),
        yMin: 0,
        xMin: end - rangeMs,
        xMax: end,
      }),
    };
  });

  // ---------------------------------------------------------- predictions
  readonly predictionList = computed(() => this.predictions().data ?? []);

  readonly probabilityChart = computed(() => {
    const preds = [...this.predictionList()].reverse();
    const has = preds.some((p) => ATTACK_CLASSES.some((c) => p.probabilities[c] !== null));
    const series = ATTACK_CLASSES.map((c) => ({
      name: `P(${c})`,
      color: ATTACK_COLOR[c],
      data: preds.map((p) => [new Date(p.timestamp).getTime(), p.probabilities[c]] as [number, number | null]),
    }));
    if (!has) {
      series.splice(0, series.length, {
        name: 'Confidence',
        color: '#635BFF',
        data: preds.map((p) => [new Date(p.timestamp).getTime(), p.confidence] as [number, number | null]),
      });
    }
    return {
      empty: preds.length === 0,
      title: has ? 'Class probabilities over time' : 'Prediction confidence over time',
      options: timeSeriesOption({ series, format: (v) => fmtPct(v, 0), yMin: 0, yMax: 1, splitNumber: 2 }),
    };
  });

  readonly predictionColumns: TableColumn<Prediction>[] = [
    { key: 'timestamp', label: 'Timestamp', sortable: true, mono: true },
    { key: 'prediction', label: 'Prediction', sortable: true },
    { key: 'confidence', label: 'Confidence', sortable: true, sortValue: (p) => p.confidence },
    { key: 'probs', label: 'Probabilities', hideBelow: 'md' },
  ];
  readonly predKey = (p: Prediction) => p.id;

  readonly threatEvents = computed(() => this.predictionList().filter((p) => p.prediction === 'ECLIPSE' || p.prediction === 'RPC_FLOODING').slice(0, 8));
  readonly nodeAlerts = computed(() => (this.alertsRes().data ?? []).filter((a) => a.node_id === this.id()).slice(0, 8));
}
