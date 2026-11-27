
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
    Reproduces the temporal and interaction feature engineering
    used during model training.

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

        self.raw_features = list(FEATURE_CONFIG["features"])
        self.feature_columns = list(FEATURE_COLUMNS)

        self.max_interval = float(
            FEATURE_ENGINEERING_CONFIG.get(
                "segmentation_threshold_seconds", 10
            )
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

        if self.max_interval <= 0:
            raise ValueError(
                "segmentation_threshold_seconds must be positive."
            )

        if len(set(self.raw_features)) != len(self.raw_features):
            raise ValueError("Duplicate raw feature names detected.")

        if len(set(self.feature_columns)) != len(self.feature_columns):
            raise ValueError("Duplicate model feature names detected.")

        missing_raw = [
            column for column in self.raw_features
            if column not in self.feature_columns
        ]

        if missing_raw:
            raise ValueError(
                f"Raw features missing from model features: {missing_raw}"
            )

        expected_temporal = []

        for column in self.COUNTER_COLUMNS:
            expected_temporal.extend([
                f"{column}_delta",
                f"{column}_rate"
            ])

        for column in self.GAUGE_COLUMNS:
            expected_temporal.extend([
                f"{column}_delta",
                f"{column}_change_per_sec"
            ])

        missing_temporal = [
            column for column in expected_temporal
            if column not in self.feature_columns
        ]

        if missing_temporal:
            raise ValueError(
                f"Missing temporal model features: {missing_temporal}"
            )

        print("[FEATURE BUILDER] Initialized")
        print(f"[FEATURE BUILDER] Raw features: {len(self.raw_features)}")
        print(f"[FEATURE BUILDER] Model features: {len(self.feature_columns)}")
        print(f"[FEATURE BUILDER] Max interval: {self.max_interval}s")

    # =====================================================
    # VALUE CONVERSION
    # =====================================================

    @staticmethod
    def _to_float(value: Any) -> float:
        """
        Convert a value to a finite float.
        Invalid or missing values become NaN.
        """

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

    # =====================================================
    # TIMESTAMP
    # =====================================================

    @staticmethod
    def _parse_timestamp(timestamp: Any) -> datetime:
        """
        Parse ISO-8601 timestamps and normalize to UTC.
        """

        if not timestamp:
            raise ValueError("Timestamp is missing.")

        if isinstance(timestamp, datetime):
            dt = timestamp

        elif isinstance(timestamp, (int, float)):
            dt = datetime.fromtimestamp(
                timestamp,
                tz=timezone.utc
            )

        elif isinstance(timestamp, str):
            dt = datetime.fromisoformat(
                timestamp.replace("Z", "+00:00")
            )

        else:
            raise ValueError(
                f"Unsupported timestamp type: {type(timestamp).__name__}"
            )

        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)

        return dt.astimezone(timezone.utc)

    # =====================================================
    # TEMPORAL CALCULATIONS
    # =====================================================

    @staticmethod
    def _safe_delta(
        current: float,
        previous: Optional[float]
    ) -> float:
        """
        Calculate the difference between two finite values.
        """

        if previous is None:
            return math.nan

        if not math.isfinite(current):
            return math.nan

        if not math.isfinite(previous):
            return math.nan

        return current - previous

    def _safe_rate(
        self,
        delta: float,
        interval: Optional[float]
    ) -> float:
        """
        Calculate a per-second rate.

        The interval must be positive and within the configured
        segmentation threshold.
        """

        if interval is None:
            return math.nan

        if not math.isfinite(delta):
            return math.nan

        if not math.isfinite(interval):
            return math.nan

        if not 0 < interval <= self.max_interval:
            return math.nan

        return delta / interval

    @staticmethod
    def _safe_divide(
        numerator: float,
        denominator: float
    ) -> float:
        """
        Safely divide two finite values.
        """

        if not math.isfinite(numerator):
            return math.nan

        if not math.isfinite(denominator):
            return math.nan

        if denominator == 0:
            return math.nan

        return numerator / denominator

    # =====================================================
    # RAW FEATURE VALIDATION
    # =====================================================

    def _validate_raw_features(
        self,
        raw: Dict[str, float],
        node_id: str
    ):
        """
        Validate raw feature availability.
        """

        finite_count = sum(
            math.isfinite(value)
            for value in raw.values()
        )

        if finite_count == 0:
            raise ValueError(
                f"No valid raw features for node {node_id}. "
                "Check RustAdapter field mapping."
            )

        missing = [
            column
            for column, value in raw.items()
            if not math.isfinite(value)
        ]

        if missing:
            print(
                f"[FEATURE BUILDER WARNING] Node={node_id} | "
                f"Missing {len(missing)}/{len(raw)} raw features: "
                f"{missing}"
            )

        return finite_count

    # =====================================================
    # FEATURE BUILDING
    # =====================================================

    def build(
        self,
        observation: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Build exactly 69 model features for one node observation.
        """

        if not isinstance(observation, dict):
            raise TypeError("Observation must be a dictionary.")

        node_id = observation.get("node_id")

        if not node_id:
            raise ValueError("node_id is missing.")

        timestamp = self._parse_timestamp(
            observation.get("timestamp")
        )

        node_id = str(node_id)

        # -------------------------------------------------
        # 1. Extract and normalize the 26 raw features.
        # -------------------------------------------------

        raw = {
            column: self._to_float(
                observation.get(column)
            )
            for column in self.raw_features
        }

        finite_count = self._validate_raw_features(
            raw,
            node_id
        )

        # -------------------------------------------------
        # 2. Retrieve previous state for this node only.
        # -------------------------------------------------

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

            if new_segment:
                print(
                    f"[FEATURE BUILDER] New temporal segment "
                    f"for {node_id} | interval={interval:.3f}s"
                )

        # -------------------------------------------------
        # 3. Initialize all 69 model features.
        # -------------------------------------------------

        features = {
            column: math.nan
            for column in self.feature_columns
        }

        # -------------------------------------------------
        # 4. Raw features.
        # -------------------------------------------------

        for column in self.raw_features:
            features[column] = raw[column]

        # -------------------------------------------------
        # 5. Temporal features.
        # -------------------------------------------------

        if not new_segment:

            previous_raw = previous_state["raw"]

            # Cumulative counters.
            for column in self.COUNTER_COLUMNS:

                current = raw.get(column, math.nan)
                previous = previous_raw.get(column, math.nan)

                delta = self._safe_delta(
                    current,
                    previous
                )

                # Counter reset detection.
                if math.isfinite(delta) and delta < 0:
                    delta = math.nan

                delta_column = f"{column}_delta"
                rate_column = f"{column}_rate"

                features[delta_column] = delta

                features[rate_column] = self._safe_rate(
                    delta,
                    interval
                )

            # Instantaneous measurements.
            for column in self.GAUGE_COLUMNS:

                current = raw.get(column, math.nan)
                previous = previous_raw.get(column, math.nan)

                delta = self._safe_delta(
                    current,
                    previous
                )

                delta_column = f"{column}_delta"
                rate_column = f"{column}_change_per_sec"

                features[delta_column] = delta

                features[rate_column] = self._safe_rate(
                    delta,
                    interval
                )

        # -------------------------------------------------
        # 6. Interaction features.
        # -------------------------------------------------

        interactions = {
            "rpc_requests_per_peer": self._safe_divide(
                raw.get("geth_rpc_requests_per_sec", math.nan),
                raw.get("geth_peer_count", math.nan)
            ),

            "tcp_connections_per_peer": self._safe_divide(
                raw.get("system_tcp_established_connections", math.nan),
                raw.get("geth_peer_count", math.nan)
            ),

            "network_rx_tx_ratio": self._safe_divide(
                raw.get("system_network_rx_bytes_per_sec", math.nan),
                raw.get("system_network_tx_bytes_per_sec", math.nan)
            ),

            "peer_churn_ratio": self._safe_divide(
                raw.get("geth_peer_churn_count", math.nan),
                raw.get("geth_peer_count", math.nan)
            )
        }

        rpc_rate = raw.get(
            "geth_rpc_requests_per_sec",
            math.nan
        )

        rpc_latency = raw.get(
            "geth_rpc_latency_ms",
            math.nan
        )

        if (
            math.isfinite(rpc_rate)
            and math.isfinite(rpc_latency)
        ):
            interactions["rpc_load_latency_interaction"] = (
                rpc_rate * rpc_latency
            )
        else:
            interactions["rpc_load_latency_interaction"] = math.nan

        for column, value in interactions.items():
            if column in features:
                features[column] = value

        # -------------------------------------------------
        # 7. Update independent temporal history per node.
        # -------------------------------------------------

        self.history[node_id] = {
            "timestamp": timestamp,
            "raw": raw.copy()
        }

        # -------------------------------------------------
        # 8. Return features in exact model order.
        # -------------------------------------------------

        result = {
            column: features[column]
            for column in self.feature_columns
        }

        if len(result) != 69:
            raise ValueError(
                f"Invalid feature count: {len(result)}"
            )

        valid_model_features = sum(
            math.isfinite(value)
            for value in result.values()
        )

        missing_model_features = [
            column
            for column, value in result.items()
            if not math.isfinite(value)
        ]

        print(
            f"[FEATURE BUILDER] Node={node_id} | "
            f"Raw={finite_count}/26 | "
            f"Model={valid_model_features}/69 | "
            f"RPC rate={raw.get('geth_rpc_requests_per_sec')} | "
            f"Peers={raw.get('geth_peer_count')}"
        )

        if missing_model_features:
            print(
                f"[FEATURE BUILDER WARNING] "
                f"Missing model features: {missing_model_features}"
            )

        return result

    # =====================================================
    # HISTORY MANAGEMENT
    # =====================================================

    def reset_node(self, node_id: str):
        """
        Reset temporal history for one node.
        """

        self.history.pop(str(node_id), None)

    def reset(self):
        """
        Reset temporal history for all nodes.
        """

        self.history.clear()

    def get_feature_columns(self):
        return self.feature_columns.copy()
