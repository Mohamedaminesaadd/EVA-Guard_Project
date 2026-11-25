import { ChangeDetectionStrategy, Component, computed, input } from '@angular/core';
import { sparklineOption } from '../../charts/chart-options';
import { EchartDirective } from '../../charts/echart.directive';
import { Tone } from '../../../core/utils/labels';
import { IconComponent } from '../icon/icon';

export interface MetricTrend {
  /** Signed change vs. the reference point. */
  delta: number;
  /** Formatted delta shown to the user ("+2", "−4.1%"). */
  label: string;
  /** Whether an increase is good (green) or bad (red). */
  goodWhen: 'up' | 'down';
  /** Reference description, e.g. "vs 5 min ago". */
  reference: string;
}

/** KPI tile: value + unit + label + description, optional trend and sparkline. */
@Component({
  selector: 'app-metric-card',
  imports: [IconComponent, EchartDirective],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="mc card" [class.mc--compact]="compact()">
      <div class="mc__head">
        @if (icon()) {
          <span class="mc__icon tone-{{ tone() }}"><app-icon [name]="icon()!" [size]="14" /></span>
        }
        <span class="mc__label">{{ label() }}</span>
        @if (stale()) {
          <span class="mc__stale" title="Last refresh failed — showing the last value received">stale</span>
        }
      </div>

      @if (loading()) {
        <div class="skeleton mc__skel"></div>
        <div class="skeleton mc__skel mc__skel--sm"></div>
      } @else {
        <div class="mc__value-row">
          <span class="mc__value mono" [class.na]="value() === '—'">{{ value() }}</span>
          @if (unit() && value() !== '—') {
            <span class="mc__unit">{{ unit() }}</span>
          }
          @if (badge()) {
            <span class="pill pill--outline mc__badge" [attr.title]="badgeTitle() || null">{{ badge() }}</span>
          }
          @if (trend(); as t) {
            <span class="mc__trend" [class.is-good]="trendGood()" [class.is-bad]="trendBad()" [attr.title]="t.reference">
              <app-icon [name]="t.delta > 0 ? 'trending-up' : t.delta < 0 ? 'trending-down' : 'arrow-right'" [size]="12" />
              {{ t.label }}
            </span>
          }
        </div>
        @if (description()) {
          <div class="mc__desc">{{ description() }}</div>
        }
        @if (sparkOptions(); as opt) {
          <div class="mc__spark" [appEchart]="opt"></div>
        }
      }
    </div>
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .mc { padding: 14px 16px 12px; display: flex; flex-direction: column; gap: 6px; height: 100%; position: relative; overflow: hidden; }
    .mc__head { display: flex; align-items: center; gap: 8px; min-height: 22px; }
    .mc__icon {
      width: 22px; height: 22px; border-radius: 5px; display: inline-flex; align-items: center; justify-content: center;
      background: var(--tone-bg); color: var(--tone);
    }
    .mc__label { font-size: 12px; font-weight: 500; color: var(--eg-text-2); flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .mc__badge { margin-left: auto; align-self: center; height: 18px; font-size: 10px; font-weight: 500; color: var(--eg-text-2); }
    .mc__stale { font-size: 10px; font-weight: 600; color: var(--eg-warning-text); background: var(--eg-warning-bg); padding: 1px 6px; border-radius: 4px; text-transform: uppercase; letter-spacing: .04em; }
    .mc__value-row { display: flex; align-items: baseline; gap: 4px; flex-wrap: wrap; }
    .mc__value { font-size: 26px; font-weight: 500; letter-spacing: -0.03em; line-height: 1.15; color: var(--eg-text); }
    .mc--compact .mc__value { font-size: 20px; }
    .mc__unit { font-size: 13px; color: var(--eg-text-2); font-weight: 500; }
    .mc__trend {
      margin-left: auto; display: inline-flex; align-items: center; gap: 3px; font-size: 11px; font-weight: 600;
      color: var(--eg-text-2); background: var(--eg-surface-3); padding: 2px 6px; border-radius: 4px; font-family: var(--eg-font-mono);
    }
    .mc__trend.is-good { color: var(--eg-success-text); background: var(--eg-success-bg); }
    .mc__trend.is-bad { color: var(--eg-critical-text); background: var(--eg-critical-bg); }
    .mc__desc { font-size: 12px; color: var(--eg-text-2); line-height: 1.35; }
    .mc__spark { height: 32px; margin: 2px -4px -2px; }
    .mc__skel { height: 28px; width: 60%; }
    .mc__skel--sm { height: 12px; width: 85%; }
  `,
})
export class MetricCardComponent {
  readonly label = input.required<string>();
  readonly value = input<string>('—');
  readonly unit = input<string>('');
  readonly description = input<string>('');
  readonly icon = input<string | null>(null);
  readonly tone = input<Tone>('primary');
  readonly trend = input<MetricTrend | null>(null);
  readonly sparkline = input<(number | null)[] | null>(null);
  readonly sparkColor = input<string>('#635BFF');
  readonly loading = input(false);
  readonly stale = input(false);
  readonly badge = input<string>('');
  readonly badgeTitle = input<string>('');
  readonly compact = input(false);

  readonly trendGood = computed(() => {
    const t = this.trend();
    return !!t && t.delta !== 0 && (t.delta > 0) === (t.goodWhen === 'up');
  });
  readonly trendBad = computed(() => {
    const t = this.trend();
    return !!t && t.delta !== 0 && (t.delta > 0) !== (t.goodWhen === 'up');
  });

  readonly sparkOptions = computed(() => {
    const s = this.sparkline();
    return s && s.filter((v) => v !== null).length >= 2 ? sparklineOption(s, this.sparkColor()) : null;
  });
}
