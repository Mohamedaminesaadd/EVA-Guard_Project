import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { bucketPredictions } from '../../core/api/normalizers';
import { FEATURE_CATEGORIES, MODEL_INFO, OFFLINE_EVALUATION } from '../../core/config/model-info';
import { ATTACK_CLASSES, AttackClass, initialResource } from '../../core/models/common.model';
import { DashboardSummary } from '../../core/models/dashboard.model';
import { Prediction } from '../../core/models/prediction.model';
import { ClockService } from '../../core/services/clock.service';
import { ConnectionService } from '../../core/services/connection.service';
import { DashboardService } from '../../core/services/dashboard.service';
import { PredictionService } from '../../core/services/prediction.service';
import { fmtNumber, fmtPct } from '../../core/utils/format';
import { ATTACK_COLOR } from '../../core/utils/labels';
import { barOption, donutOption, heatmapOption, stackedTimeBarOption, timeSeriesOption } from '../../shared/charts/chart-options';
import { EchartDirective } from '../../shared/charts/echart.directive';
import { ChartCardComponent } from '../../shared/components/chart-card/chart-card';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { MetricCardComponent } from '../../shared/components/metric-card/metric-card';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe } from '../../shared/pipes/format.pipes';

interface NodeStats {
  node_id: string;
  total: number;
  NORMAL: number;
  ECLIPSE: number;
  RPC_FLOODING: number;
  attackRate: number;
  avgConfidence: number | null;
  last: string | null;
}

@Component({
  selector: 'app-ml-analytics-page',
  imports: [
    RouterLink,
    MetricCardComponent,
    ChartCardComponent,
    EchartDirective,
    DataTableComponent,
    CellDefDirective,
    StatusBadgeComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgoPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './ml-analytics.page.html',
  styleUrl: './ml-analytics.page.scss',
})
export class MlAnalyticsPage {
  protected readonly clock = inject(ClockService);
  protected readonly connection = inject(ConnectionService);
  readonly history = toSignal(inject(PredictionService).history$, { initialValue: initialResource<Prediction[]>('live') });
  readonly summary = toSignal(inject(DashboardService).summary$, { initialValue: initialResource<DashboardSummary>('live') });

  readonly model = MODEL_INFO;
  readonly offline = OFFLINE_EVALUATION;
  readonly features = FEATURE_CATEGORIES;

  readonly preds = computed(() => this.history().data ?? []);

  readonly modelStatus = computed(() => {
    const s = this.connection.status();
    const loaded = s.data?.model_loaded ?? null;
    if (s.status === 'error' && !s.data) return { label: 'Unknown', tone: 'offline', hint: 'GET /status unreachable' };
    if (loaded === true) return { label: 'Loaded', tone: 'success', hint: s.data?.model_version ? `version ${s.data.model_version}` : 'Reported by GET /status' };
    if (loaded === false) return { label: 'Not loaded', tone: 'critical', hint: 'Reported by GET /status' };
    return { label: 'Unknown', tone: 'offline', hint: '/status does not report model state' };
  });

  readonly lastPrediction = computed(() => this.summary().data?.last_prediction_at ?? this.preds()[0]?.timestamp ?? null);
  readonly totalPredictions = computed(() => {
    const t = this.summary().data?.total_predictions;
    return t !== null && t !== undefined
      ? { value: fmtNumber(t), hint: 'Reported by /dashboard/summary' }
      : { value: fmtNumber(this.preds().length), hint: 'Predictions loaded from /predictions' };
  });

  readonly offlineCards = [
    { label: 'Accuracy', value: (OFFLINE_EVALUATION.accuracy * 100).toFixed(2), icon: 'target' },
    { label: 'Macro precision', value: (OFFLINE_EVALUATION.macroPrecision * 100).toFixed(2), icon: 'crosshair' },
    { label: 'Macro recall', value: (OFFLINE_EVALUATION.macroRecall * 100).toFixed(2), icon: 'radar' },
    { label: 'Macro F1-score', value: (OFFLINE_EVALUATION.macroF1 * 100).toFixed(2), icon: 'scale' },
  ];

  readonly confusionOptions = computed(() =>
    OFFLINE_EVALUATION.confusionMatrix ? heatmapOption({ matrix: OFFLINE_EVALUATION.confusionMatrix, labels: [...MODEL_INFO.classes] }) : null,
  );

  // ------------------------------------------------------------ analytics
  readonly counts = computed(() => {
    const c: Record<AttackClass, number> = { NORMAL: 0, ECLIPSE: 0, RPC_FLOODING: 0 };
    for (const p of this.preds()) if (p.prediction !== 'UNKNOWN') c[p.prediction]++;
    return c;
  });

  readonly distributionOptions = computed(() =>
    donutOption({
      data: ATTACK_CLASSES.map((c) => ({ name: c, value: this.counts()[c], color: ATTACK_COLOR[c] })),
      centerValue: fmtNumber(this.preds().length),
      centerLabel: 'predictions',
      format: (v) => fmtNumber(v),
    }),
  );

  readonly confidenceOptions = computed(() => {
    const edges = [0, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0001];
    const labels = ['<50%', '50–60%', '60–70%', '70–80%', '80–90%', '90–95%', '95–100%'];
    const bin = (cls: AttackClass) =>
      labels.map((_, i) => this.preds().filter((p) => p.prediction === cls && p.confidence !== null && p.confidence >= edges[i] && p.confidence < edges[i + 1]).length);
    return barOption({
      categories: labels,
      series: ATTACK_CLASSES.map((c) => ({ name: c, color: ATTACK_COLOR[c], data: bin(c) })),
      format: (v) => fmtNumber(v),
      stacked: true,
    });
  });

  private readonly buckets = computed(() => {
    const p = this.preds();
    if (!p.length) return { pts: [], ms: 60_000 };
    const times = p.map((x) => new Date(x.timestamp).getTime());
    const span = Math.max(...times) - Math.min(...times);
    const ms = span > 6 * 3600_000 ? 15 * 60_000 : span > 3600_000 ? 5 * 60_000 : 60_000;
    return { pts: bucketPredictions(p, ms), ms };
  });

  readonly overTimeOptions = computed(() => {
    const t = (iso: string) => new Date(iso).getTime();
    const pts = this.buckets().pts;
    return stackedTimeBarOption({
      series: [
        { name: 'NORMAL', color: ATTACK_COLOR.NORMAL, data: pts.map((p) => [t(p.timestamp), p.normal]) },
        { name: 'RPC_FLOODING', color: ATTACK_COLOR.RPC_FLOODING, data: pts.map((p) => [t(p.timestamp), p.rpc_flooding]) },
        { name: 'ECLIPSE', color: ATTACK_COLOR.ECLIPSE, data: pts.map((p) => [t(p.timestamp), p.eclipse]) },
      ],
      format: (v) => `${fmtNumber(v)} predictions`,
    });
  });

  readonly bucketLabel = computed(() => `${this.buckets().ms / 60_000}-min buckets`);

  readonly trendOptions = computed(() => {
    const t = (iso: string) => new Date(iso).getTime();
    const pts = this.buckets().pts;
    const rate = (n: number, total: number) => (total ? n / total : null);
    return timeSeriesOption({
      series: [
        { name: 'ECLIPSE share', color: ATTACK_COLOR.ECLIPSE, data: pts.map((p) => [t(p.timestamp), rate(p.eclipse, p.normal + p.eclipse + p.rpc_flooding)]) },
        { name: 'RPC_FLOODING share', color: ATTACK_COLOR.RPC_FLOODING, data: pts.map((p) => [t(p.timestamp), rate(p.rpc_flooding, p.normal + p.eclipse + p.rpc_flooding)]) },
      ],
      format: (v) => fmtPct(v, 0),
      yMin: 0,
      yMax: 1,
      step: true,
    });
  });

  readonly nodeStats = computed<NodeStats[]>(() => {
    const map = new Map<string, Prediction[]>();
    for (const p of this.preds()) map.set(p.node_id, [...(map.get(p.node_id) ?? []), p]);
    return [...map.entries()].map(([node_id, list]) => {
      const c = { NORMAL: 0, ECLIPSE: 0, RPC_FLOODING: 0 };
      for (const p of list) if (p.prediction !== 'UNKNOWN') c[p.prediction]++;
      const confs = list.map((p) => p.confidence).filter((x): x is number => x !== null);
      return {
        node_id,
        total: list.length,
        ...c,
        attackRate: list.length ? (c.ECLIPSE + c.RPC_FLOODING) / list.length : 0,
        avgConfidence: confs.length ? confs.reduce((a, b) => a + b, 0) / confs.length : null,
        last: list.reduce<string | null>((m, p) => (!m || p.timestamp > m ? p.timestamp : m), null),
      };
    });
  });

  readonly statColumns: TableColumn<NodeStats>[] = [
    { key: 'node_id', label: 'Node', sortable: true, mono: true },
    { key: 'total', label: 'Predictions', sortable: true, align: 'right', mono: true },
    { key: 'NORMAL', label: 'NORMAL', sortable: true, align: 'right', mono: true },
    { key: 'ECLIPSE', label: 'ECLIPSE', sortable: true, align: 'right', mono: true },
    { key: 'RPC_FLOODING', label: 'RPC_FLOODING', sortable: true, align: 'right', mono: true },
    { key: 'attackRate', label: 'Attack rate', sortable: true, align: 'right', mono: true, value: (r) => fmtPct(r.attackRate) },
    { key: 'avgConfidence', label: 'Avg confidence', sortable: true, align: 'right', mono: true, value: (r) => fmtPct(r.avgConfidence), hideBelow: 'md' },
    { key: 'last', label: 'Last prediction', sortable: true, hideBelow: 'md' },
  ];
  readonly statKey = (r: NodeStats) => r.node_id;
}
