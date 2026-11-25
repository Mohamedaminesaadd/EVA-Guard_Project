import { DOCUMENT } from '@angular/common';
import { Injectable, inject } from '@angular/core';
import { toObservable } from '@angular/core/rxjs-interop';
import {
  EMPTY,
  Observable,
  Subject,
  catchError,
  combineLatest,
  distinctUntilChanged,
  exhaustMap,
  fromEvent,
  map,
  merge,
  of,
  scan,
  shareReplay,
  startWith,
  switchMap,
  timer,
} from 'rxjs';
import { ApiError, DataSourceMode, Resource, initialResource } from '../models/common.model';
import { toApiError } from './api.service';
import { SettingsService } from './settings.service';

export interface PollConfig<T> {
  /** Live request (raw JSON). */
  live: () => Observable<unknown>;
  /** Maps raw JSON to the UI model. */
  normalize: (raw: unknown) => T;
  /** Explicit demo-mode generator. Only called when Settings → Data source = Demo. */
  demo: () => T;
  /** Poll every `interval × multiplier` (default 1). */
  intervalMultiplier?: number;
  /** Optional pause switch (e.g. a "Live" toggle on a table page). */
  paused$?: Observable<boolean>;
}

type PollEvent<T> = { kind: 'ok'; data: T } | { kind: 'err'; error: ApiError };

/**
 * Creates shared, self-cleaning polled resources.
 *
 * - One HTTP request per tick, whatever the number of subscribers (shareReplay + refCount).
 * - exhaustMap: a slow request is never stacked with the next tick.
 * - Polling pauses while the tab is hidden and stops when nothing subscribes.
 * - On error, the last real payload is kept and flagged `stale`; nothing is fabricated.
 * - Switching the data source resets the state.
 */
@Injectable({ providedIn: 'root' })
export class PollingService {
  private readonly settings = inject(SettingsService);
  private readonly document = inject(DOCUMENT);
  private readonly refresh$ = new Subject<void>();

  private readonly mode$ = toObservable(this.settings.dataSource);
  private readonly interval$ = toObservable(this.settings.pollingIntervalMs);
  private readonly apiRoot$ = toObservable(this.settings.apiRoot);
  private readonly visible$ = fromEvent(this.document, 'visibilitychange').pipe(
    map(() => !this.document.hidden),
    startWith(!this.document.hidden),
    distinctUntilChanged(),
  );

  /** Forces an immediate refresh of every active resource. */
  refreshNow(): void {
    this.refresh$.next();
  }

  poll<T>(cfg: PollConfig<T>): Observable<Resource<T>> {
    const paused$ = cfg.paused$ ?? of(false);
    const multiplier = cfg.intervalMultiplier ?? 1;

    return combineLatest([this.mode$, this.apiRoot$]).pipe(
      switchMap(([mode]) => {
        const fetchOnce = (): Observable<PollEvent<T>> => {
          if (mode === 'demo') {
            try {
              return of({ kind: 'ok', data: cfg.demo() } as PollEvent<T>);
            } catch (e) {
              return of({ kind: 'err', error: toApiError(e) } as PollEvent<T>);
            }
          }
          return cfg.live().pipe(
            map((raw) => ({ kind: 'ok', data: cfg.normalize(raw) }) as PollEvent<T>),
            catchError((e: unknown) => of({ kind: 'err', error: toApiError(e) } as PollEvent<T>)),
          );
        };

        const ticks$ = combineLatest([this.interval$, this.visible$, paused$]).pipe(
          switchMap(([ms, visible, paused]) =>
            visible && !paused ? merge(timer(0, ms * multiplier), this.refresh$) : merge(this.refresh$, EMPTY),
          ),
        );

        return ticks$.pipe(
          exhaustMap(fetchOnce),
          scan((state: Resource<T>, ev: PollEvent<T>) => reduce(state, ev, mode), initialResource<T>(mode)),
          startWith(initialResource<T>(mode)),
        );
      }),
      shareReplay({ bufferSize: 1, refCount: true }),
    );
  }
}

function reduce<T>(state: Resource<T>, ev: PollEvent<T>, source: DataSourceMode): Resource<T> {
  if (ev.kind === 'ok') {
    return { status: 'success', data: ev.data, error: null, updatedAt: Date.now(), stale: false, source };
  }
  return { ...state, status: 'error', error: ev.error, stale: state.data !== null, source };
}
