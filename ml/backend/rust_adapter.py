
from typing import Any, Dict, List
import logging
import math

from .config import FEATURE_CONFIG


logger = logging.getLogger(__name__)


class RustAdapter:
    """
    Converts aggregated Rust observations into the 26 raw
    features expected by the ML feature engineering pipeline.

    Expected input:
        {
            "node_id": "...",
            "timestamp": "...",
            "components": {
                "geth": {...},
                "beacon": {...},
                "validator": {...}
            },
            "system": {...}
        }

    Output:
        {
            "node_id": "...",
            "timestamp": "...",
            ... 26 numeric raw features
        }
    """

    FIELD_MAPPING = {
        # Geth
        "geth_peer_count": "peer_count",
        "geth_admin_peer_count": "admin_peer_count",
        "geth_new_peers": "new_peers",
        "geth_disconnected_peers": "disconnected_peers",
        "geth_peer_churn_count": "peer_churn_count",
        "geth_rpc_latency_ms": "rpc_latency_ms",
        "geth_rpc_requests_total": "rpc_requests_total",
        "geth_rpc_requests_per_sec": "rpc_requests_per_sec",
        "geth_block_number": "block_number",
        "geth_latest_block_age_secs": "latest_block_age_secs",

        # System
        "system_cpu_usage_percent": "cpu_usage_percent",
        "system_memory_used_mb": "memory_used_mb",
        "system_network_rx_bytes_total": "network_rx_bytes_total",
        "system_network_tx_bytes_total": "network_tx_bytes_total",
        "system_network_rx_bytes_per_sec": "network_rx_bytes_per_sec",
        "system_network_tx_bytes_per_sec": "network_tx_bytes_per_sec",
        "system_tcp_established_connections": "tcp_established_connections",

        # Beacon
        "beacon_finalized_epoch": "finalized_epoch",
        "beacon_justified_epoch": "justified_epoch",
        "beacon_finality_distance_epochs": "finality_distance_epochs",
        "beacon_head_slot": "head_slot",
        "beacon_sync_distance": "sync_distance",
        "beacon_block_propagation_delay_ms": "block_propagation_delay_ms",

        # Validator
        "validator_attestations_total": "attestations_total",
        "validator_blocks_proposed_total": "blocks_proposed_total",
        "validator_proposer_duties": "proposer_duties",
    }

    COMPONENT_PREFIXES = {
        "geth_": "geth",
        "beacon_": "beacon",
        "validator_": "validator",
        "system_": "system",
    }

    def __init__(self):
        self.feature_columns = list(FEATURE_CONFIG["features"])

        if len(self.feature_columns) != 26:
            raise ValueError(
                f"Expected 26 raw features, got {len(self.feature_columns)}"
            )

        if len(set(self.feature_columns)) != len(self.feature_columns):
            raise ValueError("Duplicate raw feature names detected.")

        unsupported = [
            feature
            for feature in self.feature_columns
            if feature not in self.FIELD_MAPPING
        ]

        if unsupported:
            raise ValueError(
                f"No Rust mapping defined for features: {unsupported}"
            )

        logger.info(
            "[ADAPTER] Initialized with %d raw features.",
            len(self.feature_columns)
        )

    @staticmethod
    def _to_numeric(value: Any) -> float:
        """
        Convert a value to a finite float.

        Zero and False are preserved as valid numeric values.
        None, invalid strings, NaN and infinity become NaN.
        """

        if value is None:
            return math.nan

        if isinstance(value, bool):
            return float(value)

        if isinstance(value, str) and not value.strip():
            return math.nan

        try:
            result = float(value)

            if not math.isfinite(result):
                return math.nan

            return result

        except (TypeError, ValueError, OverflowError):
            return math.nan

    @staticmethod
    def _get_nested_value(data: Any, field: str):
        if not isinstance(data, dict):
            return None

        return data.get(field)

    def _get_component_name(self, feature: str):
        for prefix, component in self.COMPONENT_PREFIXES.items():
            if feature.startswith(prefix):
                return component

        return None

    def _get_source_value(
        self,
        observation: Dict[str, Any],
        feature: str,
        source_field: str
    ):
        component_name = self._get_component_name(feature)

        if component_name is None:
            return None

        # 1. Direct flattened fields.
        # System fields are also checked here because the
        # aggregator may expose them at the root level.
        if source_field in observation:
            return observation[source_field]

        # 2. System metrics.
        if component_name == "system":

            system = observation.get("system", {})

            value = self._get_nested_value(
                system,
                source_field
            )

            if value is not None:
                return value

            # Fallback to original system metrics.
            raw_metrics = observation.get("raw_metrics", {})

            if isinstance(raw_metrics, dict):
                raw_system = raw_metrics.get("system", {})

                if isinstance(raw_system, dict):
                    for component_metrics in raw_system.values():

                        value = self._get_nested_value(
                            component_metrics,
                            source_field
                        )

                        if value is not None:
                            return value

            return None

        # 3. Component metrics.
        components = observation.get("components", {})

        if isinstance(components, dict):
            component = components.get(component_name, {})

            value = self._get_nested_value(
                component,
                source_field
            )

            if value is not None:
                return value

        # 4. Fallback to original raw component metrics.
        raw_metrics = observation.get("raw_metrics", {})

        if isinstance(raw_metrics, dict):
            component = raw_metrics.get(component_name, {})

            return self._get_nested_value(
                component,
                source_field
            )

        return None

    def adapt(self, observation: Dict[str, Any]) -> Dict[str, Any]:
        """
        Convert one aggregated observation into 26 raw features.
        """

        if not isinstance(observation, dict):
            raise TypeError("Observation must be a dictionary.")

        node_id = observation.get("node_id")
        timestamp = observation.get("timestamp")

        if not node_id:
            raise ValueError("Observation node_id is missing.")

        if not timestamp:
            raise ValueError("Observation timestamp is missing.")

        adapted = {
            "node_id": str(node_id),
            "timestamp": timestamp,
        }

        missing_features: List[str] = []

        for feature in self.feature_columns:

            source_field = self.FIELD_MAPPING[feature]

            raw_value = self._get_source_value(
                observation,
                feature,
                source_field
            )

            numeric_value = self._to_numeric(raw_value)

            adapted[feature] = numeric_value

            if not math.isfinite(numeric_value):
                missing_features.append(feature)

        valid_count = len(self.feature_columns) - len(missing_features)

        logger.info(
            "[ADAPTER] Node=%s | Valid=%d/%d | Missing=%d/%d",
            node_id,
            valid_count,
            len(self.feature_columns),
            len(missing_features),
            len(self.feature_columns)
        )

        if missing_features:
            logger.warning(
                "[ADAPTER] Missing features for node %s: %s",
                node_id,
                missing_features
            )

        if valid_count == 0:
            raise ValueError(
                f"All 26 raw features are missing for node {node_id}."
            )

        return adapted

    def adapt_many(
        self,
        observations: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:

        return [
            self.adapt(observation)
            for observation in observations
        ]

    def get_feature_columns(self) -> List[str]:
        return self.feature_columns.copy()
