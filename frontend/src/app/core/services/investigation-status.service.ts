import { Injectable, signal } from '@angular/core';
import { readJson, removeKey, writeJson } from '../utils/storage';

export type InvestigationStatus = 'new' | 'investigating' | 'confirmed' | 'false_positive';

export const INVESTIGATION_LABEL: Record<InvestigationStatus, string> = {
  new: 'New',
  investigating: 'Investigating',
  confirmed: 'Confirmed',
  false_positive: 'False positive',
};

export const INVESTIGATION_TONE: Record<InvestigationStatus, string> = {
  new: 'info',
  investigating: 'warning',
  confirmed: 'critical',
  false_positive: 'offline',
};

const KEY = 'eva-guard.investigation-status.v1';

/** Investigation status of threat events — LOCAL UI-ONLY (no backend endpoint). */
@Injectable({ providedIn: 'root' })
export class InvestigationStatusService {
  private readonly _map = signal<Record<string, InvestigationStatus>>(readJson(KEY, {}));
  readonly map = this._map.asReadonly();

  get(id: string): InvestigationStatus {
    return this._map()[id] ?? 'new';
  }

  set(id: string, status: InvestigationStatus): void {
    this._map.update((m) => ({ ...m, [id]: status }));
    writeJson(KEY, this._map());
  }

  clear(): void {
    this._map.set({});
    removeKey(KEY);
  }
}

/** Short, stable display id for an event (`EVT-3FA9C1`). */
export function shortEventId(id: string): string {
  if (/^[A-Z]{2,5}-[\w-]{3,12}$/.test(id)) return id;
  let h = 2166136261;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return `EVT-${(h >>> 0).toString(16).toUpperCase().padStart(8, '0').slice(0, 6)}`;
}
