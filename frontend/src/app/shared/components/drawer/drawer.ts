import { ChangeDetectionStrategy, Component, ElementRef, effect, input, model, viewChild } from '@angular/core';
import { IconComponent } from '../icon/icon';

/** Right-side details drawer (Esc / backdrop closes, focus moves into the panel). */
@Component({
  selector: 'app-drawer',
  imports: [IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: { '(document:keydown.escape)': 'open() && open.set(false)' },
  template: `
    @if (open()) {
      <div class="backdrop" (click)="open.set(false)"></div>
      <aside class="drawer" role="dialog" aria-modal="true" [attr.aria-label]="title()" [style.width.px]="width()" #panel tabindex="-1">
        <header class="drawer__head">
          <div class="drawer__titles">
            @if (eyebrow()) {
              <div class="drawer__eyebrow">{{ eyebrow() }}</div>
            }
            <h2 class="drawer__title">{{ title() }}</h2>
          </div>
          <ng-content select="[drawer-actions]" />
          <button class="btn btn--ghost btn--icon" type="button" (click)="open.set(false)" aria-label="Close">
            <app-icon name="x" [size]="16" />
          </button>
        </header>
        <div class="drawer__body"><ng-content /></div>
        <ng-content select="[drawer-footer]" />
      </aside>
    }
  `,
  styles: `
    .backdrop { position: fixed; inset: 0; background: rgba(16, 24, 40, .28); z-index: 80; animation: fade .12s ease; }
    .drawer {
      position: fixed; top: 0; right: 0; bottom: 0; max-width: 100vw; background: var(--eg-surface); z-index: 81;
      display: flex; flex-direction: column; box-shadow: var(--eg-shadow-lg); border-left: 1px solid var(--eg-border);
      animation: slide .16s ease; outline: none;
    }
    .drawer__head { display: flex; align-items: center; gap: 8px; padding: 14px 16px; border-bottom: 1px solid var(--eg-border); }
    .drawer__titles { flex: 1; min-width: 0; }
    .drawer__eyebrow { font-size: 11px; font-weight: 600; color: var(--eg-text-2); text-transform: uppercase; letter-spacing: .05em; }
    .drawer__title { font-size: 15px; font-weight: 600; font-family: var(--eg-font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .drawer__body { flex: 1; overflow-y: auto; padding: 16px; display: flex; flex-direction: column; gap: 16px; }
    @keyframes slide { from { transform: translateX(24px); opacity: .6; } to { transform: none; opacity: 1; } }
    @keyframes fade { from { opacity: 0; } to { opacity: 1; } }
    @media (prefers-reduced-motion: reduce) { .drawer, .backdrop { animation: none; } }
  `,
})
export class DrawerComponent {
  readonly open = model(false);
  readonly title = input('');
  readonly eyebrow = input('');
  readonly width = input(480);
  private readonly panel = viewChild<ElementRef<HTMLElement>>('panel');

  constructor() {
    effect(() => {
      const p = this.panel();
      if (this.open() && p) queueMicrotask(() => p.nativeElement.focus());
    });
  }
}
