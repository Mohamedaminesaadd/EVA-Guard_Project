
from collections import defaultdict
from datetime import datetime, timezone
from typing import Any


class ObservationAggregator:
    REQUIRED_COMPONENTS = {"geth", "beacon", "validator"}

    COMPONENT_PRIORITY = ("geth", "beacon", "validator")

    def __init__(self):
        self.pending = {}

    @staticmethod
    def _normalize_timestamp(timestamp: Any) -> str:
        if isinstance(timestamp, datetime):
            dt = timestamp
        elif isinstance(timestamp, (int, float)):
            dt = datetime.fromtimestamp(timestamp, tz=timezone.utc)
        elif isinstance(timestamp, str):
            value = timestamp.strip()

            if value.endswith("Z"):
                value = value[:-1] + "+00:00"

            dt = datetime.fromisoformat(value)
        else:
            raise ValueError(f"Invalid timestamp: {timestamp}")

        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)

        return dt.astimezone(timezone.utc).isoformat()

    @staticmethod
    def _get_value(metrics: dict, *keys, default=None):
        for key in keys:
            value = metrics.get(key)

            if value is not None:
                return value

        return default

    @staticmethod
    def _to_dict(value):
        return value if isinstance(value, dict) else {}

    def add_observation(self, observation: dict):
        """
        Add one component observation.

        Returns:
            dict: Aggregated observation when all components are available.
            None: When waiting for other components.
        """

        if not isinstance(observation, dict):
            raise ValueError("Observation must be a dictionary")

        timestamp = observation.get("timestamp")
        node_id = observation.get("node_id")
        component = observation.get("component")

        if not timestamp:
            raise ValueError("Observation timestamp is missing")

        if not node_id:
            raise ValueError("Observation node_id is missing")

        if not component:
            raise ValueError("Observation component is missing")

        component = str(component).lower().strip()

        if component not in self.REQUIRED_COMPONENTS:
            return None

        normalized_timestamp = self._normalize_timestamp(timestamp)

        metrics = self._to_dict(observation.get("metrics"))
        system = self._to_dict(observation.get("system"))

        if not metrics:
            raise ValueError(
                f"Metrics are missing for component {component}"
            )

        key = (str(node_id), normalized_timestamp)

        if key not in self.pending:
            self.pending[key] = {
                "timestamp": normalized_timestamp,
                "node_id": str(node_id),
                "components": {},
                "system": {},
            }

        entry = self.pending[key]

        # Preserve all original component metrics.
        entry["components"][component] = metrics.copy()

        # Preserve system metrics for each component.
        entry["system"][component] = system.copy()

        # Wait until all required components are received.
        if not self.REQUIRED_COMPONENTS.issubset(
            entry["components"].keys()
        ):
            return None

        aggregated = self._build_aggregated_row(entry)

        del self.pending[key]

        return aggregated

    def _merge_system_metrics(self, system_by_component: dict) -> dict:
        """
        Merge system metrics without discarding fields from other components.

        Priority:
            Geth -> Beacon -> Validator

        Existing values are not overwritten by lower-priority components.
        """

        merged = {}

        for component in reversed(self.COMPONENT_PRIORITY):
            metrics = system_by_component.get(component, {})

            if isinstance(metrics, dict):
                merged.update(metrics)

        return merged

    def _build_aggregated_row(self, entry: dict) -> dict:
        components = entry["components"]

        geth = self._to_dict(components.get("geth"))
        beacon = self._to_dict(components.get("beacon"))
        validator = self._to_dict(components.get("validator"))

        system_by_component = entry["system"]

        # Merge system metrics from all components.
        merged_system = self._merge_system_metrics(
            system_by_component
        )

        # Preserve original metrics for the investigation agents.
        raw_metrics = {
            "geth": geth.copy(),
            "beacon": beacon.copy(),
            "validator": validator.copy(),
            "system": {
                component: self._to_dict(
                    system_by_component.get(component)
                ).copy()
                for component in self.COMPONENT_PRIORITY
            },
        }

        row = {
            "timestamp": entry["timestamp"],
            "node_id": entry["node_id"],

            # Original metrics, preserved for AI agents.
            "raw_metrics": raw_metrics,

            # Original component metrics.
            "components": {
                "geth": geth.copy(),
                "beacon": beacon.copy(),
                "validator": validator.copy(),
            },

            # Merged system metrics.
            "system": merged_system.copy(),
        }

        # -------------------------------------------------
        # GETH METRICS
        # -------------------------------------------------

        row.update({
            "peer_count": self._get_value(
                geth,
                "peer_count",
                "geth_peer_count",
            ),

            "admin_peer_count": self._get_value(
                geth,
                "admin_peer_count",
                "geth_admin_peer_count",
            ),

            "new_peers": self._get_value(
                geth,
                "new_peers",
                "geth_new_peers",
            ),

            "disconnected_peers": self._get_value(
                geth,
                "disconnected_peers",
                "geth_disconnected_peers",
            ),

            "peer_churn_count": self._get_value(
                geth,
                "peer_churn_count",
                "geth_peer_churn_count",
            ),

            "rpc_latency_ms": self._get_value(
                geth,
                "rpc_latency_ms",
                "geth_rpc_latency_ms",
            ),

            "rpc_requests_total": self._get_value(
                geth,
                "rpc_requests_total",
                "geth_rpc_requests_total",
            ),

            "rpc_requests_per_sec": self._get_value(
                geth,
                "rpc_requests_per_sec",
                "geth_rpc_requests_per_sec",
            ),

            "block_number": self._get_value(
                geth,
                "block_number",
                "geth_block_number",
            ),

            "latest_block_age_secs": self._get_value(
                geth,
                "latest_block_age_secs",
                "geth_latest_block_age_secs",
            ),
        })

        # -------------------------------------------------
        # SYSTEM METRICS
        # -------------------------------------------------

        row.update({
            "cpu_usage_percent": self._get_value(
                merged_system,
                "cpu_usage_percent",
                "system_cpu_usage_percent",
            ),

            "memory_used_mb": self._get_value(
                merged_system,
                "memory_used_mb",
                "system_memory_used_mb",
            ),

            "network_rx_bytes_total": self._get_value(
                merged_system,
                "network_rx_bytes_total",
                "system_network_rx_bytes_total",
            ),

            "network_tx_bytes_total": self._get_value(
                merged_system,
                "network_tx_bytes_total",
                "system_network_tx_bytes_total",
            ),

            "network_rx_bytes_per_sec": self._get_value(
                merged_system,
                "network_rx_bytes_per_sec",
                "system_network_rx_bytes_per_sec",
            ),

            "network_tx_bytes_per_sec": self._get_value(
                merged_system,
                "network_tx_bytes_per_sec",
                "system_network_tx_bytes_per_sec",
            ),

            "tcp_established_connections": self._get_value(
                merged_system,
                "tcp_established_connections",
                "system_tcp_established_connections",
            ),
        })

        # -------------------------------------------------
        # BEACON METRICS
        # -------------------------------------------------

        row.update({
            "finalized_epoch": self._get_value(
                beacon,
                "finalized_epoch",
                "beacon_finalized_epoch",
            ),

            "justified_epoch": self._get_value(
                beacon,
                "justified_epoch",
                "beacon_justified_epoch",
            ),

            "finality_distance_epochs": self._get_value(
                beacon,
                "finality_distance_epochs",
                "beacon_finality_distance_epochs",
            ),

            "head_slot": self._get_value(
                beacon,
                "head_slot",
                "beacon_head_slot",
            ),

            "sync_distance": self._get_value(
                beacon,
                "sync_distance",
                "beacon_sync_distance",
            ),

            "block_propagation_delay_ms": self._get_value(
                beacon,
                "block_propagation_delay_ms",
                "beacon_block_propagation_delay_ms",
            ),
        })

        # -------------------------------------------------
        # VALIDATOR METRICS
        # -------------------------------------------------

        row.update({
            "attestations_total": self._get_value(
                validator,
                "attestations_total",
                "validator_attestations_total",
            ),

            "blocks_proposed_total": self._get_value(
                validator,
                "blocks_proposed_total",
                "validator_blocks_proposed_total",
            ),

            "proposer_duties": self._get_value(
                validator,
                "proposer_duties",
                "validator_proposer_duties",
            ),
        })

        return row

    def clear(self):
        """Clear all incomplete observations."""
        self.pending.clear()

    def pending_count(self) -> int:
        """Return the number of incomplete aggregated observations."""
        return len(self.pending)
