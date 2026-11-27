import { Injectable, inject, signal } from '@angular/core';
import { toObservable } from '@angular/core/rxjs-interop';
import { Observable, combineLatest, map, shareReplay, tap } from 'rxjs';
import { normalizeAlerts } from '../api/normalizers';
import { AlertStatus, SecurityAlert } from '../models/alert.model';
import { Resource } from '../models/common.model';
import { ATTACK_LABEL, SEVERITY_TONE } from '../utils/labels';
import { readJson, removeKey, writeJson } from '../utils/storage';
import { API_ENDPOINTS, ApiService } from './api.service';
import { MockDataService } from './mock-data.service';
import { NotificationService } from './notification.service';
import { PollingService } from './polling.service';
import { SettingsService } from './settings.service';

interface LocalOverride {
  status: AlertStatus;
  at: number;
}

const OVERRIDES_KEY = 'eva-guard.alert-overrides.v1';

/**
 * GET /api/v1/alerts.
 * Acknowledge / resolve are LOCAL UI-ONLY actions: the backend exposes no mutation
 * endpoint yet (expected: PATCH /api/v1/alerts/{id}). They are stored in this
 * browser and every affected alert is flagged `locallyModified`.
 */
@Injectable({ providedIn: 'root' })
export class AlertService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly mock = inject(MockDataService);
  private readonly notifications = inject(NotificationService);
  private readonly settings = inject(SettingsService);

  /** Mutation endpoint availability — flip to true once PATCH /alerts/{id} exists. */
  readonly backendMutationsSupported = false;

  private readonly overrides = signal<Record<string, LocalOverride>>(readJson(OVERRIDES_KEY, {}));
  private readonly seenIds = new Set<string>();
  private seeded = false;
  private seenSource: string | null = null;

  private readonly raw$ = this.polling.poll<SecurityAlert[]>({
    live: () => this.api.get(API_ENDPOINTS.alerts),
    normalize: normalizeAlerts,
    demo: () => this.mock.alerts(),
  });

  readonly alerts$: Observable<Resource<SecurityAlert[]>> = combineLatest([this.raw$, toObservable(this.overrides)]).pipe(
    map(([res, ov]) => (res.data ? { ...res, data: res.data.map((a) => applyOverride(a, ov[a.id])) } : res)),
    tap((res) => this.detectNew(res)),
    shareReplay({ bufferSize: 1, refCount: true }),
  );

  acknowledge(id: string): void {
    this.setLocal(id, 'acknowledged');
  }

  resolve(id: string): void {
    this.setLocal(id, 'resolved');
  }

  reopen(id: string): void {
    this.overrides.update((o) => {
      const { [id]: _removed, ...rest } = o;
      return rest;
    });
    writeJson(OVERRIDES_KEY, this.overrides());
  }

  clearLocalState(): void {
    this.overrides.set({});
    removeKey(OVERRIDES_KEY);
  }

  private setLocal(id: string, status: AlertStatus): void {
    this.overrides.update((o) => ({ ...o, [id]: { status, at: Date.now() } }));
    writeJson(OVERRIDES_KEY, this.overrides());
  }

  /** Raises a notification for each new active alert (not on first load). */
  private detectNew(res: Resource<SecurityAlert[]>): void {
    if (res.status !== 'success' || !res.data) return;
    if (res.source !== this.seenSource) {
      this.seenSource = res.source;
      this.seenIds.clear();
      this.seeded = false;
    }
    const fresh = res.data.filter((a) => !this.seenIds.has(a.id));
    fresh.forEach((a) => this.seenIds.add(a.id));
    if (!this.seeded) {
      this.seeded = true;
      return;
    }
    if (!this.settings.notificationsEnabled()) return;
    for (const a of fresh.filter((x) => x.status === 'active').slice(0, 3)) {
      this.notifications.push({
        title: `${ATTACK_LABEL[a.attack_type]} detected`,
        message: `${a.node_id}${a.confidence !== null ? ` · ${(a.confidence * 100).toFixed(1)}% confidence` : ''}`,
        tone: SEVERITY_TONE[a.severity],
        icon: a.attack_type === 'ECLIPSE' ? 'eclipse' : 'zap',
        link: '/alerts',
      });
    }
  }
}

function applyOverride(a: SecurityAlert, o: LocalOverride | undefined): SecurityAlert {
  if (!o) return a;
  // A backend "resolved" always wins over a local acknowledgement.
  if (a.backendStatus === 'resolved') return a;
  return { ...a, status: o.status, locallyModified: true };
}
