import { ChangeDetectionStrategy, Component, ElementRef, computed, inject, input, output, signal, viewChild } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { ActivatedRouteSnapshot, NavigationEnd, Router, RouterLink } from '@angular/router';
import { filter, map, startWith } from 'rxjs';
import { SecurityAlert } from '../../core/models/alert.model';
import { EthNode } from '../../core/models/node.model';
import { ClockService } from '../../core/services/clock.service';
import { ConnectionService } from '../../core/services/connection.service';
import { NotificationService } from '../../core/services/notification.service';
import { SettingsService } from '../../core/services/settings.service';
import { fmtMs, timeAgo } from '../../core/utils/format';
import { ATTACK_LABEL } from '../../core/utils/labels';
import { IconComponent } from '../../shared/components/icon/icon';
import { ALL_NAV_ITEMS } from '../nav.config';

interface Crumb {
  label: string;
  url: string | null;
}

interface SearchResult {
  kind: 'page' | 'node' | 'alert';
  label: string;
  hint: string;
  icon: string;
  url: string;
}

@Component({
  selector: 'app-topbar',
  imports: [IconComponent, RouterLink],
  changeDetection: ChangeDetectionStrategy.OnPush,
  host: {
    '(document:keydown)': 'onGlobalKey($event)',
    '(document:click)': 'onDocClick($event)',
  },
  templateUrl: './topbar.html',
  styleUrl: './topbar.scss',
})
export class TopbarComponent {
  readonly nodes = input<EthNode[]>([]);
  readonly alerts = input<SecurityAlert[]>([]);
  readonly menuClick = output<void>();

  private readonly router = inject(Router);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  protected readonly clock = inject(ClockService);
  protected readonly connection = inject(ConnectionService);
  protected readonly notifications = inject(NotificationService);
  protected readonly settings = inject(SettingsService);

  private readonly searchInput = viewChild<ElementRef<HTMLInputElement>>('searchInput');

  readonly query = signal('');
  readonly searchOpen = signal(false);
  readonly activeIndex = signal(0);
  readonly notifOpen = signal(false);

  // ---------------------------------------------------------------- breadcrumb
  private readonly url = toSignal(
    this.router.events.pipe(
      filter((e) => e instanceof NavigationEnd),
      map(() => this.router.url),
      startWith(this.router.url),
    ),
    { initialValue: this.router.url },
  );

  readonly crumbs = computed<Crumb[]>(() => {
    this.url();
    const out: Crumb[] = [{ label: 'EVA-Guard', url: '/dashboard' }];
    let snap: ActivatedRouteSnapshot | null = this.router.routerState.snapshot.root;
    let path = '';
    while (snap) {
      const seg = snap.url.map((u) => u.path).join('/');
      if (seg) path += `/${seg}`;
      const data = snap.data as { section?: string; title?: string; crumbParam?: string };
      if (snap.routeConfig && data.title) {
        if (data.section && !out.some((c) => c.label === data.section)) out.push({ label: data.section, url: null });
        const label = data.crumbParam ? (snap.paramMap.get(data.crumbParam) ?? data.title) : data.title;
        if (out[out.length - 1]?.label !== label) out.push({ label, url: path || null });
      }
      snap = snap.firstChild;
    }
    return out;
  });

  // ------------------------------------------------------------------- status
  readonly network = computed(() => {
    const nodes = this.nodes();
    if (!nodes.length) return { tone: 'offline', label: 'No nodes', title: 'No node data received yet' };
    const healthy = nodes.filter((n) => n.status === 'healthy').length;
    const critical = nodes.filter((n) => n.status === 'critical').length;
    const degraded = nodes.filter((n) => n.status === 'warning' || n.status === 'offline').length;
    const tone = critical ? 'critical' : degraded ? 'warning' : 'success';
    const label = `${healthy}/${nodes.length} healthy`;
    return { tone, label, title: `${healthy} healthy · ${critical} critical · ${degraded} degraded/offline` };
  });

  readonly backend = computed(() => {
    const s = this.connection.state();
    const latency = this.connection.latencyMs();
    switch (s) {
      case 'connected':
        return { tone: 'success', label: 'Connected', detail: latency !== null ? fmtMs(latency) : '', title: `FastAPI reachable at ${this.settings.apiBaseUrl()}` };
      case 'connecting':
        return { tone: 'info', label: 'Connecting', detail: '', title: `Contacting ${this.settings.apiBaseUrl()}` };
      case 'demo':
        return { tone: 'warning', label: 'Demo mode', detail: '', title: 'Simulated data — switch to Live in Settings' };
      default:
        return { tone: 'critical', label: 'Disconnected', detail: '', title: this.connection.lastError()?.message ?? 'Backend unreachable' };
    }
  });

  readonly dateLabel = computed(() => {
    const d = new Date(this.clock.now());
    return d.toLocaleDateString('en-GB', { weekday: 'short', day: '2-digit', month: 'short' });
  });
  readonly timeLabel = computed(() => {
    const d = new Date(this.clock.now());
    return d.toLocaleTimeString('en-GB', { hour12: false });
  });
  readonly tzLabel = computed(() => {
    const off = -new Date(this.clock.now()).getTimezoneOffset();
    const h = Math.floor(Math.abs(off) / 60);
    const m = Math.abs(off) % 60;
    return `UTC${off >= 0 ? '+' : '−'}${h}${m ? ':' + String(m).padStart(2, '0') : ''}`;
  });

  // ------------------------------------------------------------------- search
  readonly results = computed<SearchResult[]>(() => {
    const q = this.query().trim().toLowerCase();
    if (!q) return [];
    const pages: SearchResult[] = ALL_NAV_ITEMS.filter((i) => `${i.label} ${i.keywords ?? ''}`.toLowerCase().includes(q)).map((i) => ({
      kind: 'page',
      label: i.label,
      hint: 'Page',
      icon: i.icon,
      url: i.route,
    }));
    const nodes: SearchResult[] = this.nodes()
      .filter((n) => n.node_id.toLowerCase().includes(q))
      .map((n) => ({ kind: 'node', label: n.node_id, hint: `Node · ${n.status}`, icon: 'server', url: `/nodes/${encodeURIComponent(n.node_id)}` }));
    const alerts: SearchResult[] = this.alerts()
      .filter((a) => `${a.id} ${a.node_id} ${a.attack_type}`.toLowerCase().includes(q))
      .slice(0, 5)
      .map((a) => ({ kind: 'alert', label: `${ATTACK_LABEL[a.attack_type]} · ${a.node_id}`, hint: `Alert ${a.id}`, icon: 'bell-ring', url: '/alerts' }));
    return [...nodes, ...pages, ...alerts].slice(0, 9);
  });

  onSearchKey(e: KeyboardEvent): void {
    const n = this.results().length;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      this.activeIndex.set(n ? (this.activeIndex() + 1) % n : 0);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      this.activeIndex.set(n ? (this.activeIndex() - 1 + n) % n : 0);
    } else if (e.key === 'Enter') {
      const r = this.results()[this.activeIndex()];
      if (r) this.go(r);
    } else if (e.key === 'Escape') {
      this.closeSearch();
      (e.target as HTMLInputElement).blur();
    }
  }

  onQuery(v: string): void {
    this.query.set(v);
    this.activeIndex.set(0);
    this.searchOpen.set(true);
  }

  go(r: SearchResult): void {
    this.router.navigateByUrl(r.url);
    this.closeSearch();
  }

  closeSearch(): void {
    this.searchOpen.set(false);
    this.query.set('');
  }

  onGlobalKey(e: KeyboardEvent): void {
    const target = e.target as HTMLElement;
    const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) || target.isContentEditable;
    if ((e.key === 'k' && (e.metaKey || e.ctrlKey)) || (e.key === '/' && !typing)) {
      e.preventDefault();
      this.searchInput()?.nativeElement.focus();
    }
  }

  onDocClick(e: MouseEvent): void {
    const el = this.host.nativeElement;
    const t = e.target as Node;
    if (!el.querySelector('.tb__search')?.contains(t)) this.searchOpen.set(false);
    if (!el.querySelector('.tb__notif')?.contains(t)) this.notifOpen.set(false);
  }

  toggleNotifications(): void {
    this.notifOpen.set(!this.notifOpen());
  }

  ago(t: number): string {
    return timeAgo(t, this.clock.now());
  }
}
