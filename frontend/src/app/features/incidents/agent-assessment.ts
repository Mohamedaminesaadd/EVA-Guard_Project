import { ChangeDetectionStrategy, Component, input } from '@angular/core';
import { AgentAssessment } from '../../core/models/incident.model';
import { IconComponent } from '../../shared/components/icon/icon';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge';
import { PctPipe } from '../../shared/pipes/format.pipes';

/** Read-only rendering of a Hypothesis / Confirmation Agent output. */
@Component({
  selector: 'app-agent-assessment',
  imports: [IconComponent, StatusBadgeComponent, PctPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card assessment">
      <div class="card__header">
        <div class="card__title">
          <app-icon [name]="icon()" [size]="15" />
          {{ title() }}
          <span class="card__subtitle">{{ subtitle() }}</span>
        </div>
        @if (a(); as a) {
          <div class="row">
            @if (a.verdict) {
              <app-status-badge kind="verdict" [value]="a.verdict" />
            }
            @if (a.attackHypothesis) {
              <app-status-badge kind="attack" [value]="a.attackHypothesis" />
            }
            <span class="pill pill--outline mono" title="Agent confidence">{{ a.confidence | pct }}</span>
          </div>
        }
      </div>

      @if (a(); as a) {
        <div class="card__body stack">
          <div>
            <h3 class="label">Reasoning</h3>
            <p class="reasoning">{{ a.reasoning ?? 'Not reported.' }}</p>
          </div>

          <div class="grid grid-2">
            <div>
              <h3 class="label"><app-icon name="circle-check" [size]="13" /> Supporting evidence</h3>
              @if (a.supporting.length) {
                <ul class="evidence">
                  @for (e of a.supporting; track $index) {
                    <li>
                      {{ e.description }}
                      @if (e.metric) {
                        <span class="mono subtle">{{ e.metric }}@if (e.observed !== null) { = {{ e.observed }}}@if (e.expected !== null) { (expected {{ e.expected }})}</span>
                      }
                    </li>
                  }
                </ul>
              } @else {
                <p class="na">None reported.</p>
              }
            </div>
            <div>
              <h3 class="label"><app-icon name="circle-x" [size]="13" /> Contradicting evidence</h3>
              @if (a.contradicting.length) {
                <ul class="evidence">
                  @for (e of a.contradicting; track $index) {
                    <li>
                      {{ e.description }}
                      @if (e.metric) {
                        <span class="mono subtle">{{ e.metric }}@if (e.observed !== null) { = {{ e.observed }}}@if (e.expected !== null) { (expected {{ e.expected }})}</span>
                      }
                    </li>
                  }
                </ul>
              } @else {
                <p class="na">None reported.</p>
              }
            </div>
          </div>

          @if (a.alternatives.length) {
            <div>
              <h3 class="label">Alternative explanations</h3>
              <ul class="evidence">
                @for (x of a.alternatives; track $index) {
                  <li>{{ x }}</li>
                }
              </ul>
            </div>
          }

          @if (a.recommendations.length) {
            <div>
              <h3 class="label">Recommendations</h3>
              <ol class="evidence">
                @for (x of a.recommendations; track $index) {
                  <li>{{ x }}</li>
                }
              </ol>
            </div>
          }

          <div>
            <h3 class="label"><app-icon name="database" [size]="13" /> RAG sources</h3>
            @if (a.ragSources.length) {
              <ul class="sources">
                @for (s of a.ragSources; track $index) {
                  <li>
                    <app-icon name="file-text" [size]="13" />
                    <span class="mono">{{ s.file_name }}</span>
                    @if (s.category) {
                      <span class="pill pill--outline">{{ s.category }}</span>
                    }
                    @if (s.distance !== null) {
                      <span class="mono subtle" title="Vector distance (lower is closer)">d={{ s.distance.toFixed(3) }}</span>
                    }
                  </li>
                }
              </ul>
            } @else {
              <p class="na">No knowledge-base source cited.</p>
            }
          </div>
        </div>
      } @else {
        <div class="card__body"><p class="na">{{ emptyText() }}</p></div>
      }
    </section>
  `,
  styles: `
    .label { display: flex; align-items: center; gap: 6px; font-size: 11.5px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.04em; color: var(--eg-text-2); margin: 0 0 6px; }
    .reasoning { margin: 0; white-space: pre-wrap; line-height: 1.55; }
    .evidence { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 4px; line-height: 1.5; }
    .evidence .mono { margin-left: 6px; font-size: 11.5px; }
    .sources { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
    .sources li { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  `,
})
export class AgentAssessmentComponent {
  readonly a = input<AgentAssessment | null>(null);
  readonly title = input.required<string>();
  readonly subtitle = input('');
  readonly icon = input('bot');
  readonly emptyText = input('No output yet.');
}
