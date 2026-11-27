import { AttackClass } from '../models/common.model';

/**
 * Static facts about the trained model (from the training pipeline, not from the API).
 * Offline metrics come from the held-out test set evaluation — they are NOT live accuracy.
 */
export const MODEL_INFO = {
  name: 'Logistic Regression',
  framework: 'scikit-learn Pipeline',
  estimator: 'LogisticRegression',
  classes: ['NORMAL', 'ECLIPSE', 'RPC_FLOODING'] as AttackClass[],
  featureCount: 69,
  input: 'Temporal and engineered Ethereum node metrics (Geth, Lighthouse beacon/validator, system, network)',
} as const;

export interface OfflineEvaluation {
  label: string;
  accuracy: number;
  macroPrecision: number;
  macroRecall: number;
  macroF1: number;
  /**
   * Rows = true class, columns = predicted class, in MODEL_INFO.classes order.
   * Left `null` on purpose: the matrix was not provided. Paste the values printed by
   * sklearn.metrics.confusion_matrix(y_test, y_pred, labels=[...]) to display it.
   */
  confusionMatrix: number[][] | null;
}

export const OFFLINE_EVALUATION: OfflineEvaluation = {
  label: 'Offline test evaluation',
  accuracy: 0.9556,
  macroPrecision: 0.9569,
  macroRecall: 0.9556,
  macroF1: 0.9555,
  confusionMatrix: null,
};

export interface FeatureCategory {
  key: string;
  name: string;
  description: string;
  /** Source metrics from the Rust Observation schema the category is built from. */
  sources: string[];
  icon: string;
}

export const FEATURE_CATEGORIES: FeatureCategory[] = [
  {
    key: 'raw',
    name: 'Raw metrics',
    description: 'Gauges taken as-is from each observation (system, network, Geth, beacon, validator).',
    sources: ['cpu_usage_percent', 'memory_used_mb', 'peer_count', 'rpc_latency_ms', 'tcp_established_connections', 'sync_distance'],
    icon: 'database',
  },
  {
    key: 'delta',
    name: 'Counter deltas',
    description: 'Difference of monotonic counters between two consecutive observations of the same node.',
    sources: ['network_rx_bytes_total', 'network_tx_bytes_total', 'rpc_requests_total', 'attestations_total', 'block_number'],
    icon: 'git-compare',
  },
  {
    key: 'rate',
    name: 'Counter rates',
    description: 'Counter deltas divided by the elapsed time (per-second rates), robust to irregular collection intervals.',
    sources: ['network_rx_bytes_total', 'network_tx_bytes_total', 'rpc_requests_total'],
    icon: 'gauge',
  },
  {
    key: 'gauge-change',
    name: 'Gauge changes',
    description: 'Variation of gauges over time to capture sudden shifts (peer drops, latency spikes, propagation delay).',
    sources: ['peer_count', 'rpc_latency_ms', 'block_propagation_delay_ms', 'latest_block_age_seconds'],
    icon: 'activity',
  },
  {
    key: 'interaction',
    name: 'Interaction features',
    description: 'Combinations of metrics that separate attack signatures (e.g. RPC load vs. latency, peers vs. churn).',
    sources: ['rpc_requests_per_sec × rpc_latency_ms', 'peer_count ÷ peer_churn_count', 'cpu_usage_percent × rpc rate'],
    icon: 'sparkles',
  },
];
