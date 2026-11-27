import { Injectable } from '@angular/core';
import { deriveHealth, severityForAttack, bucketPredictions } from '../api/normalizers';
import { SecurityAlert } from '../models/alert.model';
import { AttackClass } from '../models/common.model';
import { DashboardSummary, DistributionEntry, SystemStatus, TimelinePoint } from '../models/dashboard.model';
import { EthNode, MetricSample, NodeComponents, NodeMetrics } from '../models/node.model';
import { Prediction } from '../models/prediction.model';

/**
 * ============================================================================
 * DEMO MODE ONLY — simulated data, never used when Settings → Data source = Live.
 * ============================================================================
 * Deterministic function of time so that every view (nodes, predictions, alerts,
 * timeline) stays consistent. Node ids node-01…node-05 are placeholders.
 * The UI shows a "Demo data" banner whenever this generator is active.
 */
const NODE_IDS = ['node-01', 'node-02', 'node-03', 'node-04', 'node-05'];
const STEP_MS = 5000; // matches the Rust collection_interval (5 s)
const GENESIS = Date.UTC(2026, 0, 1);

function noise(seed: number, i: number): number {
  const x = Math.sin(seed * 12.9898 + i * 78.233) * 43758.5453;
  return x - Math.floor(x);
}

/** Smooth noise in [0,1) with the given period. */
function smooth(seed: number, t: number, periodMs: number): number {
  const p = t / periodMs;
  const i = Math.floor(p);
  const f = p - i;
  const u = f * f * (3 - 2 * f);
  return noise(seed, i) * (1 - u) + noise(seed, i + 1) * u;
}

@Injectable({ providedIn: 'root' })
export class MockDataService {
  readonly nodeIds = NODE_IDS;

  /** Ground-truth scenario of a node at time t (the "attack schedule"). */
  scenarioAt(idx: number, t: number): AttackClass {
    const minute = Math.floor(t / 60_000);
    if (idx === 2 && [3, 4, 5].includes(minute % 12)) return 'RPC_FLOODING';
    if (idx === 4 && [10, 11, 12, 13].includes(minute % 20)) return 'ECLIPSE';
    if (idx === 1 && [20, 21].includes(minute % 30)) return 'RPC_FLOODING';
    return 'NORMAL';
  }

  private isOffline(idx: number, t: number): boolean {
    const minute = Math.floor(t / 60_000);
    return idx === 3 && [40, 41].includes(minute % 45);
  }

  metricsAt(idx: number, t: number): NodeMetrics {
    if (this.isOffline(idx, t)) {
      return { rpc_available: false, cpu_usage_percent: null, memory_used_mb: null, memory_total_mb: 16384, peer_count: null };
    }
    const sc = this.scenarioAt(idx, t);
    const s = (k: number, period = 45_000) => smooth(idx * 31 + k, t, period);
    const flood = sc === 'RPC_FLOODING';
    const eclipse = sc === 'ECLIPSE';
    const headSlot = Math.floor((t - GENESIS) / 12_000);
    const blockAge = ((t / 1000) % 12) + (eclipse ? 40 + s(9) * 60 : 0);
    const peers = eclipse ? 2 + Math.round(s(3) * 2) : 22 + idx * 2 + Math.round(s(3) * 6);
    const newPeers = Math.round(noise(idx + 100, Math.floor(t / STEP_MS)) * (eclipse ? 6 : 2));
    const discPeers = Math.round(noise(idx + 200, Math.floor(t / STEP_MS)) * (eclipse ? 9 : 2));
    const rpcRate = flood ? 1600 + s(5, 15_000) * 900 : 10 + idx * 2 + s(5) * 8;
    const elapsedS = (t - GENESIS) / 1000;

    return {
      cpu_usage_percent: Math.min(99, 14 + idx * 3 + s(1) * 10 + (flood ? 52 + s(11, 10_000) * 12 : 0)),
      memory_used_mb: 3100 + idx * 420 + s(2, 120_000) * 350 + (flood ? 900 : 0),
      memory_total_mb: 16384,
      network_rx_bytes_per_sec: (380_000 + s(4) * 160_000) * (flood ? 7 : eclipse ? 0.35 : 1),
      network_tx_bytes_per_sec: (210_000 + s(6) * 90_000) * (flood ? 4.5 : eclipse ? 0.4 : 1),
      tcp_established_connections: Math.round(38 + s(7) * 12 + (flood ? 210 : 0) - (eclipse ? 22 : 0)),
      peer_count: peers,
      admin_peer_count: peers,
      new_peers: newPeers,
      disconnected_peers: discPeers,
      peer_churn_count: newPeers + discPeers,
      block_number: 21_400_000 + Math.floor((t - GENESIS) / 12_000) - (eclipse ? 4 : 0),
      latest_block_age_seconds: blockAge,
      is_synced: !eclipse,
      rpc_available: true,
      rpc_latency_ms: flood ? 140 + s(8, 15_000) * 320 : 1.2 + s(8) * 3.5,
      rpc_requests_per_sec: rpcRate,
      rpc_requests_total: Math.floor(elapsedS * (12 + idx * 2)),
      head_slot: headSlot - (eclipse ? 6 : 0),
      finalized_epoch: Math.floor(headSlot / 32) - 2,
      justified_epoch: Math.floor(headSlot / 32) - 1,
      sync_distance: eclipse ? 4 + Math.round(s(10) * 8) : 0,
      block_propagation_delay_ms: eclipse ? 2400 + s(12) * 1800 : 320 + s(12) * 380,
      attestations_total: Math.floor(headSlot / 32) * (idx + 1),
      blocks_proposed: Math.floor(headSlot / (5200 + idx * 700)),
      proposer_duties: Math.floor(headSlot / (5200 + idx * 700)) + (idx % 2),
    };
  }

  predictionAt(idx: number, t: number): Prediction | null {
    const tk = Math.floor(t / STEP_MS) * STEP_MS;
    if (this.isOffline(idx, tk)) return null;
    const truth = this.scenarioAt(idx, tk);
    const r = noise(idx * 7 + 3, tk / STEP_MS);
    // Occasional disagreement with the ground truth to mimic an imperfect model.
    let label: AttackClass = truth;
    if (truth === 'NORMAL' && r < 0.012) label = idx % 2 ? 'RPC_FLOODING' : 'ECLIPSE';
    else if (truth !== 'NORMAL' && r < 0.06) label = 'NORMAL';
    const conf = label === 'NORMAL' ? 0.86 + noise(idx, tk) * 0.13 : 0.78 + noise(idx + 9, tk) * 0.21;
    const rest = 1 - conf;
    const others = (['NORMAL', 'ECLIPSE', 'RPC_FLOODING'] as AttackClass[]).filter((c) => c !== label);
    const split = 0.3 + noise(idx + 5, tk) * 0.4;
    const probabilities = { NORMAL: 0, ECLIPSE: 0, RPC_FLOODING: 0 } as Record<AttackClass, number>;
    probabilities[label] = conf;
    probabilities[others[0]] = rest * split;
    probabilities[others[1]] = rest * (1 - split);
    const node_id = NODE_IDS[idx];
    const timestamp = new Date(tk).toISOString();
    return { id: `${node_id}@${timestamp}`, node_id, timestamp, prediction: label, confidence: conf, probabilities, scenario: truth };
  }

  private components(m: NodeMetrics, idx: number, t: number): NodeComponents {
    if (this.isOffline(idx, t)) return { geth: 'down', beacon: 'unknown', validator: 'unknown' };
    return {
      geth: m.is_synced === false ? 'syncing' : 'up',
      beacon: (m.sync_distance ?? 0) > 2 ? 'syncing' : 'up',
      validator: 'up',
    };
  }

  nodeAt(idx: number, t = Date.now()): EthNode {
    const metrics = this.metricsAt(idx, t);
    const components = this.components(metrics, idx, t);
    const latest = this.predictionAt(idx, t) ?? this.predictionAt(idx, t - 3 * 60_000);
    return {
      node_id: NODE_IDS[idx],
      status: deriveHealth(metrics, components, this.isOffline(idx, t) ? null : latest),
      statusDerived: true,
      components,
      metrics,
      latest_prediction: latest,
      last_update: new Date(Math.floor(t / STEP_MS) * STEP_MS).toISOString(),
    };
  }

  nodes(t = Date.now()): EthNode[] {
    return NODE_IDS.map((_, i) => this.nodeAt(i, t));
  }

  node(id: string): EthNode {
    const idx = NODE_IDS.indexOf(id);
    if (idx < 0) throw { status: 404, message: `404 Endpoint not found — node ${id} does not exist (demo)`, network: false };
    return this.nodeAt(idx);
  }

  latestPredictions(t = Date.now()): Prediction[] {
    return NODE_IDS.map((_, i) => this.predictionAt(i, t)).filter((p): p is Prediction => p !== null);
  }

  predictionHistory(limit = 500, nodeId?: string, t = Date.now()): Prediction[] {
    const idxs = nodeId ? [NODE_IDS.indexOf(nodeId)].filter((i) => i >= 0) : NODE_IDS.map((_, i) => i);
    const out: Prediction[] = [];
    let tk = Math.floor(t / STEP_MS) * STEP_MS;
    while (out.length < limit && t - tk < 24 * 3600_000) {
      for (const i of idxs) {
        const p = this.predictionAt(i, tk);
        if (p) out.push(p);
      }
      tk -= STEP_MS;
    }
    return out.slice(0, limit);
  }

  metricsHistory(nodeId: string, rangeMs: number, t = Date.now()): MetricSample[] {
    const idx = NODE_IDS.indexOf(nodeId);
    if (idx < 0) return [];
    const step = Math.max(STEP_MS, Math.ceil(rangeMs / 360 / STEP_MS) * STEP_MS);
    const out: MetricSample[] = [];
    for (let tk = Math.floor((t - rangeMs) / step) * step; tk <= t; tk += step) {
      out.push({ t: tk, metrics: this.metricsAt(idx, tk) });
    }
    return out;
  }

  alerts(t = Date.now()): SecurityAlert[] {
    const out: SecurityAlert[] = [];
    const nowMin = Math.floor(t / 60_000);
    for (let idx = 0; idx < NODE_IDS.length; idx++) {
      let episodeStart: number | null = null;
      for (let m = nowMin - 180; m <= nowMin + 1; m++) {
        const attack = m <= nowMin ? this.scenarioAt(idx, m * 60_000) : 'NORMAL';
        if (attack !== 'NORMAL' && episodeStart === null) episodeStart = m;
        if ((attack === 'NORMAL' || m === nowMin + 1) && episodeStart !== null) {
          const type = this.scenarioAt(idx, episodeStart * 60_000);
          const ongoing = m > nowMin;
          const endedAgo = nowMin - m;
          const status = ongoing ? 'active' : endedAgo < 8 ? 'acknowledged' : 'resolved';
          const conf = 0.82 + noise(idx, episodeStart) * 0.16;
          out.push({
            id: `ALT-${String(episodeStart % 100000).padStart(5, '0')}${idx + 1}`,
            node_id: NODE_IDS[idx],
            attack_type: type,
            severity: severityForAttack(type),
            confidence: conf,
            timestamp: new Date(episodeStart * 60_000 + 7000).toISOString(),
            backendStatus: status,
            status,
            locallyModified: false,
            message:
              type === 'ECLIPSE'
                ? `Peer set collapsed and head lag increased on ${NODE_IDS[idx]}`
                : `RPC request rate far above baseline on ${NODE_IDS[idx]}`,
          });
          episodeStart = null;
        }
      }
    }
    return out.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
  }

  summary(t = Date.now()): DashboardSummary {
    const nodes = this.nodes(t);
    const elapsedToday = t - new Date(new Date(t).toDateString()).getTime();
    return {
      total_nodes: nodes.length,
      healthy_nodes: nodes.filter((n) => n.status === 'healthy').length,
      active_attacks: nodes.filter((n) => n.latest_prediction && n.latest_prediction.prediction !== 'NORMAL' && n.status !== 'offline').length,
      total_alerts: this.alerts(t).filter((a) => a.status === 'active').length,
      model_accuracy: null,
      total_predictions: Math.floor(elapsedToday / STEP_MS) * nodes.length,
      last_prediction_at: new Date(Math.floor(t / STEP_MS) * STEP_MS).toISOString(),
    };
  }

  distribution(t = Date.now()): DistributionEntry[] {
    const preds = this.predictionHistory(720 * NODE_IDS.length, undefined, t); // last hour
    return (['NORMAL', 'ECLIPSE', 'RPC_FLOODING'] as AttackClass[]).map((label) => ({
      label,
      count: preds.filter((p) => p.prediction === label).length,
    }));
  }

  timeline(t = Date.now()): TimelinePoint[] {
    const preds = this.predictionHistory(360 * NODE_IDS.length, undefined, t); // last 30 min
    return bucketPredictions(preds, 60_000);
  }

  status(): SystemStatus {
    return {
      status: 'demo',
      model_loaded: true,
      model_name: 'LogisticRegression',
      model_version: 'demo',
      api_version: 'demo',
      uptime_seconds: (Date.now() % 86_400_000) / 1000,
      nodes_monitored: NODE_IDS.length,
      raw: null,
    };
  }
}
