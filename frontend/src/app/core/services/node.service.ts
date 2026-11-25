import { Injectable, inject } from '@angular/core';
import { Observable, combineLatest, map, shareReplay, tap } from 'rxjs';
import { deriveHealth, normalizeNode, normalizeNodes } from '../api/normalizers';
import { Resource } from '../models/common.model';
import { EthNode } from '../models/node.model';
import { Prediction } from '../models/prediction.model';
import { API_ENDPOINTS, ApiService } from './api.service';
import { MetricsHistoryService } from './metrics-history.service';
import { MockDataService } from './mock-data.service';
import { PollingService } from './polling.service';
import { PredictionService } from './prediction.service';

/** GET /api/v1/nodes and GET /api/v1/nodes/{node_id}. */
@Injectable({ providedIn: 'root' })
export class NodeService {
  private readonly api = inject(ApiService);
  private readonly polling = inject(PollingService);
  private readonly mock = inject(MockDataService);
  private readonly history = inject(MetricsHistoryService);
  private readonly predictions = inject(PredictionService);

  private lastRecorded: number | null = null;
  private readonly detailCache = new Map<string, Observable<Resource<EthNode>>>();

  /** GET /nodes; every fresh payload feeds the metrics history buffer. */
  private readonly rawNodes$: Observable<Resource<EthNode[]>> = this.polling
    .poll<EthNode[]>({
      live: () => this.api.get(API_ENDPOINTS.nodes),
      normalize: normalizeNodes,
      demo: () => this.mock.nodes(),
    })
    .pipe(
      tap((res) => {
        if (res.status === 'success' && res.source === 'live' && res.data && res.updatedAt !== this.lastRecorded) {
          this.lastRecorded = res.updatedAt;
          this.history.record(res.data, res.updatedAt ?? Date.now());
        }
      }),
      shareReplay({ bufferSize: 1, refCount: true }),
    );

  /**
   * Shared live list of nodes, enriched with GET /predictions/latest: when the
   * latest-prediction endpoint has a newer classification than the one embedded
   * in /nodes (or /nodes embeds none), it is used — and a UI-derived status is
   * recomputed from it.
   */
  readonly nodes$: Observable<Resource<EthNode[]>> = combineLatest([this.rawNodes$, this.predictions.latest$]).pipe(
    map(([nodes, latest]) => (nodes.data && latest.data?.length ? { ...nodes, data: mergeLatest(nodes.data, latest.data) } : nodes)),
    shareReplay({ bufferSize: 1, refCount: true }),
  );

  /** Live detail of one node (shared per id while subscribed). */
  node$(nodeId: string): Observable<Resource<EthNode>> {
    let stream = this.detailCache.get(nodeId);
    if (!stream) {
      stream = this.polling.poll<EthNode>({
        live: () => this.api.get(API_ENDPOINTS.node(nodeId)),
        normalize: (raw) => {
          const n = normalizeNode(raw, nodeId);
          if (!n) throw new Error(`Unexpected payload for node ${nodeId}`);
          return n;
        },
        demo: () => this.mock.node(nodeId),
      });
      this.detailCache.set(nodeId, stream);
    }
    return stream;
  }

  /** Convenience: ids currently known (for filters). */
  nodeIds$(): Observable<string[]> {
    return this.nodes$.pipe(map((r) => (r.data ?? []).map((n) => n.node_id)));
  }
}

function mergeLatest(nodes: EthNode[], latest: Prediction[]): EthNode[] {
  const byNode = new Map(latest.map((p) => [p.node_id, p]));
  return nodes.map((n) => {
    const p = byNode.get(n.node_id);
    if (!p || (n.latest_prediction && n.latest_prediction.timestamp >= p.timestamp)) return n;
    return {
      ...n,
      latest_prediction: p,
      status: n.statusDerived ? deriveHealth(n.metrics, n.components, p) : n.status,
    };
  });
}
