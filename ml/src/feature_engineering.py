
from pathlib import Path
import json

import numpy as np
import pandas as pd


ROOT = Path(__file__).resolve().parents[1]

WARMUP_FILE = ROOT / "data/processed/dataset_warmup_5.csv"
CLEAN_FILE = ROOT / "data/processed/dataset_clean.csv"

OUTPUT_FILE = ROOT / "data/processed/dataset_features.csv"
CONFIG_FILE = ROOT / "data/processed/feature_engineering_config.json"

GROUP_COLUMNS = ["session", "node_id"]

# Les observations normales sont espacées de 5 secondes.
# Une interruption de 20 secondes marque une nouvelle séquence.
MAX_INTERVAL_SEC = 10


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


METADATA_COLUMNS = [
    "timestamp",
    "node_id",
    "session",
    "scenario"
]


def load_dataset():

    if WARMUP_FILE.exists():
        input_file = WARMUP_FILE
    else:
        input_file = CLEAN_FILE

    df = pd.read_csv(input_file)

    print("Dataset utilisé :", input_file)

    return df


def create_temporal_segments(df):

    df = df.copy()

    df["timestamp"] = pd.to_datetime(
        df["timestamp"],
        utc=True,
        errors="coerce"
    )

    df = df.sort_values(
        GROUP_COLUMNS + ["timestamp"]
    ).reset_index(drop=True)

    grouped = df.groupby(GROUP_COLUMNS, sort=False)

    df["observation_interval_sec"] = (
        grouped["timestamp"]
        .diff()
        .dt.total_seconds()
    )

    # Nouvelle séquence après une interruption supérieure à 10 sec.
    df["new_segment"] = (
        df["observation_interval_sec"].isna()
        | (df["observation_interval_sec"] > MAX_INTERVAL_SEC)
        | (df["observation_interval_sec"] <= 0)
    )

    df["segment_id"] = (
        df.groupby(GROUP_COLUMNS)["new_segment"]
        .cumsum()
    )

    return df


def calculate_temporal_features(df):

    df = df.copy()

    temporal_groups = df.groupby(
        GROUP_COLUMNS + ["segment_id"],
        sort=False
    )

    valid_interval = df["observation_interval_sec"].where(
        df["observation_interval_sec"].between(
            0,
            MAX_INTERVAL_SEC,
            inclusive="neither"
        )
    )

    # Compteurs cumulatifs
    for col in COUNTER_COLUMNS:

        if col not in df.columns:
            continue

        delta_col = f"{col}_delta"
        rate_col = f"{col}_rate"

        delta = temporal_groups[col].diff()

        # Détecter les resets des compteurs.
        delta = delta.where(delta >= 0)

        df[delta_col] = delta

        df[rate_col] = (
            delta / valid_interval
        )

    # Mesures instantanées
    for col in GAUGE_COLUMNS:

        if col not in df.columns:
            continue

        delta_col = f"{col}_delta"
        rate_col = f"{col}_change_per_sec"

        delta = temporal_groups[col].diff()

        df[delta_col] = delta

        df[rate_col] = (
            delta / valid_interval
        )

    return df


def calculate_interaction_features(df):

    df = df.copy()

    # Charge RPC par peer.
    # Si peer_count = 0, le ratio est indéfini.
    if {
        "geth_rpc_requests_per_sec",
        "geth_peer_count"
    }.issubset(df.columns):

        df["rpc_requests_per_peer"] = (
            df["geth_rpc_requests_per_sec"]
            / df["geth_peer_count"].replace(0, np.nan)
        )

    # Connexions TCP par peer.
    if {
        "system_tcp_established_connections",
        "geth_peer_count"
    }.issubset(df.columns):

        df["tcp_connections_per_peer"] = (
            df["system_tcp_established_connections"]
            / df["geth_peer_count"].replace(0, np.nan)
        )

    # Trafic entrant / sortant.
    if {
        "system_network_rx_bytes_per_sec",
        "system_network_tx_bytes_per_sec"
    }.issubset(df.columns):

        df["network_rx_tx_ratio"] = (
            df["system_network_rx_bytes_per_sec"]
            / df["system_network_tx_bytes_per_sec"].replace(0, np.nan)
        )

    # Interaction entre charge RPC et latence.
    if {
        "geth_rpc_requests_per_sec",
        "geth_rpc_latency_ms"
    }.issubset(df.columns):

        df["rpc_load_latency_interaction"] = (
            df["geth_rpc_requests_per_sec"]
            * df["geth_rpc_latency_ms"]
        )

    # Churn relatif au nombre de peers.
    if {
        "geth_peer_churn_count",
        "geth_peer_count"
    }.issubset(df.columns):

        df["peer_churn_ratio"] = (
            df["geth_peer_churn_count"]
            / df["geth_peer_count"].replace(0, np.nan)
        )

    return df


def validate_dataset(df, feature_columns):

    print("\n========== VALIDATION ==========")

    print("\nShape :", df.shape)

    print("\nDistribution des scénarios :")
    print(df["scenario"].value_counts())

    print("\nNombre de features :", len(feature_columns))

    print("\nValeurs manquantes :")

    missing = df[feature_columns].isna().sum()

    print(missing[missing > 0])

    print("\nNombre de séquences :")
    print(df.groupby(GROUP_COLUMNS + ["segment_id"]).ngroups)

    print("\nObservations par séquence :")
    print(
        df.groupby(GROUP_COLUMNS + ["segment_id"])
        .size()
        .value_counts()
        .sort_index()
    )

    print("\nIntervalles temporels :")
    print(df["observation_interval_sec"].describe())


def main():

    df = load_dataset()

    initial_shape = df.shape

    print("\nShape initiale :", initial_shape)

    # 1. Créer les séquences temporelles.
    df = create_temporal_segments(df)

    # 2. Calculer les deltas et les taux.
    df = calculate_temporal_features(df)

    # 3. Créer les ratios et interactions.
    df = calculate_interaction_features(df)

    # 4. Nettoyer les valeurs infinies.
    df = df.replace([np.inf, -np.inf], np.nan)

    # Colonnes numériques utilisables par le modèle.
    feature_columns = [
        col for col in df.columns
        if col not in METADATA_COLUMNS
        and col not in [
            "observation_interval_sec",
            "new_segment",
            "segment_id"
        ]
        and pd.api.types.is_numeric_dtype(df[col])
    ]

    if "scenario" not in df.columns:
        raise ValueError("La colonne scenario est absente.")

    # 5. Exporter le dataset.
    df.to_csv(OUTPUT_FILE, index=False)

    # 6. Exporter la configuration.
    config = {
        "input_shape": list(initial_shape),
        "output_shape": list(df.shape),
        "target": "scenario",
        "metadata_columns": METADATA_COLUMNS,
        "group_columns": GROUP_COLUMNS,
        "segmentation_threshold_seconds": MAX_INTERVAL_SEC,
        "feature_columns": feature_columns
    }

    with open(CONFIG_FILE, "w") as f:
        json.dump(config, f, indent=4)

    # 7. Validation.
    validate_dataset(df, feature_columns)

    print("\nFichiers générés :")
    print(OUTPUT_FILE)
    print(CONFIG_FILE)


if __name__ == "__main__":
    main()
