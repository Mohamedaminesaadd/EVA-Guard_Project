import { ChangeDetectionStrategy, Component, computed, inject, input } from '@angular/core';
import { Resource } from '../../../core/models/common.model';
import { ClockService } from '../../../core/services/clock.service';
import { SettingsService } from '../../../core/services/settings.service';
import { timeAgo } from '../../../core/utils/format';

/** Compact "data freshness" chip: Live · Demo · Stale · Error · Paused, with last update time. */
@Component({
  selector: 'app-live-indicator',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="li tone-{{ view().tone }}" [attr.title]="view().title">
      <span class="dot" [class.dot--pulse]="view().pulse"></span>
      <span class="li__label">{{ view().label }}</span>
      @if (res()?.updatedAt) {
        <span class="li__time mono">{{ ago() }}</span>
      }
    </span>
  `,
  styles: `
    :host { display: inline-flex; }
    .li { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 10px; border: 1px solid var(--eg-border); border-radius: 999px; background: var(--eg-surface); font-size: 12px; font-weight: 500; white-space: nowrap; }
    .li__label { color: var(--eg-text); }
    .li__time { color: var(--eg-text-3); font-size: 11px; }
  `,
})
export class LiveIndicatorComponent {
  readonly res = input<Resource<unknown> | null>(null);
  readonly paused = input(false);
  private readonly clock = inject(ClockService);
  private readonly settings = inject(SettingsService);

  readonly ago = computed(() => timeAgo(this.res()?.updatedAt ?? null, this.clock.now()));

  readonly view = computed(() => {
    const r = this.res();
    const interval = `${this.settings.pollingIntervalMs() / 1000}s`;
    if (this.paused()) return { tone: 'offline', label: 'Paused', pulse: false, title: 'Auto-refresh paused' };
    if (!r || (r.status === 'loading' && !r.data)) return { tone: 'info', label: 'Loading', pulse: true, title: 'Waiting for first response' };
    if (r.source === 'demo') return { tone: 'warning', label: 'Demo data', pulse: true, title: `Simulated data · refresh every ${interval}` };
    if (r.status === 'error') {
      return r.stale
        ? { tone: 'warning', label: 'Stale', pulse: false, title: r.error?.message ?? 'Refresh failed' }
        : { tone: 'critical', label: 'Error', pulse: false, title: r.error?.message ?? 'Request failed' };
    }
    return { tone: 'success', label: 'Live', pulse: true, title: `Auto-refresh every ${interval}` };
  });
}
