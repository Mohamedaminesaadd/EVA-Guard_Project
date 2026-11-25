import { JsonPipe } from '@angular/common';
import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { toObservable, toSignal } from '@angular/core/rxjs-interop';
import { RouterLink } from '@angular/router';
import { switchMap } from 'rxjs';
import { initialResource } from '../../core/models/common.model';
import { Incident, InvestigationReport } from '../../core/models/incident.model';
import { ClockService } from '../../core/services/clock.service';
import { IncidentService } from '../../core/services/incident.service';
import { ConfidenceComponent, ProbBarComponent } from '../../shared/components/confidence/confidence';
import { IconComponent } from '../../shared/components/icon/icon';
import { LiveIndicatorComponent } from '../../shared/components/live-indicator/live-indicator';
import { StateComponent } from '../../shared/components/states/states';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { AgoPipe, DateTimePipe } from '../../shared/pipes/format.pipes';
import { AgentAssessmentComponent } from './agent-assessment';

@Component({
  selector: 'app-incident-detail-page',
  imports: [
    RouterLink,
    JsonPipe,
    StatusBadgeComponent,
    ConfidenceComponent,
    ProbBarComponent,
    StateComponent,
    IconComponent,
    LiveIndicatorComponent,
    AgentAssessmentComponent,
    AgoPipe,
    DateTimePipe,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="page">
      <a routerLink="/incidents" class="back"><app-icon name="chevron-left" [size]="14" /> All incidents</a>

      <app-state [res]="res()" [loadingRows]="6">
        @if (incident(); as i) {
          <header class="card hero tone-{{ heroTone() }}">
            <div class="hero__main">
              <span class="eyebrow">Incident</span>
              <h1 class="mono">{{ i.incident_id }}</h1>
              <div class="row row--wrap">
                <app-status-badge kind="verdict" [value]="verdict()" [solid]="verdict() === 'CONFIRMED'" />
                <app-status-badge kind="attack" [value]="i.attack_type" />
                <app-status-badge kind="investigation" [value]="report()?.investigation_status ?? i.investigation_status" />
                <span class="muted">Node <a class="mono" [routerLink]="['/nodes', i.node_id]">{{ i.node_id }}</a></span>
                @if (i.time) {
                  <span class="muted">Detected <span class="mono" [attr.title]="i.time | dt">{{ i.time | ago: clock.now() }}</span></span>
                }
              </div>
            </div>
            <div class="hero__side">
              <app-live-indicator [res]="res()" />
            </div>
          </header>

          @if (!hasAgentVerdict()) {
            <div class="banner tone-warning">
              <app-icon name="shield-exclamation-point" [size]="15" />
              <span>
                <strong>Suspected attack.</strong>
                @switch (report()?.investigation_status ?? i.investigation_status) {
                  @case ('NOT_STARTED') { The ML model flagged this attack and the AI investigation is queued. }
                  @case ('RUNNING') { The ML model flagged this attack and the AI investigation is in progress. }
                  @default { No verdict from the Confirmation Agent is available. This detection comes from the ML model only. }
                }
              </span>
            </div>
          }

          <section class="grid grid-2">
            <div class="card">
              <div class="card__header"><div class="card__title"><app-icon name="brain-circuit" [size]="15" /> ML detection</div></div>
              <div class="card__body stack">
                <div class="row">
                  <app-status-badge kind="attack" [value]="i.attack_type" />
                  <app-confidence [value]="i.ml_confidence" [label]="i.attack_type" />
                </div>
                @if (i.probabilities) {
                  <app-prob-bar [probabilities]="i.probabilities" />
                } @else {
                  <p class="na">Class probabilities are not stored in persisted reports (incident loaded after a backend restart).</p>
                }
                <dl class="kv">
                  <dt>Prediction time</dt><dd class="mono">{{ (i.prediction_timestamp | dt) || '—' }}</dd>
                  <dt>Incident created</dt><dd class="mono">{{ (i.created_at | dt) || '—' }}</dd>
                  <dt>Incident status</dt><dd class="mono">{{ i.status ?? '—' }}</dd>
                </dl>
              </div>
            </div>

            <div class="card">
              <div class="card__header"><div class="card__title"><app-icon name="waypoints" [size]="15" /> Investigation workflow</div></div>
              <div class="card__body stack">
                <dl class="kv">
                  <dt>Started</dt><dd class="mono">{{ (i.investigation_started_at | dt) || '—' }}</dd>
                  <dt>Report generated</dt><dd class="mono">{{ (report()?.generated_at ?? i.investigation_completed_at | dt) || '—' }}</dd>
                </dl>
                @if (report()?.error; as err) {
                  <div class="banner tone-critical"><app-icon name="triangle-exclamation-point" [size]="15" /><span>{{ err }}</span></div>
                }
                @if (report()?.history?.length) {
                  <ol class="steps">
                    @for (s of report()!.history; track $index) {
                      <li>
                        <span class="mono">{{ s.step }}</span>
                        <span class="pill pill--outline" [class.tone-success]="s.status === 'SUCCESS'" [class.tone-critical]="s.status !== 'SUCCESS'">{{ s.status }}</span>
                        @if (s.details) {
                          <span class="subtle">{{ s.details }}</span>
                        }
                      </li>
                    }
                  </ol>
                } @else {
                  <p class="na">No workflow step recorded yet.</p>
                }
              </div>
            </div>
          </section>

          <h2 class="section-title"><app-icon name="sparkles" [size]="14" /> AI investigation</h2>
          @if (reportRes().status === 'error' && !report()) {
            <div class="banner tone-critical">
              <app-icon name="triangle-exclamation-point" [size]="15" />
              <span>Investigation report unavailable: {{ reportRes().error?.message }}</span>
            </div>
          }
          <app-agent-assessment
            title="Hypothesis Agent"
            subtitle="Investigates the ML detection with RAG"
            icon="scan-search"
            [a]="report()?.hypothesis ?? null"
            [emptyText]="pendingText()"
          />
          <app-agent-assessment
            title="Confirmation Agent"
            subtitle="Independently evaluates the hypothesis"
            icon="shield-check"
            [a]="report()?.confirmation ?? null"
            [emptyText]="pendingText()"
          />

          @if (i.observation_snapshot) {
            <details class="card snapshot">
              <summary class="card__header"><span class="card__title"><app-icon name="database" [size]="15" /> Observation snapshot at detection time</span></summary>
              <pre class="mono">{{ i.observation_snapshot | json }}</pre>
            </details>
          }
        }
      </app-state>
    </div>
  `,
  styles: `
    .back { display: inline-flex; align-items: center; gap: 4px; font-size: 12.5px; color: var(--eg-text-2); align-self: flex-start; }
    .hero { display: flex; justify-content: space-between; gap: 16px; padding: 18px 20px; border-left: 3px solid var(--tone); flex-wrap: wrap; }
    .hero__main { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
    .hero h1 { margin: 0; font-size: 17px; word-break: break-all; }
    .eyebrow { font-size: 11px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--eg-text-2); }
    .steps { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 6px; }
    .steps li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
    .snapshot summary { cursor: pointer; list-style: none; }
    .snapshot pre { margin: 0; padding: 12px 16px; max-height: 420px; overflow: auto; font-size: 11.5px; border-top: 1px solid var(--eg-border); }
  `,
})
export class IncidentDetailPage {
  /** Route param bound by withComponentInputBinding(). */
  readonly incidentId = input.required<string>();

  private readonly service = inject(IncidentService);
  protected readonly clock = inject(ClockService);

  private readonly id$ = toObservable(this.incidentId);
  readonly res = toSignal(this.id$.pipe(switchMap((id) => this.service.incident$(id))), { initialValue: initialResource<Incident>('live') });
  readonly reportRes = toSignal(this.id$.pipe(switchMap((id) => this.service.investigation$(id))), {
    initialValue: initialResource<InvestigationReport>('live'),
  });

  readonly incident = computed(() => this.res().data);
  readonly report = computed(() => this.reportRes().data);

  /** The report verdict wins over the incident list (it is the most recent source). */
  readonly verdict = computed(() => {
    const r = this.report();
    if (r?.verdict) return r.verdict;
    return this.incident()?.displayVerdict ?? 'SUSPECTED';
  });

  readonly hasAgentVerdict = computed(() => !!(this.report()?.verdict ?? this.incident()?.verdict));

  readonly heroTone = computed(() => {
    switch (this.verdict()) {
      case 'CONFIRMED':
        return 'critical';
      case 'REJECTED':
        return 'success';
      case 'INCONCLUSIVE':
        return 'offline';
      default:
        return 'warning';
    }
  });

  readonly pendingText = computed(() => {
    const state = this.report()?.investigation_status ?? this.incident()?.investigation_status;
    if (state === 'NOT_STARTED') return 'Investigation queued.';
    if (state === 'RUNNING') return 'Investigation in progress…';
    return 'No output was produced for this incident.';
  });
}
