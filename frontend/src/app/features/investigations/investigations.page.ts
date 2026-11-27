import { ChangeDetectionStrategy, Component, computed, inject } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { Router, RouterLink } from '@angular/router';
import { initialResource } from '../../core/models/common.model';
import { Incident, InvestigationReport } from '../../core/models/incident.model';
import { ClockService } from '../../core/services/clock.service';
import { ConnectionService } from '../../core/services/connection.service';
import { IncidentService } from '../../core/services/incident.service';
import { isObj } from '../../core/api/normalizers';
import { CellDefDirective, DataTableComponent, TableColumn } from '../../shared/components/data-table/data-table';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, PctPipe } from '../../shared/pipes/format.pipes';

@Component({
  selector: 'app-investigations-page',
  imports: [RouterLink, DataTableComponent, CellDefDirective, StatusBadgeComponent, StateComponent, IconComponent, LiveIndicatorComponent, AgoPipe, PctPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <header class="page-header">
        <div>
          <h1>AI Investigations</h1>
          <p class="page-header__sub">LangGraph reports: Hypothesis Agent → Confirmation Agent, grounded in the RAG knowledge base.</p>
        </div>
        <div class="page-header__actions">
          <app-live-indicator [res]="res()" />
        </div>
      </header>

      @if (orchestratorDown()) {
        <div class="banner tone-warning">
          <app-icon name="triangle-exclamation-point" [size]="15" />
          <span>
            <strong>LangGraph orchestrator not loaded.</strong> The backend reports <code>orchestrator_loaded: false</code>: ML detection keeps running, but
            new incidents are not investigated. Check the backend logs.
          </span>
        </div>
      }

      @if (ongoing().length) {
        <div class="banner tone-info">
          <app-icon name="hourglass" [size]="15" />
          <span>
            {{ ongoing().length }} investigation(s) queued or running:
            @for (i of ongoing(); track i.incident_id; let last = $last) {
              <a class="mono" [routerLink]="['/incidents', i.incident_id]">{{ i.incident_id.slice(0, 8) }}</a>{{ last ? '' : ', ' }}
            }
          </span>
        </div>
      }

      <app-state
        [res]="res()"
        [empty]="reports().length === 0"
        emptyTitle="No investigation reports"
        emptyMessage="Reports appear here once an ML-detected attack has been investigated by the agents. NORMAL predictions never trigger the agents."
        emptyIcon="scan-search"
      >
        <section class="card">
          <app-data-table [columns]="columns" [rows]="reports()" [trackKey]="key" [clickable]="true" [pageSize]="25" [initialSort]="{ key: 'generated', dir: 'desc' }" (rowClick)="open($event)">
            <ng-template appCell="incident" let-r>
              <a class="mono" [routerLink]="['/incidents', r.incident_id]" (click)="$event.stopPropagation()" [attr.title]="r.incident_id">{{ r.incident_id.slice(0, 8) }}</a>
            </ng-template>
            <ng-template appCell="node" let-r><span class="mono">{{ r.node_id ?? '—' }}</span></ng-template>
            <ng-template appCell="attack" let-r><app-status-badge kind="attack" [value]="r.attack_type" /></ng-template>
            <ng-template appCell="hypothesis" let-r><span class="mono">{{ r.hypothesis?.confidence ?? null | pct }}</span></ng-template>
            <ng-template appCell="confirmation" let-r><span class="mono">{{ r.confirmation?.confidence ?? null | pct }}</span></ng-template>
            <ng-template appCell="status" let-r><app-status-badge kind="investigation" [value]="r.investigation_status" /></ng-template>
            <ng-template appCell="verdict" let-r><app-status-badge kind="verdict" [value]="r.displayVerdict" /></ng-template>
            <ng-template appCell="generated" let-r><span class="mono subtle" [attr.title]="r.generated_at">{{ r.generated_at | ago: clock.now() }}</span></ng-template>
          </app-data-table>
        </section>
      </app-state>
    </div>
  `,
})
export class InvestigationsPage {
  private readonly router = inject(Router);
  private readonly connection = inject(ConnectionService);
  protected readonly clock = inject(ClockService);
  private readonly service = inject(IncidentService);

  readonly res = toSignal(this.service.investigations$, { initialValue: initialResource<InvestigationReport[]>('live') });
  private readonly incidentsRes = toSignal(this.service.incidents$, { initialValue: initialResource<Incident[]>('live') });

  readonly reports = computed(() => this.res().data ?? []);
  readonly ongoing = computed(() =>
    (this.incidentsRes().data ?? []).filter((i) => i.investigation_status === 'NOT_STARTED' || i.investigation_status === 'RUNNING'),
  );

  /** From GET /health (raw payload): the agents are optional, ML runs without them. */
  readonly orchestratorDown = computed(() => {
    const raw = this.connection.health().data?.raw;
    return isObj(raw) && raw['orchestrator_loaded'] === false;
  });

  readonly columns: TableColumn<InvestigationReport>[] = [
    { key: 'incident', label: 'Incident', sortable: true, mono: true, sortValue: (r) => r.incident_id },
    { key: 'node', label: 'Node', sortable: true, mono: true, sortValue: (r) => r.node_id },
    { key: 'attack', label: 'ML attack', sortable: true, sortValue: (r) => r.attack_type },
    { key: 'hypothesis', label: 'Hypothesis conf.', align: 'right', sortable: true, sortValue: (r) => r.hypothesis?.confidence ?? null, hideBelow: 'md' },
    { key: 'confirmation', label: 'Confirmation conf.', align: 'right', sortable: true, sortValue: (r) => r.confirmation?.confidence ?? null, hideBelow: 'md' },
    { key: 'status', label: 'Status', sortable: true, sortValue: (r) => r.investigation_status },
    { key: 'verdict', label: 'Verdict', sortable: true, sortValue: (r) => r.displayVerdict },
    { key: 'generated', label: 'Generated', sortable: true, sortValue: (r) => r.generated_at },
  ];
  readonly key = (r: InvestigationReport) => `${r.incident_id}@${r.generated_at ?? ''}`;

  open(r: InvestigationReport): void {
    this.router.navigate(['/incidents', r.incident_id]);
  }
}
