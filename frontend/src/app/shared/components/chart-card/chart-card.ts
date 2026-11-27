import { NgTemplateOutlet } from '@angular/common';
import { ChangeDetectionStrategy, Component, input } from '@angular/core';
import { Resource } from '../../../core/models/common.model';
import { IconComponent } from '../icon/icon';
import { EmptyStateComponent, StateComponent } from '../states/states';

/** Card wrapper for a chart: header (title, subtitle, actions), state handling, fixed-height body. */
@Component({
  selector: 'app-chart-card',
  imports: [StateComponent, EmptyStateComponent, IconComponent, NgTemplateOutlet],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="card cc">
      <header class="cc__header">
        <div class="cc__titles">
          <h3 class="cc__title">
            @if (icon()) {
              <app-icon [name]="icon()!" [size]="14" />
            }
            {{ title() }}
            @if (unit()) {
              <span class="cc__unit">{{ unit() }}</span>
            }
          </h3>
          @if (subtitle()) {
            <p class="cc__subtitle">{{ subtitle() }}</p>
          }
        </div>
        <div class="cc__actions"><ng-content select="[chart-actions]" /></div>
      </header>
      <div class="cc__body" [style.min-height.px]="height()">
        @if (res() !== undefined) {
          <app-state
            [res]="res() ?? null"
            [empty]="empty()"
            [emptyTitle]="emptyTitle()"
            [emptyMessage]="emptyMessage()"
            [emptyIcon]="'chart-line'"
            [compact]="true"
            [loadingRows]="3"
          >
            <div class="cc__chart" [style.min-height.px]="height()"><ng-container *ngTemplateOutlet="body" /></div>
          </app-state>
        } @else if (empty()) {
          <app-empty-state icon="chart-line" [title]="emptyTitle()" [message]="emptyMessage()" [compact]="true" />
        } @else {
          <div class="cc__chart" [style.min-height.px]="height()"><ng-container *ngTemplateOutlet="body" /></div>
        }
      </div>
      <ng-template #body><ng-content /></ng-template>
      <ng-content select="[chart-footer]" />
    </section>
  `,
  styles: `
    :host { display: block; min-width: 0; }
    .cc { display: flex; flex-direction: column; height: 100%; }
    .cc__header { display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; padding: 12px 14px 0; }
    .cc__titles { min-width: 0; }
    .cc__title { font-size: 13px; font-weight: 600; display: flex; align-items: center; gap: 6px; color: var(--eg-text); }
    .cc__title app-icon { color: var(--eg-text-2); }
    .cc__unit { font-weight: 400; color: var(--eg-text-3); font-size: 12px; font-family: var(--eg-font-mono); }
    .cc__subtitle { font-size: 11.5px; color: var(--eg-text-2); margin-top: 1px; }
    .cc__actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
    .cc__body { padding: 8px 10px 10px; flex: 1; display: flex; flex-direction: column; }
    .cc__body > app-state { flex: 1; display: flex; flex-direction: column; }
    .cc__chart { width: 100%; flex: 1; display: flex; flex-direction: column; }
    :host ::ng-deep .cc__chart > * { flex: 1; min-height: 0; }
  `,
})
export class ChartCardComponent {
  readonly title = input.required<string>();
  readonly subtitle = input<string>('');
  readonly unit = input<string>('');
  readonly icon = input<string | null>(null);
  readonly height = input(220);
  /** Pass a resource to get loading/error/empty handling; omit for static content. */
  readonly res = input<Resource<unknown> | null | undefined>(undefined);
  readonly empty = input(false);
  readonly emptyTitle = input('No data in this range');
  readonly emptyMessage = input('');
}
