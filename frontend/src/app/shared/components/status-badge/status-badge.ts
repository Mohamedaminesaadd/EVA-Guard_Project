import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { AlertStatus } from '../../../core/models/alert.model';
import { ComponentState, NodeHealth, PredictionLabel, Severity } from '../../../core/models/common.model';
import { InvestigationState, Verdict } from '../../../core/models/incident.model';
import {
  ALERT_STATUS_LABEL,
  ALERT_STATUS_TONE,
  ATTACK_ICON,
  ATTACK_LABEL,
  ATTACK_TONE,
  COMPONENT_LABEL,
  COMPONENT_TONE,
  HEALTH_ICON,
  HEALTH_LABEL,
  HEALTH_TONE,
  INVESTIGATION_STATE_LABEL,
  INVESTIGATION_STATE_TONE,
  SEVERITY_LABEL,
  SEVERITY_TONE,
  Tone,
  VERDICT_ICON,
  VERDICT_LABEL,
  VERDICT_TONE,
} from '../../../core/utils/labels';
import { IconComponent } from '../icon/icon';

export type BadgeKind = 'health' | 'attack' | 'severity' | 'component' | 'alert' | 'verdict' | 'investigation';

/**
 * Status pill. Colour is never the only carrier of meaning: every badge has a
 * text label and (for health / attack) an icon.
 */
@Component({
  selector: 'app-status-badge',
  imports: [IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="badge tone-{{ view().tone }}" [class.badge--solid]="solid()" [attr.title]="title() || null">
      @if (view().icon) {
        <app-icon [name]="view().icon!" [size]="12" [strokeWidth]="2.25" />
      } @else {
        <span class="dot"></span>
      }
      <span>{{ label() || view().label }}</span>
    </span>
  `,
  styles: `
    :host { display: inline-flex; }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 5px;
      height: 22px;
      padding: 0 8px 0 7px;
      border-radius: 999px;
      font-size: 11px;
      font-weight: 600;
      white-space: nowrap;
      background: var(--tone-bg);
      color: var(--tone-text);
      border: 1px solid color-mix(in srgb, var(--tone) 22%, transparent);
    }
    .badge app-icon { color: var(--tone); }
    .badge--solid { background: var(--tone); color: #fff; border-color: var(--tone); }
    .badge--solid app-icon { color: #fff; }
  `,
})
export class StatusBadgeComponent {
  readonly kind = input.required<BadgeKind>();
  readonly value = input.required<string | null | undefined>();
  readonly label = input<string>('');
  readonly title = input<string>('');
  readonly solid = input(false);

  readonly view = computed<{ tone: Tone; label: string; icon: string | null }>(() => {
    const v = this.value();
    switch (this.kind()) {
      case 'health': {
        const h = (v ?? 'unknown') as NodeHealth;
        return { tone: HEALTH_TONE[h] ?? 'offline', label: HEALTH_LABEL[h] ?? String(v), icon: HEALTH_ICON[h] ?? null };
      }
      case 'attack': {
        const a = (v ?? 'UNKNOWN') as PredictionLabel;
        return { tone: ATTACK_TONE[a] ?? 'offline', label: a === 'UNKNOWN' ? 'UNKNOWN' : a, icon: ATTACK_ICON[a] ?? null };
      }
      case 'severity': {
        const s = (v ?? 'medium') as Severity;
        return { tone: SEVERITY_TONE[s] ?? 'offline', label: SEVERITY_LABEL[s] ?? String(v), icon: null };
      }
      case 'component': {
        const c = (v ?? 'unknown') as ComponentState;
        return { tone: COMPONENT_TONE[c] ?? 'offline', label: COMPONENT_LABEL[c] ?? String(v), icon: null };
      }
      case 'alert': {
        const s = (v ?? 'active') as AlertStatus;
        return { tone: ALERT_STATUS_TONE[s] ?? 'offline', label: ALERT_STATUS_LABEL[s] ?? String(v), icon: null };
      }
      case 'verdict': {
        const d = (v ?? 'INCONCLUSIVE') as Verdict;
        return { tone: VERDICT_TONE[d] ?? 'offline', label: VERDICT_LABEL[d] ?? String(v), icon: VERDICT_ICON[d] ?? null };
      }
      case 'investigation': {
        const i = (v ?? 'UNKNOWN') as InvestigationState;
        return { tone: INVESTIGATION_STATE_TONE[i] ?? 'offline', label: INVESTIGATION_STATE_LABEL[i] ?? String(v), icon: null };
      }
    }
  });

  protected readonly attackLabels = ATTACK_LABEL;
}
