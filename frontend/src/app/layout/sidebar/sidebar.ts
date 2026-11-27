import { ChangeDetectionStrategy, Component, input, output } from '@angular/core';
import { RouterLink, RouterLinkActive } from '@angular/router';
import { IconComponent } from '../../shared/components/icon/icon';
import { NAV_SECTIONS } from '../nav.config';

@Component({
  selector: 'app-sidebar',
  imports: [RouterLink, RouterLinkActive, IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '[class.is-collapsed]': 'collapsed()',
    '[class.is-mobile-open]': 'mobileOpen()',
  },
  template: `
    <div class="sb">
      <div class="sb__brand">
        <a routerLink="/dashboard" class="sb__logo" aria-label="EVA-Guard home" (click)="navigate.emit()">
          <svg viewBox="0 0 32 32" width="30" height="30" aria-hidden="true">
            <rect width="32" height="32" rx="7" fill="#635BFF" />
            <path d="M16 5.5 7.5 9v6.2c0 5.4 3.6 9.6 8.5 11.3 4.9-1.7 8.5-5.9 8.5-11.3V9L16 5.5Z" fill="none" stroke="#fff" stroke-width="2" stroke-linejoin="round" />
            <path d="m16 10.5-3.6 5.6L16 18l3.6-1.9L16 10.5Zm-3.6 6.6L16 22l3.6-4.9L16 19l-3.6-1.9Z" fill="#fff" />
          </svg>
          @if (!collapsed()) {
            <span class="sb__brand-text">
              <span class="sb__name">EVA-Guard</span>
              <span class="sb__tagline">Ethereum Security Intelligence</span>
            </span>
          }
        </a>
        <button class="sb__close" type="button" (click)="closeMobile.emit()" aria-label="Close navigation">
          <app-icon name="x" [size]="18" />
        </button>
      </div>

      <nav class="sb__nav" aria-label="Main navigation">
        @for (section of sections; track section.title) {
          <div class="sb__section">
            @if (!collapsed()) {
              <div class="sb__section-title">{{ section.title }}</div>
            } @else {
              <div class="sb__section-sep"></div>
            }
            @for (item of section.items; track item.route) {
              <a
                class="sb__item"
                [routerLink]="item.route"
                routerLinkActive="is-active"
                #rla="routerLinkActive"
                [attr.aria-current]="rla.isActive ? 'page' : null"
                [attr.data-tip]="collapsed() ? item.label : null"
                data-tip-pos="right"
                (click)="navigate.emit()"
              >
                <app-icon [name]="item.icon" [size]="17" [strokeWidth]="1.9" />
                @if (!collapsed()) {
                  <span class="sb__label">{{ item.label }}</span>
                }
                @if (item.badge === 'alerts' && alertCount() > 0) {
                  <span class="sb__badge mono" [attr.aria-label]="alertCount() + ' active alerts'">{{ alertCount() > 99 ? '99+' : alertCount() }}</span>
                }
              </a>
            }
          </div>
        }
      </nav>

      <div class="sb__footer">
        <button
          class="sb__collapse"
          type="button"
          (click)="toggleCollapse.emit()"
          [attr.aria-label]="collapsed() ? 'Expand sidebar' : 'Collapse sidebar'"
          [attr.data-tip]="collapsed() ? 'Expand' : null"
          data-tip-pos="right"
        >
          <app-icon [name]="collapsed() ? 'panel-left-open' : 'panel-left-close'" [size]="17" />
          @if (!collapsed()) {
            <span>Collapse</span>
          }
        </button>
        <div class="sb__user" [attr.data-tip]="collapsed() ? 'Security Analyst' : null" data-tip-pos="right">
          <span class="sb__avatar">SA</span>
          @if (!collapsed()) {
            <span class="sb__user-text">
              <span class="sb__user-name">Security Analyst</span>
              <span class="sb__user-role">SOC · Administrator</span>
            </span>
          }
        </div>
      </div>
    </div>
  `,
  styleUrl: './sidebar.scss',
})
export class SidebarComponent {
  readonly collapsed = input(false);
  readonly mobileOpen = input(false);
  readonly alertCount = input(0);
  readonly toggleCollapse = output<void>();
  readonly closeMobile = output<void>();
  readonly navigate = output<void>();

  protected readonly sections = NAV_SECTIONS;
}
