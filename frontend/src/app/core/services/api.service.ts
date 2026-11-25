import { HttpClient, HttpErrorResponse, HttpParams } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, TimeoutError, catchError, defer, map, throwError, timeout } from 'rxjs';
import { environment } from '../../../environments/environment';
import { ApiError } from '../models/common.model';
import { SettingsService } from './settings.service';

export type QueryParams = Record<string, string | number | boolean | null | undefined>;

/** REST endpoints exposed by the EVA-Guard FastAPI backend. */
export const API_ENDPOINTS = {
  health: '/health',
  status: '/status',
  nodes: '/nodes',
  node: (id: string) => `/nodes/${encodeURIComponent(id)}`,
  predictionsLatest: '/predictions/latest',
  predictions: '/predictions',
  /** Latest prediction of one node (single object, not a history). */
  predictionsByNode: (id: string) => `/predictions/${encodeURIComponent(id)}`,
  alerts: '/alerts',
  dashboardSummary: '/dashboard/summary',
  dashboardDistribution: '/dashboard/distribution',
  dashboardTimeline: '/dashboard/timeline',
  dashboardOverview: '/dashboard/overview',
  monitoringLatest: '/monitoring/latest',
  incidents: '/incidents',
  incident: (id: string) => `/incidents/${encodeURIComponent(id)}`,
  investigations: '/investigations',
  investigation: (incidentId: string) => `/investigations/${encodeURIComponent(incidentId)}`,
} as const;

export interface EndpointProbe {
  path: string;
  ok: boolean;
  status: number;
  latencyMs: number;
  message: string;
}

/**
 * Thin HttpClient wrapper: base URL from environment (or Settings override),
 * request timeout and uniform error mapping. Returns raw JSON; shaping is done
 * by core/api/normalizers.ts.
 */
@Injectable({ providedIn: 'root' })
export class ApiService {
  private readonly http = inject(HttpClient);
  private readonly settings = inject(SettingsService);

  url(path: string): string {
    return `${this.settings.apiRoot()}${path}`;
  }

  get(path: string, params?: QueryParams): Observable<unknown> {
    return defer(() => {
      let httpParams = new HttpParams();
      for (const [k, v] of Object.entries(params ?? {})) {
        if (v !== null && v !== undefined && v !== '') httpParams = httpParams.set(k, String(v));
      }
      return this.http.get<unknown>(this.url(path), { params: httpParams });
    }).pipe(
      timeout(environment.requestTimeoutMs),
      catchError((err: unknown) => throwError(() => toApiError(err, this.url(path)))),
    );
  }

  /** Calls an endpoint once and reports status/latency (Settings → endpoint diagnostics). */
  probe(path: string): Observable<EndpointProbe> {
    return defer(() => {
      const t0 = performance.now();
      return this.get(path).pipe(
        map(() => ({ path, ok: true, status: 200, latencyMs: performance.now() - t0, message: 'OK' })),
        catchError((e: ApiError) =>
          [{ path, ok: false, status: e.status, latencyMs: performance.now() - t0, message: e.message }] as EndpointProbe[],
        ),
      );
    });
  }
}

export function toApiError(err: unknown, url?: string): ApiError {
  if (err instanceof TimeoutError) {
    return { status: -1, message: `Request timed out after ${environment.requestTimeoutMs / 1000}s`, url, network: true };
  }
  if (err instanceof HttpErrorResponse) {
    if (err.status === 0) {
      return { status: 0, message: 'Backend unreachable (network error or CORS)', url, network: true };
    }
    const detail = typeof err.error === 'object' && err.error !== null && 'detail' in err.error ? String((err.error as { detail: unknown }).detail) : '';
    const label = err.status === 404 ? 'Endpoint not found' : err.statusText || 'HTTP error';
    return { status: err.status, message: `${err.status} ${label}${detail ? ` — ${detail}` : ''}`, url, network: false };
  }
  if (typeof err === 'object' && err !== null && 'status' in err && 'message' in err) {
    return err as ApiError;
  }
  return { status: -2, message: err instanceof Error ? err.message : 'Unexpected error', url, network: false };
}
