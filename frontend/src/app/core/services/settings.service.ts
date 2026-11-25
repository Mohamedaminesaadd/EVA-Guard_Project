import { Injectable, computed, effect, signal } from '@angular/core';
import { environment } from '../../../environments/environment';
import { DataSourceMode } from '../models/common.model';
import { readJson, writeJson } from '../utils/storage';

interface PersistedSettings {
  dataSource: DataSourceMode;
  pollingIntervalMs: number;
  apiBaseUrlOverride: string | null;
  notificationsEnabled: boolean;
  agentMockMode: boolean;
  sidebarCollapsed: boolean;
}

const STORAGE_KEY = 'eva-guard.settings.v1';

const DEFAULTS: PersistedSettings = {
  dataSource: 'live',
  pollingIntervalMs: environment.pollingIntervalMs,
  apiBaseUrlOverride: null,
  notificationsEnabled: true,
  agentMockMode: false,
  sidebarCollapsed: false,
};

/** User-level UI settings, persisted in localStorage. */
@Injectable({ providedIn: 'root' })
export class SettingsService {
  private readonly initial = { ...DEFAULTS, ...readJson<Partial<PersistedSettings>>(STORAGE_KEY, {}) };

  /** `live` = FastAPI only. `demo` = explicit, clearly flagged simulated data. */
  readonly dataSource = signal<DataSourceMode>(this.initial.dataSource);
  readonly pollingIntervalMs = signal<number>(Math.max(1000, this.initial.pollingIntervalMs));
  readonly apiBaseUrlOverride = signal<string | null>(this.initial.apiBaseUrlOverride);
  readonly notificationsEnabled = signal<boolean>(this.initial.notificationsEnabled);
  readonly agentMockMode = signal<boolean>(this.initial.agentMockMode);
  readonly sidebarCollapsed = signal<boolean>(this.initial.sidebarCollapsed);

  readonly isDemo = computed(() => this.dataSource() === 'demo');
  readonly apiBaseUrl = computed(() => (this.apiBaseUrlOverride() || environment.apiBaseUrl).replace(/\/+$/, ''));
  readonly apiRoot = computed(() => `${this.apiBaseUrl()}${environment.apiPrefix}`);

  constructor() {
    effect(() => {
      writeJson(STORAGE_KEY, {
        dataSource: this.dataSource(),
        pollingIntervalMs: this.pollingIntervalMs(),
        apiBaseUrlOverride: this.apiBaseUrlOverride(),
        notificationsEnabled: this.notificationsEnabled(),
        agentMockMode: this.agentMockMode(),
        sidebarCollapsed: this.sidebarCollapsed(),
      } satisfies PersistedSettings);
    });
  }

  reset(): void {
    this.dataSource.set(DEFAULTS.dataSource);
    this.pollingIntervalMs.set(DEFAULTS.pollingIntervalMs);
    this.apiBaseUrlOverride.set(null);
    this.notificationsEnabled.set(DEFAULTS.notificationsEnabled);
    this.agentMockMode.set(DEFAULTS.agentMockMode);
  }
}
