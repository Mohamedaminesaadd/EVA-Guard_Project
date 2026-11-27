import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { bucketPredictions, severityForAttack } from '../../core/api/normalizers';
import { ATTACK_CLASSES, AttackClass, PredictionLabel, Severity, initialResource } from '../../core/models/common.model';
import { Prediction } from '../../core/models/prediction.model';
import { ClockService } from '../../core/services/clock.service';
import {
  INVESTIGATION_LABEL,
  INVESTIGATION_TONE,
  InvestigationStatus,
  InvestigationStatusService,
  shortEventId,
} from '../../core/services/investigation-status.service';
import { PredictionService } from '../../core/services/prediction.service';
import { fmtNumber, fmtPct } from '../../core/utils/format';
import { ATTACK_COLOR, ATTACK_LABEL, SEVERITY_RANK } from '../../core/utils/labels';
import { barOption, stackedTimeBarOption } from '../../shared/charts/chart-options';
import { EchartDirective } from '../../shared/charts/echart.directive';
import { ChartCardComponent } from '../../shared/components/chart-card/chart-card';
import { ConfidenceComponent, ProbBarComponent } from '../../shared/components/confidence/confidence';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { DrawerComponent } from '../../shared/components/drawer/drawer';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { MetricCardComponent } from '../../shared/components/metric-card/metric-card';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, DateTimePipe, PctPipe } from '../../shared/pipes/format.pipes';

export interface ThreatEvent {
  id: string;
  eventId: string;
  prediction: Prediction;
  severity: Severity;
}

@Component({
  selector: 'app-threats-page',
  imports: [
    RouterLink,
    MetricCardComponent,
    ChartCardComponent,
    EchartDirective,
    DataTableComponent,
    CellDefDirective,
    StatusBadgeComponent,
    ConfidenceComponent,
    ProbBarComponent,
    DrawerComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgoPipe,
    DateTimePipe,
    PctPipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './threats.page.html',
  styleUrl: './threats.page.scss',
})
export class ThreatsPage {
  protected readonly clock = inject(ClockService);
  protected readonly investigation = inject(InvestigationStatusService);
  readonly history = toSignal(inject(PredictionService).history$, { initialValue: initialResource<Prediction[]>('live') });

  readonly nodeFilter = signal<string>('all');
  readonly classFilter = signal<AttackClass | 'attacks' | 'all'>('attacks');
  readonly search = signal('');
  readonly selectedId = signal<string | null>(null);
  readonly drawerOpen = signal(false);

  readonly attackLabel = ATTACK_LABEL;
  readonly attackColor = ATTACK_COLOR;
  readonly classes = ATTACK_CLASSES;
  readonly classChips: (AttackClass | 'attacks' | 'all')[] = ['attacks', 'ECLIPSE', 'RPC_FLOODING', 'NORMAL', 'all'];
  readonly investigationLabel = INVESTIGATION_LABEL;
  readonly investigationTone = INVESTIGATION_TONE;
  readonly investigationOptions = Object.keys(INVESTIGATION_LABEL) as InvestigationStatus[];

  readonly events = computed<ThreatEvent[]>(() =>
    (this.history().data ?? []).map((p) => ({ id: p.id, eventId: shortEventId(p.id), prediction: p, severity: severityForAttack(p.prediction) })),
  );

  readonly nodeIds = computed(() => [...new Set(this.events().map((e) => e.prediction.node_id))].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })));

  /** Events matching node + search (class filter applied separately so counts per class stay visible). */
  private readonly baseFiltered = computed(() => {
    const node = this.nodeFilter();
    const q = this.search().trim().toLowerCase();
    return this.events().filter(
      (e) =>
        (node === 'all' || e.prediction.node_id === node) &&
        (!q || `${e.eventId} ${e.prediction.node_id} ${e.prediction.prediction} ${e.id}`.toLowerCase().includes(q)),
    );
  });

  readonly filtered = computed(() => {
    const cls = this.classFilter();
    return this.baseFiltered().filter((e) =>
      cls === 'all' ? true : cls === 'attacks' ? e.prediction.prediction !== 'NORMAL' : e.prediction.prediction === cls,
    );
  });

  readonly classCounts = computed(() => {
    const counts: Record<string, number> = { all: 0, attacks: 0, NORMAL: 0, ECLIPSE: 0, RPC_FLOODING: 0 };
    for (const e of this.baseFiltered()) {
      counts['all']++;
      counts[e.prediction.prediction] = (counts[e.prediction.prediction] ?? 0) + 1;
      if (e.prediction.prediction !== 'NORMAL') counts['attacks']++;
    }
    return counts;
  });

  readonly summary = computed(() => {
    const attacks = this.baseFiltered().filter((e) => e.prediction.prediction === 'ECLIPSE' || e.prediction.prediction === 'RPC_FLOODING');
    const confs = attacks.map((e) => e.prediction.confidence).filter((c): c is number => c !== null);
    const times = this.baseFiltered().map((e) => new Date(e.prediction.timestamp).getTime());
    const span = times.length ? (Math.max(...times) - Math.min(...times)) / 60_000 : 0;
    return {
      total: fmtNumber(attacks.length),
      eclipse: fmtNumber(attacks.filter((e) => e.prediction.prediction === 'ECLIPSE').length),
      rpc: fmtNumber(attacks.filter((e) => e.prediction.prediction === 'RPC_FLOODING').length),
      avgConf: confs.length ? fmtPct(confs.reduce((a, b) => a + b, 0) / confs.length) : '—',
      nodes: fmtNumber(new Set(attacks.map((e) => e.prediction.node_id)).size),
      window: span > 0 ? `over the last ${span >= 120 ? (span / 60).toFixed(1) + ' h' : Math.round(span) + ' min'} of history` : 'in the loaded history',
      analyzed: fmtNumber(this.baseFiltered().length),
    };
  });

  readonly perNodeOptions = computed(() => {
    const ids = this.nodeIds().filter((id) => this.nodeFilter() === 'all' || id === this.nodeFilter());
    const evs = this.baseFiltered();
    const count = (id: string, cls: PredictionLabel) => evs.filter((e) => e.prediction.node_id === id && e.prediction.prediction === cls).length;
    return barOption({
      categories: ids,
      series: [
        { name: 'ECLIPSE', color: ATTACK_COLOR.ECLIPSE, data: ids.map((id) => count(id, 'ECLIPSE')) },
        { name: 'RPC_FLOODING', color: ATTACK_COLOR.RPC_FLOODING, data: ids.map((id) => count(id, 'RPC_FLOODING')) },
      ],
      format: (v) => fmtNumber(v),
      horizontal: true,
      stacked: true,
    });
  });

  readonly hasAttacks = computed(() => this.baseFiltered().some((e) => e.prediction.prediction !== 'NORMAL' && e.prediction.prediction !== 'UNKNOWN'));

  readonly timelineOptions = computed(() => {
    const preds = this.baseFiltered().map((e) => e.prediction);
    const pts = bucketPredictions(preds, 60_000);
    const t = (iso: string) => new Date(iso).getTime();
    return stackedTimeBarOption({
      series: [
        { name: 'RPC_FLOODING', color: ATTACK_COLOR.RPC_FLOODING, data: pts.map((p) => [t(p.timestamp), p.rpc_flooding]) },
        { name: 'ECLIPSE', color: ATTACK_COLOR.ECLIPSE, data: pts.map((p) => [t(p.timestamp), p.eclipse]) },
      ],
      format: (v) => `${fmtNumber(v)} detections`,
    });
  });

  readonly columns: TableColumn<ThreatEvent>[] = [
    { key: 'eventId', label: 'Event ID', sortable: true, mono: true },
    { key: 'node', label: 'Node', sortable: true, mono: true, sortValue: (e) => e.prediction.node_id },
    { key: 'attack', label: 'Predicted attack', sortable: true, sortValue: (e) => e.prediction.prediction },
    { key: 'confidence', label: 'Confidence', sortable: true, sortValue: (e) => e.prediction.confidence },
    { key: 'timestamp', label: 'Timestamp', sortable: true, mono: true, sortValue: (e) => e.prediction.timestamp },
    { key: 'severity', label: 'Severity', sortable: true, sortValue: (e) => SEVERITY_RANK[e.severity] },
    { key: 'status', label: 'Investigation', sortable: true, sortValue: (e) => this.investigation.get(e.id), hideBelow: 'md' },
  ];
  readonly eventKey = (e: ThreatEvent) => e.id;

  readonly selected = computed(() => this.events().find((e) => e.id === this.selectedId()) ?? null);

  readonly probOptions = computed(() => {
    const e = this.selected();
    if (!e) return null;
    const p = e.prediction.probabilities;
    return barOption({
      categories: [...ATTACK_CLASSES],
      series: [{ name: 'Probability', color: '#635BFF', data: ATTACK_CLASSES.map((c) => p[c]) }],
      format: (v) => fmtPct(v, 0),
      horizontal: true,
    });
  });

  readonly hasProbabilities = computed(() => {
    const e = this.selected();
    return !!e && ATTACK_CLASSES.some((c) => e.prediction.probabilities[c] !== null);
  });

  open(e: ThreatEvent): void {
    this.selectedId.set(e.id);
    this.drawerOpen.set(true);
  }

  setStatus(id: string, v: string): void {
    this.investigation.set(id, v as InvestigationStatus);
  }

  classChipLabel(c: string): string {
    return c === 'attacks' ? 'All attacks' : c === 'all' ? 'Everything' : c;
  }
}
