
from pathlib import Path
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]

INPUT = ROOT / "data/processed/dataset_clean.csv"
OUTPUT = ROOT / "data/processed/dataset_final.csv"

TEMPORAL_FEATURES = [
    "geth_new_peers",
    "geth_disconnected_peers",
    "geth_peer_churn_count",
    "geth_rpc_requests_per_sec",
    "system_network_rx_bytes_per_sec",
    "system_network_tx_bytes_per_sec",
]

GROUP_COLUMNS = ["session", "node_id", "scenario"]


def main():
    df = pd.read_csv(INPUT)

    df["timestamp"] = pd.to_datetime(
        df["timestamp"], utc=True
    )

    print("Initial shape:", df.shape)

    # Verify that missing temporal metrics occur on the same rows
    missing_matrix = df[TEMPORAL_FEATURES].isna()

    print("\nMissing count per feature:")
    print(missing_matrix.sum())

    print(
        "\nRows where all temporal features are missing:",
        missing_matrix.all(axis=1).sum()
    )

    print(
        "Rows where only some temporal features are missing:",
        (missing_matrix.any(axis=1) & ~missing_matrix.all(axis=1)).sum()
    )

    # Sort before removing the first observation of each scenario
    df = df.sort_values(
        GROUP_COLUMNS + ["timestamp"]
    ).reset_index(drop=True)

    # Remove first observation only when all temporal metrics are missing
    first_observation = (
        df.groupby(GROUP_COLUMNS, sort=False)
          .cumcount()
          .eq(0)
    )

    rows_to_remove = first_observation & missing_matrix.all(axis=1)

    print("\nFirst observations removed:", rows_to_remove.sum())

    df = df.loc[~rows_to_remove].copy()

    # Validate remaining missing values
    print("\nRemaining missing values:")
    print(df[TEMPORAL_FEATURES].isna().sum())

    if df[TEMPORAL_FEATURES].isna().any().any():
        raise ValueError(
            "Unexpected missing values remain in temporal features."
        )

    df = df.sort_values(
        ["session", "node_id", "timestamp"]
    ).reset_index(drop=True)

    df.to_csv(OUTPUT, index=False)

    print("\nFinal shape:", df.shape)
    print("\nClass distribution:")
    print(df["scenario"].value_counts())

    print("\nSaved:", OUTPUT)


if __name__ == "__main__":
    main()
