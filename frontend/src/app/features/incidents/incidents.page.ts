import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { Router, RouterLink } from '@angular/router';
import { initialResource } from '../../core/models/common.model';
import { Incident, Verdict } from '../../core/models/incident.model';
import { ClockService } from '../../core/services/clock.service';
import { IncidentService } from '../../core/services/incident.service';
import { VERDICT_LABEL } from '../../core/utils/labels';
import { ConfidenceComponent } from '../../shared/components/confidence/confidence';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe } from '../../shared/pipes/format.pipes';

type VerdictFilter = Verdict | 'all';

@Component({
  selector: 'app-incidents-page',
  imports: [RouterLink, DataTableComponent, CellDefDirective, StatusBadgeComponent, ConfidenceComponent, StateComponent, IconComponent, LiveIndicatorComponent, AgoPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <header class="page-header">
        <div>
          <h1>Attack Incidents</h1>
          <p class="page-header__sub">Incidents opened by the ML detector and investigated by the Hypothesis and Confirmation agents.</p>
        </div>
        <div class="page-header__actions">
          <app-live-indicator [res]="res()" />
        </div>
      </header>

      <div class="banner tone-info">
        <app-icon name="info" [size]="15" />
        <span>
          <strong>Suspected</strong> means only the ML model flagged the attack. An incident is <strong>Confirmed</strong> only when the Confirmation Agent
          returns that verdict.
        </span>
      </div>

      <div class="toolbar">
        <div class="input-icon search">
          <app-icon name="search" [size]="14" />
          <input class="input" type="search" placeholder="Filter by node or incident ID…" [value]="query()" (input)="query.set($any($event.target).value)" aria-label="Filter incidents" />
        </div>
        <span class="toolbar__divider"></span>
        @for (v of verdictFilters; track v) {
          <button type="button" class="chip" [class.is-active]="verdictFilter() === v" (click)="verdictFilter.set(v)" [attr.aria-pressed]="verdictFilter() === v">
            {{ v === 'all' ? 'All' : verdictLabel[v] }} <span class="chip__count">{{ counts()[v] ?? 0 }}</span>
          </button>
        }
      </div>

      <app-state
        [res]="res()"
        [empty]="filtered().length === 0"
        [emptyTitle]="incidents().length ? 'No incident matches the filters' : 'No incidents'"
        emptyMessage="Incidents are created only when the ML model predicts ECLIPSE or RPC_FLOODING with enough confidence."
        emptyIcon="shield-check"
      >
        <section class="card">
          <app-data-table [columns]="columns" [rows]="filtered()" [trackKey]="key" [clickable]="true" [pageSize]="25" [initialSort]="{ key: 'time', dir: 'desc' }" (rowClick)="open($event)">
            <ng-template appCell="incident_id" let-i>
              <a class="mono" [routerLink]="['/incidents', i.incident_id]" (click)="$event.stopPropagation()" [attr.title]="i.incident_id">{{ i.incident_id.slice(0, 8) }}</a>
            </ng-template>
            <ng-template appCell="node" let-i><a class="mono" [routerLink]="['/nodes', i.node_id]" (click)="$event.stopPropagation()">{{ i.node_id }}</a></ng-template>
            <ng-template appCell="attack" let-i><app-status-badge kind="attack" [value]="i.attack_type" /></ng-template>
            <ng-template appCell="confidence" let-i><app-confidence [value]="i.ml_confidence" [label]="i.attack_type" /></ng-template>
            <ng-template appCell="time" let-i><span class="mono subtle" [attr.title]="i.time">{{ i.time | ago: clock.now() }}</span></ng-template>
            <ng-template appCell="investigation" let-i><app-status-badge kind="investigation" [value]="i.investigation_status" /></ng-template>
            <ng-template appCell="verdict" let-i>
              <app-status-badge kind="verdict" [value]="i.displayVerdict" [title]="i.verdict ? 'Verdict of the Confirmation Agent' : 'No agent verdict yet: detected by ML only'" />
            </ng-template>
          </app-data-table>
        </section>
      </app-state>
    </div>
  `,
  styles: `
    .search { min-width: 260px; }
  `,
})
export class IncidentsPage {
  private readonly router = inject(Router);
  protected readonly clock = inject(ClockService);
  readonly res = toSignal(inject(IncidentService).incidents$, { initialValue: initialResource<Incident[]>('live') });

  readonly query = signal('');
  readonly verdictFilter = signal<VerdictFilter>('all');
  readonly verdictFilters: VerdictFilter[] = ['all', 'SUSPECTED', 'CONFIRMED', 'REJECTED', 'INCONCLUSIVE'];
  readonly verdictLabel = VERDICT_LABEL;

  readonly incidents = computed(() => this.res().data ?? []);

  private readonly searched = computed(() => {
    const q = this.query().trim().toLowerCase();
    return this.incidents().filter((i) => !q || `${i.incident_id} ${i.node_id} ${i.attack_type}`.toLowerCase().includes(q));
  });

  readonly counts = computed(() => {
    const c: Record<string, number> = { all: 0 };
    for (const i of this.searched()) {
      c['all']++;
      c[i.displayVerdict] = (c[i.displayVerdict] ?? 0) + 1;
    }
    return c;
  });

  readonly filtered = computed(() => {
    const v = this.verdictFilter();
    return this.searched().filter((i) => v === 'all' || i.displayVerdict === v);
  });

  readonly columns: TableColumn<Incident>[] = [
    { key: 'incident_id', label: 'Incident', sortable: true, mono: true },
    { key: 'node', label: 'Node', sortable: true, mono: true, sortValue: (i) => i.node_id },
    { key: 'attack', label: 'Attack type', sortable: true, sortValue: (i) => i.attack_type },
    { key: 'confidence', label: 'ML confidence', sortable: true, sortValue: (i) => i.ml_confidence },
    { key: 'time', label: 'Detected', sortable: true, sortValue: (i) => i.time },
    { key: 'investigation', label: 'Investigation', sortable: true, sortValue: (i) => i.investigation_status, hideBelow: 'md' },
    { key: 'verdict', label: 'Verdict', sortable: true, sortValue: (i) => i.displayVerdict },
  ];
  readonly key = (i: Incident) => i.incident_id;

  open(i: Incident): void {
    this.router.navigate(['/incidents', i.incident_id]);
  }
}
