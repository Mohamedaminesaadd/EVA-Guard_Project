import { ChangeDetectionStrategy, Component, model } from '@angular/core';
import { TIME_RANGES, TimeRange } from '../../../core/models/common.model';

@Component({
  selector: 'app-time-range-selector',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="segmented" role="radiogroup" aria-label="Time range">
      @for (r of ranges; track r.value) {
        <button
          type="button"
          role="radio"
          class="mono"
          [class.is-active]="value() === r.value"
          [attr.aria-checked]="value() === r.value"
          (click)="value.set(r.value)"
        >
          {{ r.label }}
        </button>
      }
    </div>
  `,
})
export class TimeRangeSelectorComponent {
  readonly value = model<TimeRange>('15m');
  protected readonly ranges = TIME_RANGES;
}
