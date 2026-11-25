import { Injectable, inject } from '@angular/core';
import { Observable } from 'rxjs';
import { normalizePredictions } from '../api/normalizers';
import { Resource } from '../models/common.model';
import { Prediction } from '../models/prediction.model';
import { API_ENDPOINTS, ApiService } from './api.service';
import { MockDataService } from './mock-data.service';
import { PollingService } from './polling.service';

/** Requested history size for GET /api/v1/predictions (query param ignored if the API does not declare it). */
export const PREDICTION_HISTORY_LIMIT = 1000;

/** GET /api/v1/predictions/latest, /predictions and /predictions/{node_id}. */
@Injectable({ providedIn: 'root' })
export class PredictionService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly mock = inject(MockDataService);
  private readonly byNodeCache = new Map<string, Observable<Resource<Prediction[]>>>();

  /** Latest prediction per node (shared). */
  readonly latest$ = this.polling.poll<Prediction[]>({
    live: () => this.api.get(API_ENDPOINTS.predictionsLatest),
    normalize: (raw) => normalizePredictions(raw),
    demo: () => this.mock.latestPredictions(),
  });

  /** Prediction history (shared, polled at 2× the base interval). */
  readonly history$ = this.polling.poll<Prediction[]>({
    live: () => this.api.get(API_ENDPOINTS.predictions, { limit: PREDICTION_HISTORY_LIMIT }),
    normalize: (raw) => normalizePredictions(raw),
    demo: () => this.mock.predictionHistory(PREDICTION_HISTORY_LIMIT),
    intervalMultiplier: 2,
  });

  /** Paused-capable history stream for data-heavy tables. */
  historyWithPause$(paused$: Observable<boolean>): Observable<Resource<Prediction[]>> {
    return this.polling.poll<Prediction[]>({
      live: () => this.api.get(API_ENDPOINTS.predictions, { limit: PREDICTION_HISTORY_LIMIT }),
      normalize: (raw) => normalizePredictions(raw),
      demo: () => this.mock.predictionHistory(PREDICTION_HISTORY_LIMIT),
      paused$,
    });
  }

  forNode$(nodeId: string): Observable<Resource<Prediction[]>> {
    let s = this.byNodeCache.get(nodeId);
    if (!s) {
      s = this.polling.poll<Prediction[]>({
        // GET /predictions/{node_id} only returns the latest prediction; the history is filtered server-side.
        live: () => this.api.get(API_ENDPOINTS.predictions, { node_id: nodeId, limit: 300 }),
        normalize: (raw) => normalizePredictions(raw, nodeId),
        demo: () => this.mock.predictionHistory(300, nodeId),
      });
      this.byNodeCache.set(nodeId, s);
    }
    return s;
  }
}
