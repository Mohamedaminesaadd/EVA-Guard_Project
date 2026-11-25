import { ChangeDetectionStrategy, Component, computed, inject, input, output } from '@angular/core';
import { RouterLink } from '@angular/router';
import { ApiError, Resource } from '../../../core/models/common.model';
import { PollingService } from '../../../core/services/polling.service';
import { SettingsService } from '../../../core/services/settings.service';
import { IconComponent } from '../icon/icon';

@Component({
  selector: 'app-loading-state',
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="ls" role="status" aria-live="polite">
      @for (r of rowsArr(); track $index) {
        <div class="skeleton ls__row" [style.width.%]="r"></div>
      }
      <span class="sr-only">{{ label() }}</span>
    </div>
  `,
  styles: `
    .ls { display: flex; flex-direction: column; gap: 10px; padding: 16px; }
    .ls__row { height: 14px; }
  `,
})
export class LoadingStateComponent {
  readonly rows = input(4);
  readonly label = input('Loading…');
  readonly rowsArr = computed(() => Array.from({ length: this.rows() }, (_, i) => 92 - ((i * 17) % 40)));
}

@Component({
  selector: 'app-empty-state',
  imports: [IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="es" [class.es--compact]="compact()">
      <span class="es__icon"><app-icon [name]="icon()" [size]="18" /></span>
      <div class="es__title">{{ title() }}</div>
      @if (message()) {
        <div class="es__msg">{{ message() }}</div>
      }
      <ng-content />
    </div>
  `,
  styles: `
    .es { display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 6px; padding: 32px 16px; color: var(--eg-text-2); }
    .es--compact { padding: 18px 12px; }
    .es__icon { width: 36px; height: 36px; border-radius: 8px; background: var(--eg-surface-3); display: inline-flex; align-items: center; justify-content: center; color: var(--eg-text-2); margin-bottom: 4px; }
    .es__title { font-weight: 600; color: var(--eg-text); font-size: 13px; }
    .es__msg { font-size: 12px; max-width: 420px; line-height: 1.5; }
  `,
})
export class EmptyStateComponent {
  readonly icon = input('circle-dot');
  readonly title = input('No data');
  readonly message = input('');
  readonly compact = input(false);
}

@Component({
  selector: 'app-error-state',
  imports: [IconComponent, RouterLink],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="er" [class.er--compact]="compact()" role="alert">
      <span class="er__icon"><app-icon [name]="error()?.network ? 'unplug' : 'triangle-exclamation-point'" [size]="18" /></span>
      <div class="er__title">{{ error()?.network ? 'FastAPI backend unreachable' : 'Request failed' }}</div>
      <div class="er__msg">{{ error()?.message }}</div>
      @if (error()?.url) {
        <code class="er__url">GET {{ error()?.url }}</code>
      }
      <div class="er__actions">
        <button class="btn btn--sm" type="button" (click)="retry()">Retry</button>
        @if (!settings.isDemo()) {
          <a class="btn btn--sm btn--ghost" routerLink="/settings">Connection settings</a>
        }
      </div>
    </div>
  `,
  styles: `
    .er { display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 6px; padding: 28px 16px; }
    .er--compact { padding: 16px 12px; }
    .er__icon { width: 36px; height: 36px; border-radius: 8px; background: var(--eg-critical-bg); color: var(--eg-critical); display: inline-flex; align-items: center; justify-content: center; margin-bottom: 4px; }
    .er__title { font-weight: 600; font-size: 13px; }
    .er__msg { font-size: 12px; color: var(--eg-text-2); max-width: 460px; }
    .er__url { font-size: 11px; color: var(--eg-text-3); background: var(--eg-surface-2); padding: 2px 6px; border-radius: 4px; max-width: 100%; overflow-wrap: anywhere; }
    .er__actions { display: flex; gap: 8px; margin-top: 6px; }
  `,
})
export class ErrorStateComponent {
  readonly error = input<ApiError | null>(null);
  readonly compact = input(false);
  readonly retried = output<void>();
  protected readonly settings = inject(SettingsService);
  private readonly polling = inject(PollingService);

  retry(): void {
    this.polling.refreshNow();
    this.retried.emit();
  }
}

/**
 * Renders the right state for a polled resource:
 * loading (no data yet) → error (no data) → empty → content (+ stale notice).
 */
@Component({
  selector: 'app-state',
  imports: [LoadingStateComponent, ErrorStateComponent, EmptyStateComponent, IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @switch (view()) {
      @case ('loading') {
        <app-loading-state [rows]="loadingRows()" />
      }
      @case ('error') {
        <app-error-state [error]="res()?.error ?? null" [compact]="compact()" />
      }
      @case ('empty') {
        <app-empty-state [icon]="emptyIcon()" [title]="emptyTitle()" [message]="emptyMessage()" [compact]="compact()" />
      }
      @default {
        @if (res()?.stale && showStale()) {
          <div class="stale" role="status">
            <app-icon name="triangle-exclamation-point" [size]="13" />
            Showing last data received — latest refresh failed: {{ res()?.error?.message }}
          </div>
        }
        <ng-content />
      }
    }
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .stale { display: flex; align-items: center; gap: 6px; font-size: 11.5px; color: var(--eg-warning-text); background: var(--eg-warning-bg); padding: 6px 12px; border-bottom: 1px solid #fedf89; }
  `,
})
export class StateComponent {
  readonly res = input<Resource<unknown> | null>(null);
  readonly empty = input(false);
  readonly emptyTitle = input('No data yet');
  readonly emptyMessage = input('');
  readonly emptyIcon = input('circle-dot');
  readonly loadingRows = input(4);
  readonly compact = input(false);
  readonly showStale = input(true);

  readonly view = computed<'loading' | 'error' | 'empty' | 'content'>(() => {
    const r = this.res();
    if (!r) return 'loading';
    if (r.data === null || r.data === undefined) {
      if (r.status === 'error') return 'error';
      return 'loading';
    }
    if (this.empty()) return 'empty';
    return 'content';
  });
}
