
from datetime import datetime, timezone
from typing import Any, Dict, Optional

import math

from .config import (
    FEATURE_COLUMNS,
    FEATURE_CONFIG,
    FEATURE_ENGINEERING_CONFIG
)


class FeatureBuilder:
    """
    Reproduces the temporal and interaction feature
    engineering used during model training.

    Maintains independent temporal history for each node.
    """

    COUNTER_COLUMNS = [
        "geth_rpc_requests_total",
        "system_network_rx_bytes_total",
        "system_network_tx_bytes_total",
        "validator_attestations_total",
        "validator_blocks_proposed_total",
        "validator_proposer_duties"
    ]

    GAUGE_COLUMNS = [
        "geth_peer_count",
        "geth_rpc_latency_ms",
        "geth_block_number",
        "geth_latest_block_age_secs",
        "system_cpu_usage_percent",
        "system_memory_used_mb",
        "system_tcp_established_connections",
        "beacon_finalized_epoch",
        "beacon_justified_epoch",
        "beacon_finality_distance_epochs",
        "beacon_head_slot",
        "beacon_sync_distance",
        "beacon_block_propagation_delay_ms"
    ]

    def __init__(self):

        self.raw_features = FEATURE_CONFIG["features"]

        self.feature_columns = FEATURE_COLUMNS

        self.max_interval = FEATURE_ENGINEERING_CONFIG.get(
            "segmentation_threshold_seconds",
            10
        )

        self.history = {}

        if len(self.raw_features) != 26:
            raise ValueError(
                f"Expected 26 raw features, got {len(self.raw_features)}"
            )

        if len(self.feature_columns) != 69:
            raise ValueError(
                f"Expected 69 model features, got {len(self.feature_columns)}"
            )

    @staticmethod
    def _to_float(value: Any) -> float:

        if value is None:
            return math.nan

        if isinstance(value, bool):
            return float(value)

        try:
            result = float(value)

            if not math.isfinite(result):
                return math.nan

            return result

        except (TypeError, ValueError):
            return math.nan

    @staticmethod
    def _parse_timestamp(timestamp: str) -> datetime:

        if not timestamp:
            raise ValueError("Timestamp is missing.")

        dt = datetime.fromisoformat(
            timestamp.replace("Z", "+00:00")
        )

        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)

        return dt.astimezone(timezone.utc)

    @staticmethod
    def _safe_delta(
        current: float,
        previous: Optional[float]
    ) -> float:

        if previous is None:
            return math.nan

        if not math.isfinite(current):
            return math.nan

        if not math.isfinite(previous):
            return math.nan

        return current - previous

    @staticmethod
    def _safe_rate(
        delta: float,
        interval: Optional[float]
    ) -> float:

        if interval is None:
            return math.nan

        if not math.isfinite(delta):
            return math.nan

        if not 0 < interval < 10:
            return math.nan

        return delta / interval

    @staticmethod
    def _safe_divide(
        numerator: float,
        denominator: float
    ) -> float:

        if not math.isfinite(numerator):
            return math.nan

        if not math.isfinite(denominator):
            return math.nan

        if denominator == 0:
            return math.nan

        return numerator / denominator

    def build(
        self,
        observation: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Build exactly 69 features for one node observation.
        """

        node_id = observation.get("node_id")

        if not node_id:
            raise ValueError("node_id is missing.")

        timestamp = self._parse_timestamp(
            observation.get("timestamp")
        )

        # Extract and normalize the 26 raw features.
        raw = {}

        for column in self.raw_features:
            raw[column] = self._to_float(
                observation.get(column)
            )

        # Get the previous state for this node.
        previous_state = self.history.get(node_id)

        interval = None
        new_segment = True

        if previous_state is not None:

            interval = (
                timestamp - previous_state["timestamp"]
            ).total_seconds()

            new_segment = (
                interval <= 0
                or interval > self.max_interval
            )

        # Initialize all 69 model features.
        features = {
            column: math.nan
            for column in self.feature_columns
        }

        # 1. Raw features.
        for column in self.raw_features:
            if column in features:
                features[column] = raw[column]

        # 2. Temporal features.
        if not new_segment:

            previous_raw = previous_state["raw"]

            # Cumulative counters.
            for column in self.COUNTER_COLUMNS:

                current = raw[column]
                previous = previous_raw[column]

                delta = self._safe_delta(
                    current,
                    previous
                )

                # Counter reset detection.
                if math.isfinite(delta) and delta < 0:
                    delta = math.nan

                features[f"{column}_delta"] = delta

                features[f"{column}_rate"] = (
                    self._safe_rate(delta, interval)
                )

            # Instantaneous measurements.
            for column in self.GAUGE_COLUMNS:

                current = raw[column]
                previous = previous_raw[column]

                delta = self._safe_delta(
                    current,
                    previous
                )

                features[f"{column}_delta"] = delta

                features[f"{column}_change_per_sec"] = (
                    self._safe_rate(delta, interval)
                )

        # 3. Interaction features.
        features["rpc_requests_per_peer"] = (
            self._safe_divide(
                raw["geth_rpc_requests_per_sec"],
                raw["geth_peer_count"]
            )
        )

        features["tcp_connections_per_peer"] = (
            self._safe_divide(
                raw["system_tcp_established_connections"],
                raw["geth_peer_count"]
            )
        )

        features["network_rx_tx_ratio"] = (
            self._safe_divide(
                raw["system_network_rx_bytes_per_sec"],
                raw["system_network_tx_bytes_per_sec"]
            )
        )

        if (
            math.isfinite(raw["geth_rpc_requests_per_sec"])
            and math.isfinite(raw["geth_rpc_latency_ms"])
        ):
            features["rpc_load_latency_interaction"] = (
                raw["geth_rpc_requests_per_sec"]
                * raw["geth_rpc_latency_ms"]
            )

        features["peer_churn_ratio"] = (
            self._safe_divide(
                raw["geth_peer_churn_count"],
                raw["geth_peer_count"]
            )
        )

        # Update temporal history independently for each node.
        self.history[node_id] = {
            "timestamp": timestamp,
            "raw": raw
        }

        # Return features in the exact model order.
        result = {
            column: features[column]
            for column in self.feature_columns
        }

        if len(result) != 69:
            raise ValueError(
                f"Invalid feature count: {len(result)}"
            )

        return result

    def reset_node(self, node_id: str):

        self.history.pop(node_id, None)

    def reset(self):

        self.history.clear()

    def get_feature_columns(self):

        return self.feature_columns.copy()
