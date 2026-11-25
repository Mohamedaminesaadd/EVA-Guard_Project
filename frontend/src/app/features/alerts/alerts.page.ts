import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { AlertStatus, SecurityAlert } from '../../core/models/alert.model';
import { Severity, initialResource } from '../../core/models/common.model';
import { AlertService } from '../../core/services/alert.service';
import { ClockService } from '../../core/services/clock.service';
import { ATTACK_ICON, ATTACK_LABEL, SEVERITY_LABEL, SEVERITY_RANK, SEVERITY_TONE } from '../../core/utils/labels';
import { DrawerComponent } from '../../shared/components/drawer/drawer';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { ConfidenceComponent } from '../../shared/components/confidence/confidence';
import { AgoPipe, DateTimePipe } from '../../shared/pipes/format.pipes';

type Tab = AlertStatus | 'all';
type SortMode = 'newest' | 'severity';

@Component({
  selector: 'app-alerts-page',
  imports: [
    RouterLink,
    StatusBadgeComponent,
    ConfidenceComponent,
    DrawerComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgoPipe,
    DateTimePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './alerts.page.html',
  styleUrl: './alerts.page.scss',
})
export class AlertsPage {
  protected readonly service = inject(AlertService);
  protected readonly clock = inject(ClockService);
  readonly res = toSignal(this.service.alerts$, { initialValue: initialResource<SecurityAlert[]>('live') });

  readonly tab = signal<Tab>('active');
  readonly severities = signal<Set<Severity>>(new Set());
  readonly attackFilter = signal<string>('all');
  readonly nodeQuery = signal('');
  readonly sort = signal<SortMode>('newest');
  readonly selectedId = signal<string | null>(null);
  readonly drawerOpen = signal(false);

  readonly attackLabel = ATTACK_LABEL;
  readonly attackIcon = ATTACK_ICON;
  readonly severityLabel = SEVERITY_LABEL;
  readonly severityTone = SEVERITY_TONE;
  readonly severityOptions: Severity[] = ['critical', 'high', 'medium', 'low'];
  readonly tabs: { value: Tab; label: string }[] = [
    { value: 'active', label: 'Active' },
    { value: 'acknowledged', label: 'Acknowledged' },
    { value: 'resolved', label: 'Resolved' },
    { value: 'all', label: 'All' },
  ];

  readonly all = computed(() => this.res().data ?? []);

  /** Alerts matching every filter except the status tab (tab counts stay meaningful). */
  private readonly matching = computed(() => {
    const sev = this.severities();
    const atk = this.attackFilter();
    const q = this.nodeQuery().trim().toLowerCase();
    return this.all().filter(
      (a) =>
        (sev.size === 0 || sev.has(a.severity)) &&
        (atk === 'all' || a.attack_type === atk) &&
        (!q || a.node_id.toLowerCase().includes(q) || a.id.toLowerCase().includes(q)),
    );
  });

  readonly counts = computed(() => {
    const m = this.matching();
    return {
      active: m.filter((a) => a.status === 'active').length,
      acknowledged: m.filter((a) => a.status === 'acknowledged').length,
      resolved: m.filter((a) => a.status === 'resolved').length,
      all: m.length,
    } as Record<Tab, number>;
  });

  readonly severityCounts = computed(() => {
    const out: Record<string, number> = {};
    for (const a of this.all().filter((x) => x.status !== 'resolved')) out[a.severity] = (out[a.severity] ?? 0) + 1;
    return out;
  });

  readonly visible = computed(() => {
    const tab = this.tab();
    const list = this.matching().filter((a) => tab === 'all' || a.status === tab);
    return this.sort() === 'severity'
      ? [...list].sort((a, b) => SEVERITY_RANK[b.severity] - SEVERITY_RANK[a.severity] || b.timestamp.localeCompare(a.timestamp))
      : list;
  });

  readonly attackTypes = computed(() => [...new Set(this.all().map((a) => a.attack_type))].sort());
  readonly localCount = computed(() => this.all().filter((a) => a.locallyModified).length);
  readonly selected = computed(() => this.all().find((a) => a.id === this.selectedId()) ?? null);

  toggleSeverity(s: Severity): void {
    const next = new Set(this.severities());
    if (next.has(s)) next.delete(s);
    else next.add(s);
    this.severities.set(next);
  }

  open(a: SecurityAlert): void {
    this.selectedId.set(a.id);
    this.drawerOpen.set(true);
  }

  acknowledge(a: SecurityAlert, ev?: Event): void {
    ev?.stopPropagation();
    this.service.acknowledge(a.id);
  }

  resolve(a: SecurityAlert, ev?: Event): void {
    ev?.stopPropagation();
    this.service.resolve(a.id);
  }

  reopen(a: SecurityAlert, ev?: Event): void {
    ev?.stopPropagation();
    this.service.reopen(a.id);
  }

  title(a: SecurityAlert): string {
    return a.attack_type === 'NORMAL' ? 'Anomaly cleared' : `${ATTACK_LABEL[a.attack_type]} detected`;
  }
}
