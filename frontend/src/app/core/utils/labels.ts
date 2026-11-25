import { ComponentState, NodeHealth, PredictionLabel, Severity } from '../models/common.model';
import { AlertStatus } from '../models/alert.model';
import { InvestigationState, Verdict } from '../models/incident.model';

export type Tone = 'success' | 'warning' | 'high' | 'critical' | 'info' | 'offline' | 'primary';

export const ATTACK_LABEL: Record<PredictionLabel, string> = {
  NORMAL: 'Normal',
  ECLIPSE: 'Eclipse attack',
  RPC_FLOODING: 'RPC flooding',
  UNKNOWN: 'Unknown',
};

export const ATTACK_TONE: Record<PredictionLabel, Tone> = {
  NORMAL: 'success',
  ECLIPSE: 'critical',
  RPC_FLOODING: 'high',
  UNKNOWN: 'offline',
};

/** Status colours for ML classes (reserved semantic colours — not series colours). */
export const ATTACK_COLOR: Record<PredictionLabel, string> = {
  NORMAL: '#12B76A',
  ECLIPSE: '#F04438',
  RPC_FLOODING: '#EF6820',
  UNKNOWN: '#98A2B3',
};

export const ATTACK_ICON: Record<PredictionLabel, string> = {
  NORMAL: 'shield-check',
  ECLIPSE: 'eclipse',
  RPC_FLOODING: 'zap',
  UNKNOWN: 'circle-question-mark',
};

export const SEVERITY_LABEL: Record<Severity, string> = {
  healthy: 'Healthy',
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  critical: 'Critical',
};

export const SEVERITY_TONE: Record<Severity, Tone> = {
  healthy: 'success',
  low: 'info',
  medium: 'warning',
  high: 'high',
  critical: 'critical',
};

export const SEVERITY_RANK: Record<Severity, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
  healthy: 0,
};

export const HEALTH_LABEL: Record<NodeHealth, string> = {
  healthy: 'Healthy',
  warning: 'Warning',
  critical: 'Critical',
  offline: 'Offline',
  unknown: 'Unknown',
};

export const HEALTH_TONE: Record<NodeHealth, Tone> = {
  healthy: 'success',
  warning: 'warning',
  critical: 'critical',
  offline: 'offline',
  unknown: 'offline',
};

export const HEALTH_ICON: Record<NodeHealth, string> = {
  healthy: 'circle-check',
  warning: 'triangle-exclamation-point',
  critical: 'octagon-exclamation-point',
  offline: 'unplug',
  unknown: 'circle-question-mark',
};

export const COMPONENT_LABEL: Record<ComponentState, string> = {
  up: 'Running',
  syncing: 'Syncing',
  down: 'Down',
  unknown: 'Unknown',
};

export const COMPONENT_TONE: Record<ComponentState, Tone> = {
  up: 'success',
  syncing: 'warning',
  down: 'critical',
  unknown: 'offline',
};

export const ALERT_STATUS_LABEL: Record<AlertStatus, string> = {
  active: 'Active',
  acknowledged: 'Acknowledged',
  resolved: 'Resolved',
};

export const ALERT_STATUS_TONE: Record<AlertStatus, Tone> = {
  active: 'critical',
  acknowledged: 'warning',
  resolved: 'success',
};

export const VERDICT_LABEL: Record<Verdict, string> = {
  CONFIRMED: 'Confirmed',
  SUSPECTED: 'Suspected',
  REJECTED: 'Rejected',
  INCONCLUSIVE: 'Inconclusive',
};

export const VERDICT_TONE: Record<Verdict, Tone> = {
  CONFIRMED: 'critical',
  SUSPECTED: 'warning',
  REJECTED: 'success',
  INCONCLUSIVE: 'offline',
};

export const VERDICT_ICON: Record<Verdict, string> = {
  CONFIRMED: 'shield-x',
  SUSPECTED: 'shield-exclamation-point',
  REJECTED: 'shield-check',
  INCONCLUSIVE: 'circle-question-mark',
};

export const INVESTIGATION_STATE_LABEL: Record<InvestigationState, string> = {
  NOT_STARTED: 'Queued',
  RUNNING: 'Running',
  COMPLETED: 'Completed',
  PARTIAL: 'Partial',
  FAILED: 'Failed',
  UNKNOWN: 'Unknown',
};

export const INVESTIGATION_STATE_TONE: Record<InvestigationState, Tone> = {
  NOT_STARTED: 'info',
  RUNNING: 'warning',
  COMPLETED: 'success',
  PARTIAL: 'warning',
  FAILED: 'critical',
  UNKNOWN: 'offline',
};
