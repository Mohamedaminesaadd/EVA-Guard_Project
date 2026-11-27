import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { TimeRange, initialResource, rangeToMs } from '../../core/models/common.model';
import { EthNode, MetricSample } from '../../core/models/node.model';
import { ClockService } from '../../core/services/clock.service';
import { MetricsHistoryService } from '../../core/services/metrics-history.service';
import { NodeService } from '../../core/services/node.service';
import { PollingService } from '../../core/services/polling.service';
import { SettingsService } from '../../core/services/settings.service';
import { nodeColor, nodeDash } from '../../core/utils/colors';
import { fmtDateTime } from '../../core/utils/format';
import { timeSeriesOption } from '../../shared/charts/chart-options';
import { EchartDirective } from '../../shared/charts/echart.directive';
import { ChartCardComponent } from '../../shared/components/chart-card/chart-card';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { MetricCardComponent } from '../../shared/components/metric-card/metric-card';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { TimeRangeSelectorComponent } from '../../shared/components/time-range-selector/time-range-selector';
import { MbPipe, MsPipe, NumPipe, Percent100Pipe } from '../../shared/pipes/format.pipes';
import {
  AGG_LABEL,
  BEACON_CARDS,
  BEACON_CHARTS,
  GETH_CARDS,
  GETH_CHARTS,
  INFRA_METRICS,
  METRIC_GROUPS,
  MetricDef,
  MetricGroup,
  VALIDATOR_CARDS,
  VALIDATOR_CHARTS,
  aggregate,
} from './metric-defs';

interface ChartView {
  def: MetricDef;
  options: ReturnType<typeof timeSeriesOption>;
  empty: boolean;
}

interface CardView {
  def: MetricDef;
  value: string;
  badge: string;
}

const MAX_POINTS = 600;

function sameIds(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

@Component({
  selector: 'app-monitoring-page',
  imports: [
    RouterLink,
    ChartCardComponent,
    EchartDirective,
    MetricCardComponent,
    StatusBadgeComponent,
    TimeRangeSelectorComponent,
    LiveIndicatorComponent,
    StateComponent,
    IconComponent,
    Percent100Pipe,
    MbPipe,
    MsPipe,
    NumPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './monitoring.page.html',
  styleUrl: './monitoring.page.scss',
})
export class MonitoringPage {
  private readonly history = inject(MetricsHistoryService);
  private readonly polling = inject(PollingService);
  protected readonly settings = inject(SettingsService);
  protected readonly clock = inject(ClockService);

  readonly nodes = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });

  readonly range = signal<TimeRange>('15m');
  readonly group = signal<MetricGroup | 'all'>('all');
  /** Empty set = all nodes. */
  readonly selected = signal<Set<string>>(new Set());
  readonly paused = signal(false);
  private readonly frozenVersion = signal(0);

  readonly groups = METRIC_GROUPS;
  readonly aggLabel = AGG_LABEL;

  readonly allNodeIds = computed(() => (this.nodes().data ?? []).map((n) => n.node_id), { equal: sameIds });
  readonly activeIds = computed(
    () => {
      const sel = this.selected();
      const all = this.allNodeIds();
      return sel.size ? all.filter((id) => sel.has(id)) : all;
    },
    { equal: sameIds },
  );
  readonly activeNodes = computed(() => (this.nodes().data ?? []).filter((n) => this.activeIds().includes(n.node_id)));

  readonly nodeColor = nodeColor;

  toggleNode(id: string): void {
    const next = new Set(this.selected());
    if (next.has(id)) next.delete(id);
    else next.add(id);
    if (next.size === this.allNodeIds().length) next.clear();
    this.selected.set(next);
  }

  focusNode(id: string): void {
    this.selected.set(new Set([id]));
  }

  clearNodes(): void {
    this.selected.set(new Set());
  }

  isSelected(id: string): boolean {
    const s = this.selected();
    return s.size === 0 || s.has(id);
  }

  togglePause(): void {
    if (!this.paused()) this.frozenVersion.set(this.history.version());
    this.paused.set(!this.paused());
  }

  refresh(): void {
    this.polling.refreshNow();
  }

  show(g: MetricGroup): boolean {
    return this.group() === 'all' || this.group() === g;
  }

  // ------------------------------------------------------------ time series
  private readonly samples = computed(() => {
    // Re-read the buffer on every history write (or freeze it while paused).
    if (this.paused()) {
      this.frozenVersion();
    } else {
      this.history.version();
      this.nodes().updatedAt;
    }
    const rangeMs = rangeToMs(this.range());
    const out = new Map<string, MetricSample[]>();
    for (const id of this.activeIds()) {
      const s = this.history.series(id, rangeMs);
      const step = Math.ceil(s.length / MAX_POINTS);
      out.set(id, step > 1 ? s.filter((_, i) => i % step === 0 || i === s.length - 1) : s);
    }
    return out;
  });

  readonly window = computed(() => {
    this.samples();
    const rangeMs = rangeToMs(this.range());
    const end = this.history.windowEnd(rangeMs);
    return { start: end - rangeMs, end };
  });

  readonly coverage = computed(() => {
    this.samples();
    if (this.settings.isDemo()) return null;
    const oldest = this.history.oldestSampleTime();
    const rangeMs = rangeToMs(this.range());
    const lagging = this.history.isLagging(rangeMs);
    if (oldest === null) return { text: 'Waiting for the first GET /nodes response to start the live buffer.', lagging: false };
    const partial = oldest > this.window().start;
    return {
      lagging,
      partial,
      text: `Live buffer since ${fmtDateTime(oldest)} (browser session, max ≈2 h).`,
    };
  });

  private chart(def: MetricDef): ChartView {
    const samples = this.samples();
    const series = this.activeIds().map((id) => ({
      name: id,
      color: nodeColor(id),
      dashed: nodeDash(id) === 'dashed',
      data: (samples.get(id) ?? []).map((s) => [s.t, (s.metrics[def.key] as number | null | undefined) ?? null] as [number, number | null]),
    }));
    const empty = series.every((s) => s.data.every(([, v]) => v === null));
    const w = this.window();
    return {
      def,
      empty,
      options: timeSeriesOption({
        series,
        format: def.format,
        yMin: def.yMin,
        yMax: def.yMax,
        step: def.step,
        xMin: w.start,
        xMax: w.end,
      }),
    };
  }

  readonly infraCharts = computed(() => INFRA_METRICS.filter((d) => this.show(d.group)).map((d) => this.chart(d)));
  readonly gethCharts = computed(() => GETH_CHARTS.map((d) => this.chart(d)));
  readonly beaconCharts = computed(() => BEACON_CHARTS.map((d) => this.chart(d)));
  readonly validatorCharts = computed(() => VALIDATOR_CHARTS.map((d) => this.chart(d)));

  // ------------------------------------------------------------ metric cards
  private cards(defs: MetricDef[]): CardView[] {
    const nodes = this.activeNodes();
    const single = nodes.length === 1;
    return defs.map((def) => {
      const v = aggregate(
        nodes.map((n) => n.metrics[def.key] as number | null | undefined),
        def.agg,
      );
      return {
        def,
        value: v === null ? '—' : def.format(v),
        badge: single ? nodes[0].node_id : `${AGG_LABEL[def.agg]} · ${nodes.length}`,
      };
    });
  }

  readonly gethCards = computed(() => this.cards(GETH_CARDS));
  readonly beaconCards = computed(() => this.cards(BEACON_CARDS));
  readonly validatorCards = computed(() => this.cards(VALIDATOR_CARDS));
}
