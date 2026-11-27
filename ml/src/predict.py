
from pathlib import Path
import json

import joblib
import numpy as np
import pandas as pd


# =====================================================
# PATHS
# =====================================================

ROOT = Path(__file__).resolve().parents[1]

MODEL_FILE = ROOT / "models/best_model.joblib"
METADATA_FILE = ROOT / "models/model_metadata.json"

DATA_FILE = ROOT / "data/processed/dataset_clean.csv"
RESULTS_DIR = ROOT / "results"

RESULTS_DIR.mkdir(exist_ok=True)

MAX_INTERVAL_SEC = 10


# =====================================================
# RAW FEATURES
# =====================================================

RAW_FEATURES = [
    "geth_peer_count",
    "geth_admin_peer_count",
    "geth_new_peers",
    "geth_disconnected_peers",
    "geth_peer_churn_count",
    "geth_rpc_latency_ms",
    "geth_rpc_requests_total",
    "geth_rpc_requests_per_sec",
    "geth_block_number",
    "geth_latest_block_age_secs",
    "system_cpu_usage_percent",
    "system_memory_used_mb",
    "system_network_rx_bytes_total",
    "system_network_tx_bytes_total",
    "system_network_rx_bytes_per_sec",
    "system_network_tx_bytes_per_sec",
    "system_tcp_established_connections",
    "beacon_finalized_epoch",
    "beacon_justified_epoch",
    "beacon_finality_distance_epochs",
    "beacon_head_slot",
    "beacon_sync_distance",
    "beacon_block_propagation_delay_ms",
    "validator_attestations_total",
    "validator_blocks_proposed_total",
    "validator_proposer_duties"
]


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


# =====================================================
# INFERENCE ENGINE
# =====================================================

class EVAGuardInference:

    def __init__(self):

        if not MODEL_FILE.exists():
            raise FileNotFoundError(MODEL_FILE)

        if not METADATA_FILE.exists():
            raise FileNotFoundError(METADATA_FILE)

        self.model = joblib.load(MODEL_FILE)

        with open(METADATA_FILE, "r") as f:
            self.metadata = json.load(f)

        self.features = self.metadata["features"]

        # Historique indépendant pour chaque nœud.
        self.history = {}

        print("EVA-Guard inference engine loaded")
        print("Model:", self.metadata["model"])
        print("Features:", len(self.features))
        print("Classes:", self.metadata["classes"])

    # -------------------------------------------------
    # RESET HISTORY
    # -------------------------------------------------

    def reset_node(self, node_id):

        self.history.pop(node_id, None)

    def reset_all(self):

        self.history.clear()

    # -------------------------------------------------
    # NORMALIZE RAW OBSERVATION
    # -------------------------------------------------

    def normalize_record(self, record):

        required = [
            "timestamp",
            "node_id"
        ]

        for col in required:
            if col not in record:
                raise ValueError(
                    f"Missing required field: {col}"
                )

        missing = [
            col for col in RAW_FEATURES
            if col not in record
        ]

        if missing:
            raise ValueError(
                f"Missing raw features: {missing}"
            )

        timestamp = pd.to_datetime(
            record["timestamp"],
            utc=True,
            errors="coerce"
        )

        if pd.isna(timestamp):
            raise ValueError("Invalid timestamp")

        values = {}

        for col in RAW_FEATURES:
            values[col] = pd.to_numeric(
                record[col],
                errors="coerce"
            )

        return (
            str(record["node_id"]),
            timestamp,
            values
        )

    # -------------------------------------------------
    # TEMPORAL FEATURES
    # -------------------------------------------------

    def build_temporal_features(
        self,
        node_id,
        timestamp,
        values
    ):

        features = values.copy()

        previous = self.history.get(node_id)

        interval = np.nan

        if previous is not None:

            interval = (
                timestamp - previous["timestamp"]
            ).total_seconds()

        valid_interval = (
            interval
            if 0 < interval <= MAX_INTERVAL_SEC
            else np.nan
        )

        previous_values = (
            previous["values"]
            if previous is not None
            and not pd.isna(valid_interval)
            else None
        )

        # Cumulative counters
        for col in COUNTER_COLUMNS:

            current = values[col]

            delta = np.nan

            if previous_values is not None:

                old = previous_values[col]

                if pd.notna(current) and pd.notna(old):

                    difference = current - old

                    if difference >= 0:
                        delta = difference

            features[f"{col}_delta"] = delta

            features[f"{col}_rate"] = (
                delta / valid_interval
                if pd.notna(delta)
                else np.nan
            )

        # Instantaneous measurements
        for col in GAUGE_COLUMNS:

            current = values[col]

            delta = np.nan

            if previous_values is not None:

                old = previous_values[col]

                if pd.notna(current) and pd.notna(old):
                    delta = current - old

            features[f"{col}_delta"] = delta

            features[f"{col}_change_per_sec"] = (
                delta / valid_interval
                if pd.notna(delta)
                else np.nan
            )

        # Update history after calculating temporal features.
        self.history[node_id] = {
            "timestamp": timestamp,
            "values": values.copy()
        }

        return features

    # -------------------------------------------------
    # INTERACTION FEATURES
    # -------------------------------------------------

    def build_interaction_features(self, features):

        features = features.copy()

        peer_count = features["geth_peer_count"]

        if pd.notna(peer_count) and peer_count != 0:

            features["rpc_requests_per_peer"] = (
                features["geth_rpc_requests_per_sec"]
                / peer_count
                if pd.notna(features["geth_rpc_requests_per_sec"])
                else np.nan
            )

            features["tcp_connections_per_peer"] = (
                features["system_tcp_established_connections"]
                / peer_count
                if pd.notna(
                    features["system_tcp_established_connections"]
                )
                else np.nan
            )

            features["peer_churn_ratio"] = (
                features["geth_peer_churn_count"]
                / peer_count
                if pd.notna(features["geth_peer_churn_count"])
                else np.nan
            )

        else:

            features["rpc_requests_per_peer"] = np.nan
            features["tcp_connections_per_peer"] = np.nan
            features["peer_churn_ratio"] = np.nan

        tx_rate = features["system_network_tx_bytes_per_sec"]
        rx_rate = features["system_network_rx_bytes_per_sec"]

        features["network_rx_tx_ratio"] = (
            rx_rate / tx_rate
            if pd.notna(rx_rate)
            and pd.notna(tx_rate)
            and tx_rate != 0
            else np.nan
        )

        rpc_rate = features["geth_rpc_requests_per_sec"]
        rpc_latency = features["geth_rpc_latency_ms"]

        features["rpc_load_latency_interaction"] = (
            rpc_rate * rpc_latency
            if pd.notna(rpc_rate)
            and pd.notna(rpc_latency)
            else np.nan
        )

        return features

    # -------------------------------------------------
    # BUILD MODEL INPUT
    # -------------------------------------------------

    def build_model_input(self, record):

        node_id, timestamp, values = (
            self.normalize_record(record)
        )

        features = self.build_temporal_features(
            node_id,
            timestamp,
            values
        )

        features = self.build_interaction_features(
            features
        )

        X = pd.DataFrame([features])

        # Respecter exactement l'ordre des features
        # utilisé pendant le training.
        X = X.reindex(
            columns=self.features
        )

        X = X.replace(
            [np.inf, -np.inf],
            np.nan
        )

        return X

    # -------------------------------------------------
    # PREDICTION
    # -------------------------------------------------

    def predict(self, record):

        X = self.build_model_input(record)

        prediction = self.model.predict(X)[0]

        probabilities = self.model.predict_proba(X)[0]

        classifier = self.model.named_steps["classifier"]

        classes = classifier.classes_

        probability_dict = {
            class_name: round(float(probability), 4)
            for class_name, probability in zip(
                classes,
                probabilities
            )
        }

        result = {
            "node_id": str(record["node_id"]),
            "timestamp": str(record["timestamp"]),
            "predicted_scenario": str(prediction),
            "probabilities": probability_dict
        }

        return result


# =====================================================
# OFFLINE INFERENCE TEST
# =====================================================

def test_inference():

    print("\n========== OFFLINE INFERENCE TEST ==========")

    df = pd.read_csv(DATA_FILE)

    df["timestamp"] = pd.to_datetime(
        df["timestamp"],
        utc=True
    )

    # Test uniquement sur session4.
    test_df = df[
        df["session"] == "session4"
    ].copy()

    test_df = test_df.sort_values(
        ["node_id", "timestamp"]
    )

    engine = EVAGuardInference()

    results = []

    for record in test_df.to_dict(orient="records"):

        result = engine.predict(record)

        result["actual_scenario"] = record["scenario"]

        result["correct"] = (
            result["predicted_scenario"]
            == record["scenario"]
        )

        results.append(result)

    results_df = pd.DataFrame(results)

    accuracy = results_df["correct"].mean()

    print("\nObservations:", len(results_df))
    print("Correct predictions:", results_df["correct"].sum())
    print("Accuracy:", round(accuracy, 4))

    print("\nSample predictions:")

    print(
        results_df[
            [
                "node_id",
                "actual_scenario",
                "predicted_scenario",
                "correct"
            ]
        ].head(15).to_string(index=False)
    )

    output = RESULTS_DIR / "inference_test.csv"

    results_df.to_csv(
        output,
        index=False
    )

    print("\nSaved:", output)


# =====================================================
# MAIN
# =====================================================

if __name__ == "__main__":

    test_inference()
