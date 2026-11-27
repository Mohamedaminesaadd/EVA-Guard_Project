/**
 * ============================================================================
 * Response normalizers — the ONLY place that knows the FastAPI payload shapes.
 * ============================================================================
 *
 * The exact Pydantic schemas of the EVA-Guard FastAPI backend were not available
 * when this UI was written, so each normalizer accepts the common variants
 * (bare arrays vs `{items|data|nodes|...}` wrappers, alternative key names,
 * nested `metrics` / `geth` / `beacon` / `validator` groups, 0..1 vs 0..100
 * confidences, epoch vs ISO timestamps).
 *
 * If a field shows "—" while your API returns it, add its key to the alias
 * lists below. Nothing here fabricates values: a field that is absent stays
 * `null` and the UI renders it as "not reported".
 */
import { AlertStatus, SecurityAlert } from '../models/alert.model';
import { ATTACK_CLASSES, AttackClass, ComponentState, NodeHealth, PredictionLabel, Severity } from '../models/common.model';
import {
  DashboardSummary,
  DistributionEntry,
  HealthResponse,
  SystemStatus,
  TimelinePoint,
} from '../models/dashboard.model';
import { EthNode, NodeComponents, NodeMetricKey, NodeMetrics } from '../models/node.model';
import { ClassProbabilities, Prediction } from '../models/prediction.model';
import {
  AgentAssessment,
  EvidenceItem,
  Incident,
  InvestigationReport,
  InvestigationState,
  RagSource,
  Verdict,
  WorkflowStep,
} from '../models/incident.model';

type Obj = Record<string, unknown>;

// ---------------------------------------------------------------------------
// Primitive helpers
// ---------------------------------------------------------------------------
export function isObj(v: unknown): v is Obj {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function pick(o: Obj, ...keys: string[]): unknown {
  for (const k of keys) {
    const v = o[k];
    if (v !== undefined && v !== null) return v;
  }
  return undefined;
}

export function num(v: unknown): number | null {
  if (typeof v === 'number') return Number.isFinite(v) ? v : null;
  if (typeof v === 'string' && v.trim() !== '' && !Number.isNaN(Number(v))) return Number(v);
  if (isObj(v)) return num(pick(v, 'value', 'count', 'total', 'current'));
  return null;
}

function str(v: unknown): string | null {
  if (typeof v === 'string') return v.trim() === '' ? null : v;
  if (typeof v === 'number') return String(v);
  return null;
}

function bool(v: unknown): boolean | null {
  if (typeof v === 'boolean') return v;
  if (typeof v === 'number') return v !== 0;
  if (typeof v === 'string') {
    const s = v.toLowerCase();
    if (['true', 'yes', '1', 'up', 'ok'].includes(s)) return true;
    if (['false', 'no', '0', 'down'].includes(s)) return false;
  }
  return null;
}

/**
 * Converts epoch seconds / epoch ms / ISO strings to an ISO string.
 * Naive ISO strings (no timezone, typical of Python `datetime.utcnow().isoformat()`)
 * are interpreted as UTC.
 */
export function toIso(v: unknown): string | null {
  if (v === undefined || v === null || v === '') return null;
  if (typeof v === 'number') {
    const ms = v < 1e12 ? v * 1000 : v;
    const d = new Date(ms);
    return Number.isNaN(d.getTime()) ? null : d.toISOString();
  }
  if (typeof v === 'string') {
    let s = v.trim();
    if (/^\d+(\.\d+)?$/.test(s)) return toIso(Number(s));
    const hasTz = /([zZ]|[+-]\d{2}:?\d{2})$/.test(s);
    if (/^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}/.test(s) && !hasTz) s = s.replace(' ', 'T') + 'Z';
    const d = new Date(s);
    return Number.isNaN(d.getTime()) ? null : d.toISOString();
  }
  return null;
}

/** Extracts a list from a bare array or a wrapper object. */
export function unwrapList(payload: unknown, ...preferredKeys: string[]): unknown[] {
  if (Array.isArray(payload)) return payload;
  if (!isObj(payload)) return [];
  for (const k of [...preferredKeys, 'items', 'data', 'results', 'records', 'rows']) {
    const v = payload[k];
    if (Array.isArray(v)) return v;
    if (isObj(v)) {
      const nested = unwrapList(v, ...preferredKeys);
      if (nested.length) return nested;
    }
  }
  // Dictionary keyed by id: { "node-01": {...}, "node-02": {...} }
  const values = Object.entries(payload);
  if (values.length && values.every(([, v]) => isObj(v))) {
    return values.map(([k, v]) => ({ __key: k, ...(v as Obj) }));
  }
  return [];
}

function unwrapObj(payload: unknown, ...keys: string[]): Obj {
  if (!isObj(payload)) return {};
  for (const k of keys) {
    const v = payload[k];
    if (isObj(v)) return v;
  }
  return payload;
}

// ---------------------------------------------------------------------------
// Labels
// ---------------------------------------------------------------------------
export function normalizeAttack(v: unknown): PredictionLabel {
  const s = str(v);
  if (!s) return 'UNKNOWN';
  const k = s.toUpperCase().replace(/[\s-]+/g, '_');
  if (k === 'NORMAL' || k === 'BENIGN' || k === 'HEALTHY') return 'NORMAL';
  if (k.startsWith('ECLIPSE')) return 'ECLIPSE';
  if (k === 'RPC_FLOODING' || k === 'RPCFLOODING' || k === 'RPC_FLOOD' || k === 'FLOODING') return 'RPC_FLOODING';
  return 'UNKNOWN';
}

/** Confidence in 0..1 (accepts percentages). */
export function normalizeConfidence(v: unknown): number | null {
  const n = num(v);
  if (n === null) return null;
  if (n > 1 && n <= 100) return n / 100;
  if (n < 0 || n > 100) return null;
  return n;
}

export function severityForAttack(label: PredictionLabel): Severity {
  switch (label) {
    case 'ECLIPSE':
      return 'critical';
    case 'RPC_FLOODING':
      return 'high';
    case 'NORMAL':
      return 'healthy';
    default:
      return 'medium';
  }
}

function normalizeSeverity(v: unknown, fallback: PredictionLabel): Severity {
  const s = str(v)?.toLowerCase();
  switch (s) {
    case 'critical':
    case 'crit':
    case 'p1':
      return 'critical';
    case 'high':
    case 'error':
    case 'p2':
      return 'high';
    case 'medium':
    case 'warning':
    case 'warn':
    case 'p3':
      return 'medium';
    case 'low':
    case 'info':
    case 'p4':
      return 'low';
    case 'healthy':
    case 'normal':
    case 'none':
      return 'healthy';
    default:
      return severityForAttack(fallback);
  }
}

// ---------------------------------------------------------------------------
// Predictions
// ---------------------------------------------------------------------------
function normalizeProbabilities(raw: Obj): ClassProbabilities {
  const out: ClassProbabilities = { NORMAL: null, ECLIPSE: null, RPC_FLOODING: null };
  const p = pick(raw, 'probabilities', 'probas', 'proba', 'class_probabilities', 'probability_distribution', 'scores');

  if (isObj(p)) {
    for (const [k, v] of Object.entries(p)) {
      const label = normalizeAttack(k);
      if (label !== 'UNKNOWN') out[label] = normalizeConfidence(v);
    }
  } else if (Array.isArray(p)) {
    const classes = pick(raw, 'classes', 'class_names', 'labels');
    if (Array.isArray(classes) && classes.length === p.length) {
      classes.forEach((c, i) => {
        const label = normalizeAttack(c);
        if (label !== 'UNKNOWN') out[label] = normalizeConfidence(p[i]);
      });
    } else if (p.every((x) => isObj(x))) {
      for (const item of p as Obj[]) {
        const label = normalizeAttack(pick(item, 'label', 'class', 'name'));
        if (label !== 'UNKNOWN') out[label] = normalizeConfidence(pick(item, 'probability', 'value', 'score'));
      }
    }
  }

  // Flat keys: prob_normal, normal_probability, p_eclipse, proba_RPC_FLOODING …
  for (const cls of ATTACK_CLASSES) {
    if (out[cls] !== null) continue;
    const lc = cls.toLowerCase();
    const v = pick(raw, `prob_${lc}`, `${lc}_probability`, `${lc}_prob`, `p_${lc}`, `proba_${lc}`, `proba_${cls}`, `prob_${cls}`);
    if (v !== undefined) out[cls] = normalizeConfidence(v);
  }
  return out;
}

export function normalizePrediction(raw: unknown, fallbackNodeId?: string): Prediction | null {
  if (!isObj(raw)) return null;
  const node_id = str(pick(raw, 'node_id', 'nodeId', 'node', 'node_name', '__key')) ?? fallbackNodeId ?? null;
  const timestamp = toIso(pick(raw, 'timestamp', 'predicted_at', 'created_at', 'time', 'ts', 'datetime'));
  const labelRaw = pick(raw, 'prediction', 'predicted_class', 'predicted_label', 'label', 'class', 'attack_type', 'classification', 'result');
  if (!node_id || labelRaw === undefined) return null;

  const prediction = normalizeAttack(isObj(labelRaw) ? pick(labelRaw, 'label', 'class', 'name') : labelRaw);
  const probabilities = normalizeProbabilities(raw);
  let confidence = normalizeConfidence(pick(raw, 'confidence', 'score', 'probability', 'max_probability', 'ml_confidence'));
  if (confidence === null && prediction !== 'UNKNOWN') confidence = probabilities[prediction];

  const ts = timestamp ?? new Date(0).toISOString();
  return {
    id: str(pick(raw, 'id', 'prediction_id', 'uuid')) ?? `${node_id}@${ts}`,
    node_id,
    timestamp: ts,
    prediction,
    confidence,
    probabilities,
    scenario: str(pick(raw, 'scenario', 'scenario_name', 'dataset')),
  };
}

export function normalizePredictions(payload: unknown, fallbackNodeId?: string): Prediction[] {
  return unwrapList(payload, 'predictions', 'latest', 'history')
    .map((r) => normalizePrediction(r, fallbackNodeId))
    .filter((p): p is Prediction => p !== null)
    .sort((a, b) => b.timestamp.localeCompare(a.timestamp));
}

// ---------------------------------------------------------------------------
// Nodes
// ---------------------------------------------------------------------------
const METRIC_ALIASES: Record<NodeMetricKey, string[]> = {
  cpu_usage_percent: ['cpu_usage_percent', 'cpu_percent', 'cpu_usage', 'cpu'],
  memory_used_mb: ['memory_used_mb', 'memory_mb', 'mem_used_mb', 'memory_usage_mb', 'memory_used'],
  memory_total_mb: ['memory_total_mb', 'mem_total_mb', 'memory_total'],
  network_rx_bytes_per_sec: ['network_rx_bytes_per_sec', 'rx_bytes_per_sec', 'network_rx', 'net_rx_bps'],
  network_tx_bytes_per_sec: ['network_tx_bytes_per_sec', 'tx_bytes_per_sec', 'network_tx', 'net_tx_bps'],
  tcp_established_connections: ['tcp_established_connections', 'tcp_established', 'tcp_connections'],
  peer_count: ['peer_count', 'peers', 'net_peer_count', 'geth_peer_count'],
  admin_peer_count: ['admin_peer_count', 'admin_peers'],
  new_peers: ['new_peers', 'peers_new'],
  disconnected_peers: ['disconnected_peers', 'peers_disconnected'],
  peer_churn_count: ['peer_churn_count', 'peer_churn', 'churn'],
  block_number: ['block_number', 'latest_block', 'block_height', 'head_block'],
  latest_block_age_seconds: ['latest_block_age_seconds', 'latest_block_age_secs', 'latest_block_age', 'block_age_seconds', 'block_age'],
  is_synced: ['is_synced', 'synced'],
  rpc_available: ['rpc_available', 'rpc_up'],
  rpc_latency_ms: ['rpc_latency_ms', 'rpc_latency', 'latency_ms'],
  rpc_requests_total: ['rpc_requests_total', 'rpc_requests', 'rpc_request_count'],
  rpc_requests_per_sec: ['rpc_requests_per_sec', 'rpc_rate', 'rpc_requests_rate', 'rpc_rps'],
  head_slot: ['head_slot', 'beacon_head_slot', 'slot'],
  finalized_epoch: ['finalized_epoch'],
  justified_epoch: ['justified_epoch', 'current_justified_epoch'],
  sync_distance: ['sync_distance'],
  block_propagation_delay_ms: ['block_propagation_delay_ms', 'propagation_delay_ms', 'block_propagation_delay'],
  attestations_total: ['attestations_total', 'attestations', 'attestation_count'],
  blocks_proposed: ['blocks_proposed', 'blocks_proposed_total', 'proposed_blocks'],
  proposer_duties: ['proposer_duties', 'proposer_duty_count'],
};

const BOOLEAN_METRICS: NodeMetricKey[] = ['is_synced', 'rpc_available'];
const METRIC_GROUPS = ['metrics', 'latest_metrics', 'observation', 'latest_observation', 'system', 'network', 'geth', 'beacon', 'validator', 'rpc', 'p2p', 'blockchain'];

/** Flattens nested metric groups so aliases can be resolved on a single object. */
function flattenNode(raw: Obj): Obj {
  const flat: Obj = { ...raw };
  for (const group of METRIC_GROUPS) {
    const g = raw[group];
    if (!isObj(g)) continue;
    for (const [k, v] of Object.entries(g)) {
      if (isObj(v)) {
        for (const [k2, v2] of Object.entries(v)) {
          flat[k2] ??= v2;
        }
      } else {
        flat[k] ??= v;
        flat[`${group}_${k}`] ??= v;
      }
    }
  }
  return flat;
}

export function normalizeMetrics(raw: unknown): NodeMetrics {
  if (!isObj(raw)) return {};
  const flat = flattenNode(raw);
  const out: NodeMetrics = {};
  for (const key of Object.keys(METRIC_ALIASES) as NodeMetricKey[]) {
    const aliases = METRIC_ALIASES[key];
    const candidates = aliases.flatMap((a) => [a, `geth_${a}`, `beacon_${a}`, `validator_${a}`, `system_${a}`]);
    const v = pick(flat, ...candidates);
    if (BOOLEAN_METRICS.includes(key)) {
      (out as Record<string, unknown>)[key] = bool(v);
    } else {
      (out as Record<string, unknown>)[key] = num(v);
    }
  }
  return out;
}

function componentState(v: unknown): ComponentState | null {
  const s = str(isObj(v) ? pick(v, 'status', 'state') : v)?.toLowerCase();
  if (!s) {
    const b = bool(v);
    return b === null ? null : b ? 'up' : 'down';
  }
  if (['up', 'running', 'ok', 'healthy', 'active', 'online', 'synced', 'ready'].includes(s)) return 'up';
  if (['syncing', 'starting', 'catching_up'].includes(s)) return 'syncing';
  if (['down', 'stopped', 'offline', 'error', 'unreachable', 'failed', 'dead'].includes(s)) return 'down';
  return 'unknown';
}

function deriveComponents(raw: Obj, m: NodeMetrics): NodeComponents {
  const comps = isObj(raw['components']) ? (raw['components'] as Obj) : {};
  const geth =
    componentState(pick(comps, 'geth', 'execution', 'el')) ??
    componentState(pick(raw, 'geth_status', 'execution_status')) ??
    componentState(isObj(raw['geth']) ? (raw['geth'] as Obj)['status'] : undefined) ??
    (m.rpc_available === false ? 'down' : m.rpc_available === true ? (m.is_synced === false ? 'syncing' : 'up') : 'unknown');
  const beacon =
    componentState(pick(comps, 'beacon', 'consensus', 'cl', 'lighthouse')) ??
    componentState(pick(raw, 'beacon_status', 'consensus_status')) ??
    componentState(isObj(raw['beacon']) ? (raw['beacon'] as Obj)['status'] : undefined) ??
    (m.head_slot != null ? ((m.sync_distance ?? 0) > 2 ? 'syncing' : 'up') : 'unknown');
  const validator =
    componentState(pick(comps, 'validator', 'vc')) ??
    componentState(pick(raw, 'validator_status')) ??
    componentState(isObj(raw['validator']) ? (raw['validator'] as Obj)['status'] : undefined) ??
    (m.attestations_total != null ? 'up' : 'unknown');
  return { geth, beacon, validator };
}

function mapHealth(v: unknown): NodeHealth | null {
  const s = str(v)?.toLowerCase();
  if (!s) return null;
  if (['healthy', 'ok', 'normal', 'up', 'online', 'running'].includes(s)) return 'healthy';
  if (['warning', 'warn', 'degraded', 'syncing'].includes(s)) return 'warning';
  if (['critical', 'attack', 'under_attack', 'compromised', 'alert'].includes(s)) return 'critical';
  if (['offline', 'down', 'unreachable', 'stopped'].includes(s)) return 'offline';
  return null;
}

/**
 * UI rule used ONLY when the backend does not send a status:
 * - offline  : Geth RPC unreachable
 * - critical : latest ML prediction is an attack (ECLIPSE / RPC_FLOODING)
 * - warning  : CPU ≥ 90 %, memory ≥ 90 %, beacon sync distance > 32 slots or RPC latency > 1 s
 * - healthy  : otherwise, when at least one metric or prediction is known
 */
export function deriveHealth(m: NodeMetrics, components: NodeComponents, p: Prediction | null): NodeHealth {
  if (components.geth === 'down' || m.rpc_available === false) return 'offline';
  if (p && (p.prediction === 'ECLIPSE' || p.prediction === 'RPC_FLOODING')) return 'critical';
  const memPct = m.memory_used_mb != null && m.memory_total_mb ? (m.memory_used_mb / m.memory_total_mb) * 100 : null;
  if ((m.cpu_usage_percent ?? 0) >= 90 || (memPct ?? 0) >= 90 || (m.sync_distance ?? 0) > 32 || (m.rpc_latency_ms ?? 0) > 1000) {
    return 'warning';
  }
  const known = Object.values(m).some((v) => v !== null && v !== undefined) || p !== null;
  return known ? 'healthy' : 'unknown';
}

export function normalizeNode(raw: unknown, fallbackId?: string): EthNode | null {
  if (!isObj(raw)) return null;
  const node_id = str(pick(raw, 'node_id', 'id', 'nodeId', 'name', 'node', '__key')) ?? fallbackId ?? null;
  if (!node_id) return null;

  const metrics = normalizeMetrics(raw);
  const components = deriveComponents(raw, metrics);

  let latest: Prediction | null = null;
  const lp = pick(raw, 'latest_prediction', 'prediction', 'ml_prediction', 'classification', 'current_prediction');
  if (isObj(lp)) {
    latest = normalizePrediction({ node_id, ...lp }, node_id);
  } else if (typeof lp === 'string') {
    latest = normalizePrediction(
      { node_id, prediction: lp, confidence: pick(raw, 'confidence', 'ml_confidence'), timestamp: pick(raw, 'prediction_timestamp', 'timestamp') },
      node_id,
    );
  }

  const groups = METRIC_GROUPS.map((g) => raw[g]).filter(isObj);
  const lastUpdate =
    toIso(pick(raw, 'last_update', 'last_updated', 'updated_at', 'last_seen', 'timestamp')) ??
    toIso(groups.map((g) => pick(g, 'timestamp', 'collected_at')).find((v) => v !== undefined));

  const backendStatus = mapHealth(pick(raw, 'status', 'health', 'state', 'node_status'));
  return {
    node_id,
    status: backendStatus ?? deriveHealth(metrics, components, latest),
    statusDerived: backendStatus === null,
    components,
    metrics,
    latest_prediction: latest,
    last_update: lastUpdate,
  };
}

export function normalizeNodes(payload: unknown): EthNode[] {
  return unwrapList(payload, 'nodes')
    .map((r) => normalizeNode(r))
    .filter((n): n is EthNode => n !== null)
    .sort((a, b) => a.node_id.localeCompare(b.node_id, undefined, { numeric: true }));
}

// ---------------------------------------------------------------------------
// Alerts
// ---------------------------------------------------------------------------
function mapAlertStatus(raw: Obj): AlertStatus {
  const s = str(pick(raw, 'status', 'state'))?.toLowerCase();
  if (s) {
    if (['resolved', 'closed', 'cleared', 'done'].includes(s)) return 'resolved';
    if (['acknowledged', 'ack', 'acked', 'investigating', 'in_progress'].includes(s)) return 'acknowledged';
    return 'active';
  }
  if (bool(raw['resolved']) === true) return 'resolved';
  if (bool(raw['acknowledged']) === true) return 'acknowledged';
  return 'active';
}

export function normalizeAlert(raw: unknown): SecurityAlert | null {
  if (!isObj(raw)) return null;
  const node_id = str(pick(raw, 'node_id', 'node', 'nodeId', 'source'));
  if (!node_id) return null;
  const attack_type = normalizeAttack(pick(raw, 'attack_type', 'prediction', 'type', 'attack', 'threat', 'label', 'category'));
  const timestamp = toIso(pick(raw, 'timestamp', 'created_at', 'detected_at', 'time', 'ts')) ?? new Date(0).toISOString();
  const status = mapAlertStatus(raw);
  return {
    id: str(pick(raw, 'id', 'alert_id', 'uuid')) ?? `${node_id}-${attack_type}-${timestamp}`,
    node_id,
    attack_type,
    severity: normalizeSeverity(pick(raw, 'severity', 'level', 'priority'), attack_type),
    confidence: normalizeConfidence(pick(raw, 'confidence', 'score', 'probability')),
    timestamp,
    backendStatus: status,
    status,
    locallyModified: false,
    message: str(pick(raw, 'message', 'description', 'title', 'details', 'summary')),
  };
}

export function normalizeAlerts(payload: unknown): SecurityAlert[] {
  return unwrapList(payload, 'alerts')
    .map(normalizeAlert)
    .filter((a): a is SecurityAlert => a !== null)
    .sort((a, b) => b.timestamp.localeCompare(a.timestamp));
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------
export function normalizeSummary(payload: unknown): DashboardSummary {
  const o = unwrapObj(payload, 'summary', 'data');
  const lastPred = pick(o, 'last_prediction_at', 'latest_prediction_at', 'last_prediction');
  return {
    total_nodes: num(pick(o, 'total_nodes', 'nodes_total', 'node_count', 'nodes')),
    healthy_nodes: num(pick(o, 'healthy_nodes', 'nodes_healthy', 'normal_nodes', 'healthy')),
    active_attacks: num(pick(o, 'active_attacks', 'attacks_active', 'attack_nodes', 'active_threats', 'attacks', 'threats')),
    total_alerts: num(pick(o, 'total_alerts', 'security_alerts', 'alerts_count', 'alert_count', 'active_alerts', 'alerts')),
    model_accuracy: normalizeConfidence(pick(o, 'model_accuracy', 'accuracy')),
    total_predictions: num(pick(o, 'total_predictions', 'prediction_count', 'predictions_total', 'predictions')),
    last_prediction_at: isObj(lastPred) ? toIso(pick(lastPred, 'timestamp')) : toIso(lastPred),
  };
}

export function normalizeDistribution(payload: unknown): DistributionEntry[] {
  const counts: Record<AttackClass, number> = { NORMAL: 0, ECLIPSE: 0, RPC_FLOODING: 0 };
  const list = Array.isArray(payload) ? payload : isObj(payload) ? unwrapList(payload, 'distribution', 'classes') : [];
  const listIsEntries = list.length > 0 && list.every((x) => isObj(x) && pick(x as Obj, 'label', 'class', 'attack_type', 'prediction', 'name') !== undefined);

  if (listIsEntries) {
    for (const item of list as Obj[]) {
      const label = normalizeAttack(pick(item, 'label', 'class', 'attack_type', 'prediction', 'name'));
      if (label !== 'UNKNOWN') counts[label] += num(pick(item, 'count', 'value', 'total', 'n')) ?? 0;
    }
  } else {
    const o = unwrapObj(payload, 'distribution', 'counts', 'data');
    for (const [k, v] of Object.entries(o)) {
      const label = normalizeAttack(k);
      if (label !== 'UNKNOWN') counts[label] += num(v) ?? 0;
    }
  }
  return ATTACK_CLASSES.map((label) => ({ label, count: counts[label] }));
}

/** Buckets predictions into a timeline (used when the API returns raw predictions). */
export function bucketPredictions(preds: Prediction[], bucketMs = 60_000): TimelinePoint[] {
  const buckets = new Map<number, { n: number; e: number; r: number; confSum: number; confN: number }>();
  for (const p of preds) {
    const t = Math.floor(new Date(p.timestamp).getTime() / bucketMs) * bucketMs;
    const b = buckets.get(t) ?? { n: 0, e: 0, r: 0, confSum: 0, confN: 0 };
    if (p.prediction === 'NORMAL') b.n++;
    else if (p.prediction === 'ECLIPSE') b.e++;
    else if (p.prediction === 'RPC_FLOODING') b.r++;
    if (p.confidence !== null) {
      b.confSum += p.confidence;
      b.confN++;
    }
    buckets.set(t, b);
  }
  return [...buckets.entries()]
    .sort(([a], [b]) => a - b)
    .map(([t, b]) => ({
      timestamp: new Date(t).toISOString(),
      normal: b.n,
      eclipse: b.e,
      rpc_flooding: b.r,
      avg_confidence: b.confN ? b.confSum / b.confN : null,
    }));
}

export function normalizeTimeline(payload: unknown): TimelinePoint[] {
  // Shape { labels: [...], series: { NORMAL: [...], ... } }
  if (isObj(payload) && Array.isArray(payload['labels']) && isObj(payload['series'])) {
    const labels = payload['labels'] as unknown[];
    const series = payload['series'] as Obj;
    const get = (cls: string, i: number) => {
      const arr = pick(series, cls, cls.toLowerCase());
      return Array.isArray(arr) ? num(arr[i]) ?? 0 : 0;
    };
    const conf = pick(payload, 'confidence', 'avg_confidence');
    return labels.map((l, i) => ({
      timestamp: toIso(l) ?? String(l),
      normal: get('NORMAL', i),
      eclipse: get('ECLIPSE', i),
      rpc_flooding: get('RPC_FLOODING', i),
      avg_confidence: Array.isArray(conf) ? normalizeConfidence(conf[i]) : null,
    }));
  }

  const list = unwrapList(payload, 'timeline', 'points', 'buckets');
  if (!list.length) return [];

  const first = list[0];
  const looksLikePredictions =
    isObj(first) &&
    pick(first, 'prediction', 'predicted_class', 'label') !== undefined &&
    pick(first, 'normal', 'NORMAL', 'counts') === undefined;
  if (looksLikePredictions) return bucketPredictions(normalizePredictions(list));

  return list
    .filter(isObj)
    .map((item) => {
      const c = isObj(item['counts']) ? (item['counts'] as Obj) : item;
      return {
        timestamp: toIso(pick(item, 'timestamp', 'time', 'bucket', 't', 'date', 'minute')) ?? '',
        normal: num(pick(c, 'normal', 'NORMAL')) ?? 0,
        eclipse: num(pick(c, 'eclipse', 'ECLIPSE')) ?? 0,
        rpc_flooding: num(pick(c, 'rpc_flooding', 'RPC_FLOODING', 'rpc')) ?? 0,
        avg_confidence: normalizeConfidence(pick(item, 'avg_confidence', 'mean_confidence', 'confidence')),
      };
    })
    .filter((p) => p.timestamp !== '')
    .sort((a, b) => a.timestamp.localeCompare(b.timestamp));
}

// ---------------------------------------------------------------------------
// Health / status
// ---------------------------------------------------------------------------
export function normalizeHealth(payload: unknown): HealthResponse {
  const o = isObj(payload) ? payload : {};
  const status = str(pick(o, 'status', 'state', 'health')) ?? 'ok';
  const bad = ['error', 'down', 'unhealthy', 'fail', 'failed'].includes(status.toLowerCase());
  return { status, ok: !bad, raw: payload };
}

export function normalizeStatus(payload: unknown): SystemStatus {
  const o = unwrapObj(payload, 'status_info', 'data');
  const model = isObj(o['model']) ? (o['model'] as Obj) : {};
  const nodes = pick(o, 'nodes_monitored', 'monitored_nodes', 'node_count', 'total_nodes', 'nodes_detected', 'nodes');
  return {
    status: str(pick(o, 'status', 'state')),
    model_loaded: bool(pick(o, 'model_loaded', 'model_ready') ?? pick(model, 'loaded', 'ready')),
    model_name: str(pick(o, 'model_name') ?? pick(model, 'name', 'type')),
    model_version: str(pick(o, 'model_version') ?? pick(model, 'version')),
    api_version: str(pick(o, 'version', 'api_version')),
    uptime_seconds: num(pick(o, 'uptime_seconds', 'uptime')),
    nodes_monitored: Array.isArray(nodes) ? nodes.length : num(nodes),
    raw: payload,
  };
}

// ---------------------------------------------------------------------------
// Incidents & AI investigations (agentic/schemas, ml/backend/main.py)
// ---------------------------------------------------------------------------
const VERDICTS: Verdict[] = ['CONFIRMED', 'SUSPECTED', 'REJECTED', 'INCONCLUSIVE'];
const INVESTIGATION_STATES: InvestigationState[] = ['NOT_STARTED', 'RUNNING', 'COMPLETED', 'PARTIAL', 'FAILED'];

export function normalizeVerdict(v: unknown): Verdict | null {
  const s = str(v)?.toUpperCase();
  return s && (VERDICTS as string[]).includes(s) ? (s as Verdict) : null;
}

function normalizeInvestigationState(v: unknown): InvestigationState {
  const s = str(v)?.toUpperCase();
  return s && (INVESTIGATION_STATES as string[]).includes(s) ? (s as InvestigationState) : 'UNKNOWN';
}

/** Verdict shown to the user: an ML-only detection is SUSPECTED, never CONFIRMED. */
function displayVerdict(verdict: Verdict | null, state: InvestigationState): Verdict {
  if (verdict) return verdict;
  if (state === 'FAILED' || state === 'PARTIAL') return 'INCONCLUSIVE';
  return 'SUSPECTED';
}

function strList(v: unknown): string[] {
  return Array.isArray(v) ? v.map((x) => str(x)).filter((x): x is string => x !== null) : [];
}

function normalizeEvidence(v: unknown): EvidenceItem[] {
  if (!Array.isArray(v)) return [];
  return v
    .map((item) => {
      if (typeof item === 'string') return { description: item, metric: null, observed: null, expected: null };
      if (!isObj(item)) return null;
      const description = str(item['description']);
      if (!description) return null;
      return { description, metric: str(item['metric']), observed: str(item['observed_value']), expected: str(item['expected_value']) };
    })
    .filter((e): e is EvidenceItem => e !== null);
}

function normalizeRagSources(v: unknown): RagSource[] {
  if (!Array.isArray(v)) return [];
  return v.filter(isObj).map((r) => ({
    source_id: str(r['source_id']) ?? '',
    file_name: str(r['file_name']) ?? str(r['source']) ?? 'unknown',
    source: str(r['source']) ?? '',
    category: str(r['category']),
    distance: num(r['distance']),
  }));
}

function normalizeAssessment(v: unknown): AgentAssessment | null {
  if (!isObj(v)) return null;
  const hyp = pick(v, 'attack_hypothesis');
  return {
    reasoning: str(v['reasoning']),
    confidence: normalizeConfidence(v['confidence']),
    supporting: normalizeEvidence(v['supporting_evidence']),
    contradicting: normalizeEvidence(v['contradicting_evidence']),
    alternatives: strList(v['alternative_explanations']),
    ragSources: normalizeRagSources(v['rag_sources']),
    recommendations: strList(v['recommendations']),
    verdict: normalizeVerdict(v['verdict']),
    attackHypothesis: hyp === undefined ? null : normalizeAttack(hyp),
  };
}

export function normalizeIncident(raw: unknown): Incident | null {
  if (!isObj(raw)) return null;
  const incident_id = str(raw['incident_id']);
  const node_id = str(raw['node_id']);
  if (!incident_id || !node_id) return null;
  const state = normalizeInvestigationState(raw['investigation_status']);
  const verdict = normalizeVerdict(raw['final_verdict']);
  const created_at = toIso(raw['created_at']);
  const prediction_timestamp = toIso(raw['prediction_timestamp']);
  const completed = toIso(raw['investigation_completed_at']);
  return {
    incident_id,
    node_id,
    attack_type: normalizeAttack(pick(raw, 'attack_type', 'ml_prediction')),
    ml_confidence: normalizeConfidence(raw['ml_confidence']),
    probabilities: isObj(raw['probabilities']) ? normalizeProbabilities(raw) : null,
    prediction_timestamp,
    created_at,
    status: str(raw['status']),
    investigation_status: state,
    verdict,
    displayVerdict: displayVerdict(verdict, state),
    report_available: bool(raw['report_available']) ?? false,
    investigation_started_at: toIso(raw['investigation_started_at']),
    investigation_completed_at: completed,
    observation_snapshot: isObj(raw['observation_snapshot']) ? raw['observation_snapshot'] : null,
    time: created_at ?? prediction_timestamp ?? completed,
  };
}

export function normalizeIncidents(payload: unknown): Incident[] {
  return unwrapList(payload, 'incidents')
    .map(normalizeIncident)
    .filter((i): i is Incident => i !== null)
    .sort((a, b) => (b.time ?? '').localeCompare(a.time ?? ''));
}

export function normalizeInvestigationReport(raw: unknown): InvestigationReport | null {
  if (!isObj(raw)) return null;
  const incident_id = str(raw['incident_id']);
  if (!incident_id) return null;
  const state = normalizeInvestigationState(raw['investigation_status']);
  const verdict = normalizeVerdict(raw['final_verdict']);
  const history: WorkflowStep[] = Array.isArray(raw['history'])
    ? raw['history'].filter(isObj).map((h) => ({
        step: str(h['step']) ?? '—',
        status: str(h['status']) ?? '—',
        timestamp: toIso(h['timestamp']),
        details: str(h['details']),
      }))
    : [];
  return {
    incident_id,
    node_id: str(raw['node_id']),
    attack_type: normalizeAttack(pick(raw, 'attack_type', 'ml_prediction')),
    investigation_status: state,
    verdict,
    displayVerdict: displayVerdict(verdict, state),
    ml_confidence: normalizeConfidence(raw['ml_confidence']),
    hypothesis: normalizeAssessment(raw['hypothesis']),
    confirmation: normalizeAssessment(raw['confirmation']),
    error: str(raw['error']),
    history,
    generated_at: toIso(raw['generated_at']),
  };
}

export function normalizeInvestigationReports(payload: unknown): InvestigationReport[] {
  return unwrapList(payload, 'investigations')
    .map(normalizeInvestigationReport)
    .filter((r): r is InvestigationReport => r !== null)
    .sort((a, b) => (b.generated_at ?? '').localeCompare(a.generated_at ?? ''));
}
