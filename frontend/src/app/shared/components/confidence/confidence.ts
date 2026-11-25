import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { ATTACK_CLASSES, PredictionLabel } from '../../../core/models/common.model';
import { ClassProbabilities } from '../../../core/models/prediction.model';
import { ATTACK_COLOR } from '../../../core/utils/labels';
import { fmtPct } from '../../../core/utils/format';

/** Confidence value + thin bar coloured by predicted class. */
@Component({
  selector: 'app-confidence',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="cf" [attr.title]="title()">
      <span class="cf__bar"><span [style.width.%]="pct()" [style.background]="color()"></span></span>
      <span class="cf__val mono">{{ text() }}</span>
    </span>
  `,
  styles: `
    :host { display: inline-flex; }
    .cf { display: inline-flex; align-items: center; gap: 8px; }
    .cf__bar { width: 56px; height: 6px; border-radius: 3px; background: var(--eg-surface-3); overflow: hidden; display: inline-block; }
    .cf__bar span { display: block; height: 100%; border-radius: 3px; }
    .cf__val { font-size: 12px; min-width: 44px; text-align: right; }
  `,
})
export class ConfidenceComponent {
  readonly value = input<number | null>(null);
  readonly label = input<PredictionLabel | null>(null);
  readonly pct = computed(() => Math.max(0, Math.min(100, (this.value() ?? 0) * 100)));
  readonly text = computed(() => fmtPct(this.value()));
  readonly color = computed(() => ATTACK_COLOR[this.label() ?? 'UNKNOWN'] ?? '#635BFF');
  readonly title = computed(() => `ML confidence ${this.text()}`);
}

/** Stacked probability bar (NORMAL / ECLIPSE / RPC_FLOODING) with accessible title. */
@Component({
  selector: 'app-prob-bar',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="prob-bar" [attr.title]="title()" role="img" [attr.aria-label]="title()">
      @for (s of segments(); track s.cls) {
        <span [style.width.%]="s.pct" [style.background]="s.color"></span>
      }
    </div>
  `,
  styles: `:host { display: block; min-width: 80px; }`,
})
export class ProbBarComponent {
  readonly probabilities = input<ClassProbabilities | null>(null);
  readonly segments = computed(() => {
    const p = this.probabilities();
    if (!p) return [];
    return ATTACK_CLASSES.filter((c) => (p[c] ?? 0) > 0).map((c) => ({ cls: c, pct: (p[c] ?? 0) * 100, color: ATTACK_COLOR[c] }));
  });
  readonly title = computed(() => {
    const p = this.probabilities();
    if (!p) return 'No probabilities';
    return ATTACK_CLASSES.map((c) => `${c} ${fmtPct(p[c])}`).join(' · ');
  });
}
