import { Injectable, computed, inject, signal } from '@angular/core';
import { toSignal } from '@angular/core/rxjs-interop';
import { defer, tap } from 'rxjs';
import { normalizeHealth, normalizeStatus } from '../api/normalizers';
import { ConnectionState, initialResource } from '../models/common.model';
import { HealthResponse, SystemStatus } from '../models/dashboard.model';
import { API_ENDPOINTS, ApiService } from './api.service';
import { MockDataService } from './mock-data.service';
import { PollingService } from './polling.service';
import { SettingsService } from './settings.service';

/** FastAPI connectivity (GET /health every tick) and system status (GET /status, slower). */
@Injectable({ providedIn: 'root' })
export class ConnectionService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly mock = inject(MockDataService);
  private readonly settings = inject(SettingsService);

  readonly latencyMs = signal<number | null>(null);

  readonly health$ = this.polling.poll<HealthResponse>({
    live: () =>
      defer(() => {
        const t0 = performance.now();
        return this.api.get(API_ENDPOINTS.health).pipe(tap(() => this.latencyMs.set(performance.now() - t0)));
      }),
    normalize: normalizeHealth,
    demo: () => ({ status: 'demo', ok: true, raw: null }),
  });

  readonly status$ = this.polling.poll<SystemStatus>({
    live: () => this.api.get(API_ENDPOINTS.status),
    normalize: normalizeStatus,
    demo: () => this.mock.status(),
    intervalMultiplier: 5,
  });

  /** Always-on: the topbar indicator is visible on every page. */
  readonly health = toSignal(this.health$, { initialValue: initialResource<HealthResponse>('live') });
  readonly status = toSignal(this.status$, { initialValue: initialResource<SystemStatus>('live') });

  readonly state = computed<ConnectionState>(() => {
    if (this.settings.isDemo()) return 'demo';
    const h = this.health();
    if (h.status === 'success') return h.data?.ok ? 'connected' : 'disconnected';
    if (h.status === 'error') return 'disconnected';
    return 'connecting';
  });

  readonly isLive = computed(() => this.state() === 'connected');
  readonly lastError = computed(() => this.health().error);
}
