import { NodeMetricKey } from '../../core/models/node.model';
import { fmtBytes, fmtDuration, fmtMb, fmtMs, fmtNumber, fmtPercentValue } from '../../core/utils/format';

export type MetricGroup = 'system' | 'network' | 'geth' | 'beacon' | 'validator';
export type Aggregation = 'avg' | 'sum' | 'max' | 'min';

export interface MetricDef {
  key: NodeMetricKey;
  title: string;
  unit: string;
  group: MetricGroup;
  format: (v: number) => string;
  agg: Aggregation;
  icon?: string;
  yMin?: number;
  yMax?: number;
  step?: boolean;
  /** One-line explanation shown under metric cards. */
  hint?: string;
}

const n0 = (v: number) => fmtNumber(v);
const n1 = (v: number) => fmtNumber(v, 1);

/** The 8 infrastructure time-series of the monitoring workspace. */
export const INFRA_METRICS: MetricDef[] = [
  { key: 'cpu_usage_percent', title: 'CPU usage', unit: '%', group: 'system', format: (v) => fmtPercentValue(v, 0), agg: 'avg', yMin: 0, yMax: 100, icon: 'cpu' },
  { key: 'memory_used_mb', title: 'Memory usage', unit: 'MB', group: 'system', format: fmtMb, agg: 'avg', yMin: 0, icon: 'memory-stick' },
  { key: 'network_rx_bytes_per_sec', title: 'Network RX', unit: 'bytes/s', group: 'network', format: (v) => fmtBytes(v, true), agg: 'sum', yMin: 0, icon: 'arrow-down-right' },
  { key: 'network_tx_bytes_per_sec', title: 'Network TX', unit: 'bytes/s', group: 'network', format: (v) => fmtBytes(v, true), agg: 'sum', yMin: 0, icon: 'arrow-up-right' },
  { key: 'tcp_established_connections', title: 'TCP established connections', unit: 'conn', group: 'network', format: n0, agg: 'sum', yMin: 0, icon: 'cable' },
  { key: 'peer_count', title: 'Geth peer count', unit: 'peers', group: 'geth', format: n0, agg: 'avg', yMin: 0, step: true, icon: 'waypoints' },
  { key: 'rpc_requests_per_sec', title: 'RPC requests/sec', unit: 'req/s', group: 'geth', format: n1, agg: 'sum', yMin: 0, icon: 'zap' },
  { key: 'rpc_latency_ms', title: 'RPC latency', unit: 'ms', group: 'geth', format: fmtMs, agg: 'max', yMin: 0, icon: 'timer' },
];

export const GETH_CARDS: MetricDef[] = [
  { key: 'peer_count', title: 'Peer count', unit: 'peers', group: 'geth', format: n0, agg: 'avg', hint: 'net_peerCount' },
  { key: 'admin_peer_count', title: 'Admin peer count', unit: 'peers', group: 'geth', format: n0, agg: 'avg', hint: 'admin_peers length' },
  { key: 'new_peers', title: 'New peers', unit: '/cycle', group: 'geth', format: n0, agg: 'sum', hint: 'Joined since last cycle' },
  { key: 'disconnected_peers', title: 'Disconnected peers', unit: '/cycle', group: 'geth', format: n0, agg: 'sum', hint: 'Left since last cycle' },
  { key: 'peer_churn_count', title: 'Peer churn', unit: '/cycle', group: 'geth', format: n0, agg: 'sum', hint: 'New + disconnected' },
  { key: 'block_number', title: 'Block number', unit: '', group: 'geth', format: n0, agg: 'max', hint: 'Highest head' },
  { key: 'latest_block_age_seconds', title: 'Latest block age', unit: '', group: 'geth', format: (v) => fmtDuration(v), agg: 'max', hint: 'Worst node' },
  { key: 'rpc_requests_total', title: 'RPC requests', unit: 'total', group: 'geth', format: (v) => fmtNumber(v), agg: 'sum', hint: 'Counter' },
];

export const BEACON_CARDS: MetricDef[] = [
  { key: 'head_slot', title: 'Head slot', unit: '', group: 'beacon', format: n0, agg: 'max', hint: 'Highest head slot' },
  { key: 'finalized_epoch', title: 'Finalized epoch', unit: '', group: 'beacon', format: n0, agg: 'min', hint: 'Lowest across nodes' },
  { key: 'justified_epoch', title: 'Justified epoch', unit: '', group: 'beacon', format: n0, agg: 'min', hint: 'Lowest across nodes' },
  { key: 'sync_distance', title: 'Sync distance', unit: 'slots', group: 'beacon', format: n0, agg: 'max', hint: 'Worst node' },
  { key: 'block_propagation_delay_ms', title: 'Block propagation delay', unit: '', group: 'beacon', format: fmtMs, agg: 'max', hint: 'Worst node' },
];

export const VALIDATOR_CARDS: MetricDef[] = [
  { key: 'attestations_total', title: 'Attestations total', unit: '', group: 'validator', format: n0, agg: 'sum', hint: 'Counter, all validators' },
  { key: 'blocks_proposed', title: 'Blocks proposed', unit: '', group: 'validator', format: n0, agg: 'sum', hint: 'Counter' },
  { key: 'proposer_duties', title: 'Proposer duties', unit: '', group: 'validator', format: n0, agg: 'sum', hint: 'Assigned duties' },
];

/** Secondary charts shown under each Ethereum section. */
export const GETH_CHARTS: MetricDef[] = [
  { key: 'peer_churn_count', title: 'Peer churn', unit: 'peers/cycle', group: 'geth', format: n0, agg: 'sum', yMin: 0, step: true },
  { key: 'latest_block_age_seconds', title: 'Latest block age', unit: 's', group: 'geth', format: (v) => `${v.toFixed(0)} s`, agg: 'max', yMin: 0 },
];
export const BEACON_CHARTS: MetricDef[] = [
  { key: 'sync_distance', title: 'Sync distance', unit: 'slots', group: 'beacon', format: n0, agg: 'max', yMin: 0, step: true },
  { key: 'block_propagation_delay_ms', title: 'Block propagation delay', unit: 'ms', group: 'beacon', format: fmtMs, agg: 'max', yMin: 0 },
];
export const VALIDATOR_CHARTS: MetricDef[] = [
  { key: 'attestations_total', title: 'Attestations total', unit: 'count', group: 'validator', format: n0, agg: 'sum', step: true },
];

export const METRIC_GROUPS: { value: MetricGroup | 'all'; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'system', label: 'System' },
  { value: 'network', label: 'Network' },
  { value: 'geth', label: 'Geth' },
  { value: 'beacon', label: 'Beacon' },
  { value: 'validator', label: 'Validator' },
];

export function aggregate(values: (number | null | undefined)[], agg: Aggregation): number | null {
  const v = values.filter((x): x is number => typeof x === 'number' && Number.isFinite(x));
  if (!v.length) return null;
  switch (agg) {
    case 'sum':
      return v.reduce((a, b) => a + b, 0);
    case 'max':
      return Math.max(...v);
    case 'min':
      return Math.min(...v);
    default:
      return v.reduce((a, b) => a + b, 0) / v.length;
  }
}

export const AGG_LABEL: Record<Aggregation, string> = { avg: 'avg', sum: 'sum', max: 'max', min: 'min' };
