import { ChangeDetectionStrategy, Component, computed, effect, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { Router, RouterLink } from '@angular/router';
import { OFFLINE_EVALUATION } from '../../core/config/model-info';
import { SecurityAlert } from '../../core/models/alert.model';
import { initialResource } from '../../core/models/common.model';
import { DashboardSummary, DistributionEntry, TimelinePoint } from '../../core/models/dashboard.model';
import { Incident, Verdict } from '../../core/models/incident.model';
import { EthNode } from '../../core/models/node.model';
import { AlertService } from '../../core/services/alert.service';
import { ClockService } from '../../core/services/clock.service';
import { DashboardService } from '../../core/services/dashboard.service';
import { IncidentService } from '../../core/services/incident.service';
import { NodeService } from '../../core/services/node.service';
import { PollingService } from '../../core/services/polling.service';
import { fmtNumber, fmtPct } from '../../core/utils/format';
import { ATTACK_COLOR, ATTACK_LABEL } from '../../core/utils/labels';
import { Severity } from '../../core/models/common.model';
import { donutOption, stackedTimeBarOption, timeSeriesOption } from '../../shared/charts/chart-options';
import { EchartDirective } from '../../shared/charts/echart.directive';
import { ChartCardComponent } from '../../shared/components/chart-card/chart-card';
import { ConfidenceComponent } from '../../shared/components/confidence/confidence';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { MetricCardComponent, MetricTrend } from '../../shared/components/metric-card/metric-card';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, MbPipe, NumPipe, PctPipe, Percent100Pipe } from '../../shared/pipes/format.pipes';

interface KpiSample {
  t: number;
  total: number | null;
  healthy: number | null;
  attacks: number | null;
  alerts: number | null;
}

@Component({
  selector: 'app-dashboard-page',
  imports: [
    RouterLink,
    MetricCardComponent,
    ChartCardComponent,
    EchartDirective,
    DataTableComponent,
    CellDefDirective,
    StatusBadgeComponent,
    ConfidenceComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgoPipe,
    MbPipe,
    NumPipe,
    PctPipe,
    Percent100Pipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './dashboard.page.html',
  styleUrl: './dashboard.page.scss',
})
export class DashboardPage {
  private readonly router = inject(Router);
  private readonly polling = inject(PollingService);
  protected readonly clock = inject(ClockService);
  private readonly dashboard = inject(DashboardService);

  readonly summary = toSignal(this.dashboard.summary$, { initialValue: initialResource<DashboardSummary>('live') });
  readonly distribution = toSignal(this.dashboard.distribution$, { initialValue: initialResource<DistributionEntry[]>('live') });
  readonly timeline = toSignal(this.dashboard.timeline$, { initialValue: initialResource<TimelinePoint[]>('live') });
  readonly nodes = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });
  readonly alerts = toSignal(inject(AlertService).alerts$, { initialValue: initialResource<SecurityAlert[]>('live') });
  readonly incidents = toSignal(inject(IncidentService).incidents$, { initialValue: initialResource<Incident[]>('live') });

  /** Session-only KPI history (the API has no KPI history endpoint) for sparklines and trends. */
  private readonly kpiHistory = signal<KpiSample[]>([]);
  private lastKpiAt: number | null = null;
  private lastKpiSource: string | null = null;

  constructor() {
    effect(() => {
      const s = this.summary();
      if (s.status !== 'success' || !s.data || s.updatedAt === this.lastKpiAt) return;
      this.lastKpiAt = s.updatedAt;
      const d = s.data;
      const sample: KpiSample = { t: s.updatedAt ?? Date.now(), total: d.total_nodes, healthy: d.healthy_nodes, attacks: d.active_attacks, alerts: d.total_alerts };
      if (s.source !== this.lastKpiSource) {
        this.lastKpiSource = s.source;
        this.kpiHistory.set([sample]);
      } else {
        this.kpiHistory.update((h) => [...h, sample].slice(-200));
      }
    });
  }

  private series(key: keyof Omit<KpiSample, 't'>): (number | null)[] {
    return this.kpiHistory()
      .slice(-60)
      .map((s) => s[key]);
  }

  private trend(key: keyof Omit<KpiSample, 't'>, goodWhen: 'up' | 'down'): MetricTrend | null {
    const h = this.kpiHistory();
    if (h.length < 2) return null;
    const last = h[h.length - 1];
    const target = last.t - 5 * 60_000;
    const ref = h.find((s) => s.t >= target) ?? h[0];
    if (last.t - ref.t < 30_000) return null;
    const a = ref[key];
    const b = last[key];
    if (a === null || b === null) return null;
    const delta = b - a;
    const mins = Math.max(1, Math.round((last.t - ref.t) / 60_000));
    return { delta, label: `${delta > 0 ? '+' : delta < 0 ? '−' : '±'}${Math.abs(delta)}`, goodWhen, reference: `Change vs ${mins} min ago (this session)` };
  }

  readonly kpis = computed(() => {
    const s = this.summary();
    const d = s.data;
    const loading = !d && s.status !== 'error';
    const liveAccuracy = d?.model_accuracy ?? null;
    return {
      loading,
      stale: s.stale,
      total: {
        value: fmtNumber(d?.total_nodes),
        trend: this.trend('total', 'up'),
        spark: this.series('total'),
      },
      healthy: {
        value: fmtNumber(d?.healthy_nodes),
        unit: d?.total_nodes ? `/ ${d.total_nodes}` : '',
        trend: this.trend('healthy', 'up'),
        spark: this.series('healthy'),
      },
      attacks: {
        value: fmtNumber(d?.active_attacks),
        trend: this.trend('attacks', 'down'),
        spark: this.series('attacks'),
      },
      alerts: {
        value: fmtNumber(d?.total_alerts),
        trend: this.trend('alerts', 'down'),
        spark: this.series('alerts'),
      },
      accuracy:
        liveAccuracy !== null
          ? { value: (liveAccuracy * 100).toFixed(2), badge: 'API', desc: 'Accuracy reported by the backend', title: 'Value returned by GET /dashboard/summary' }
          : {
              value: (OFFLINE_EVALUATION.accuracy * 100).toFixed(2),
              badge: 'Offline test',
              desc: 'Held-out test set evaluation — not live accuracy',
              title: 'Static value from the training evaluation (core/config/model-info.ts)',
            },
    };
  });

  // ----------------------------------------------------------- network table
  readonly nodeColumns: TableColumn<EthNode>[] = [
    { key: 'node_id', label: 'Node ID', sortable: true, mono: true },
    { key: 'status', label: 'Status', sortable: true, sortValue: (n) => ['critical', 'offline', 'warning', 'unknown', 'healthy'].indexOf(n.status) },
    { key: 'threat', label: 'Current threat', sortable: true, sortValue: (n) => n.latest_prediction?.prediction ?? null },
    { key: 'confidence', label: 'ML confidence', sortable: true, sortValue: (n) => n.latest_prediction?.confidence ?? null },
    { key: 'cpu', label: 'CPU', align: 'right', sortable: true, sortValue: (n) => n.metrics.cpu_usage_percent ?? null },
    { key: 'memory', label: 'Memory', align: 'right', sortable: true, sortValue: (n) => n.metrics.memory_used_mb ?? null, hideBelow: 'md' },
    { key: 'peers', label: 'Peers', align: 'right', sortable: true, mono: true, sortValue: (n) => n.metrics.peer_count ?? null },
    { key: 'updated', label: 'Last update', sortable: true, sortValue: (n) => n.last_update, hideBelow: 'md' },
  ];
  readonly nodeKey = (n: EthNode) => n.node_id;

  openNode(n: EthNode): void {
    this.router.navigate(['/nodes', n.node_id]);
  }

  // ------------------------------------------------------------- charts
  readonly distributionTotal = computed(() => (this.distribution().data ?? []).reduce((a, d) => a + d.count, 0));

  readonly distributionOptions = computed(() => {
    const data = this.distribution().data;
    if (!data) return null;
    return donutOption({
      data: data.map((d) => ({ name: d.label, value: d.count, color: ATTACK_COLOR[d.label] })),
      centerValue: fmtNumber(this.distributionTotal()),
      centerLabel: 'predictions',
      format: (v) => fmtNumber(v),
    });
  });

  readonly timelineBarOptions = computed(() => {
    const pts = this.timeline().data;
    if (!pts) return null;
    const ts = (p: TimelinePoint) => new Date(p.timestamp).getTime();
    return stackedTimeBarOption({
      series: [
        { name: 'NORMAL', color: ATTACK_COLOR.NORMAL, data: pts.map((p) => [ts(p), p.normal]) },
        { name: 'RPC_FLOODING', color: ATTACK_COLOR.RPC_FLOODING, data: pts.map((p) => [ts(p), p.rpc_flooding]) },
        { name: 'ECLIPSE', color: ATTACK_COLOR.ECLIPSE, data: pts.map((p) => [ts(p), p.eclipse]) },
      ],
      format: (v) => `${fmtNumber(v)} predictions`,
    });
  });

  readonly hasConfidence = computed(() => (this.timeline().data ?? []).some((p) => p.avg_confidence !== null));

  readonly timelineConfidenceOptions = computed(() => {
    const pts = this.timeline().data;
    if (!pts || !this.hasConfidence()) return null;
    return timeSeriesOption({
      series: [{ name: 'Mean confidence', color: '#635BFF', data: pts.map((p) => [new Date(p.timestamp).getTime(), p.avg_confidence]) }],
      format: (v) => fmtPct(v, 0),
      area: true,
      yMin: 0,
      yMax: 1,
      splitNumber: 2,
    });
  });

  readonly attackCountInTimeline = computed(() => (this.timeline().data ?? []).reduce((a, p) => a + p.eclipse + p.rpc_flooding, 0));

  // ------------------------------------------------------------- alerts
  readonly recentAlerts = computed(() => (this.alerts().data ?? []).slice(0, 7));
  readonly activeAlertCount = computed(() => (this.alerts().data ?? []).filter((a) => a.status === 'active').length);
  readonly attackLabel = ATTACK_LABEL;

  // ------------------------------------------------------------- incidents (ML → LangGraph)
  readonly recentIncidents = computed(() => (this.incidents().data ?? []).slice(0, 6));
  readonly ongoingInvestigations = computed(
    () => (this.incidents().data ?? []).filter((i) => i.investigation_status === 'NOT_STARTED' || i.investigation_status === 'RUNNING').length,
  );
  readonly confirmedIncidents = computed(() => (this.incidents().data ?? []).filter((i) => i.verdict === 'CONFIRMED').length);
  readonly verdictSeverity: Record<Verdict, Severity> = { CONFIRMED: 'critical', SUSPECTED: 'medium', REJECTED: 'healthy', INCONCLUSIVE: 'low' };

  refresh(): void {
    this.polling.refreshNow();
  }

  memPct(n: EthNode): number | null {
    const m = n.metrics;
    return m.memory_used_mb != null && m.memory_total_mb ? (m.memory_used_mb / m.memory_total_mb) * 100 : null;
  }
}
