import { ChangeDetectionStrategy, Component, DestroyRef, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { NavigationEnd, Router, RouterLink, RouterOutlet } from '@angular/router';
import { filter } from 'rxjs';
import { SecurityAlert } from '../../core/models/alert.model';
import { initialResource } from '../../core/models/common.model';
import { EthNode } from '../../core/models/node.model';
import { AlertService } from '../../core/services/alert.service';
import { ConnectionService } from '../../core/services/connection.service';
import { NodeService } from '../../core/services/node.service';
import { NotificationService } from '../../core/services/notification.service';
import { PollingService } from '../../core/services/polling.service';
import { SettingsService } from '../../core/services/settings.service';
import { IconComponent } from '../../shared/components/icon/icon';
import { SidebarComponent } from '../sidebar/sidebar';
import { TopbarComponent } from '../topbar/topbar';

/**
 * Persistent shell. Keeps two shared streams alive app-wide:
 * - nodes   → topbar network indicator + metrics history buffer,
 * - alerts  → sidebar badge, global search and new-threat notifications.
 */
@Component({
  selector: 'app-main-layout',
  imports: [RouterOutlet, RouterLink, SidebarComponent, TopbarComponent, IconComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  templateUrl: './main-layout.html',
  styleUrl: './main-layout.scss',
})
export class MainLayoutComponent {
  protected readonly settings = inject(SettingsService);
  protected readonly connection = inject(ConnectionService);
  protected readonly notifications = inject(NotificationService);
  private readonly polling = inject(PollingService);
  private readonly router = inject(Router);

  private readonly nodesRes = toSignal(inject(NodeService).nodes$, { initialValue: initialResource<EthNode[]>('live') });
  private readonly alertsRes = toSignal(inject(AlertService).alerts$, { initialValue: initialResource<SecurityAlert[]>('live') });

  readonly nodes = computed(() => this.nodesRes().data ?? []);
  readonly alerts = computed(() => this.alertsRes().data ?? []);
  readonly activeAlertCount = computed(() => this.alerts().filter((a) => a.status === 'active').length);

  readonly mobileOpen = signal(false);
  readonly isMobile = signal(false);
  readonly collapsed = computed(() => this.settings.sidebarCollapsed() && !this.isMobile());

  constructor() {
    const mq = window.matchMedia('(max-width: 900px)');
    this.isMobile.set(mq.matches);
    const onChange = (e: MediaQueryListEvent) => {
      this.isMobile.set(e.matches);
      if (!e.matches) this.mobileOpen.set(false);
    };
    mq.addEventListener('change', onChange);
    inject(DestroyRef).onDestroy(() => mq.removeEventListener('change', onChange));

    this.router.events.pipe(filter((e) => e instanceof NavigationEnd)).subscribe(() => this.mobileOpen.set(false));
  }

  toggleCollapse(): void {
    this.settings.sidebarCollapsed.update((v) => !v);
  }

  retry(): void {
    this.polling.refreshNow();
  }

  useDemo(): void {
    this.settings.dataSource.set('demo');
  }

  useLive(): void {
    this.settings.dataSource.set('live');
  }
}
