
from pathlib import Path
import json

import pandas as pd


ROOT = Path(__file__).resolve().parents[1]

INPUT_FILE = ROOT / "data/raw/dataset_merged.csv"
OUTPUT_DIR = ROOT / "data/processed"

TARGET = "scenario"
METADATA = ["timestamp", "node_id", "session"]

EXCLUDED_FEATURES = [
    "beacon_attestation_inclusion_distance"
]


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    df = pd.read_csv(INPUT_FILE)

    print("Initial dataset:", df.shape)

    # Validate mandatory columns
    required = METADATA + [TARGET]
    missing = set(required) - set(df.columns)

    if missing:
        raise ValueError(f"Missing columns: {missing}")

    # Remove rows without target
    df = df.dropna(subset=[TARGET]).copy()

    # Extract metadata and target
    metadata = df[METADATA + [TARGET]].copy()

    X = df.drop(columns=METADATA + [TARGET]).copy()

    # Convert boolean columns
    bool_columns = X.select_dtypes(
        include=["bool"]
    ).columns.tolist()

    X[bool_columns] = X[bool_columns].astype("int8")

    # Remove explicitly excluded features
    excluded = [
        col for col in EXCLUDED_FEATURES
        if col in X.columns
    ]

    X = X.drop(columns=excluded)

    # Remove columns with 100% missing values
    all_missing = X.columns[
        X.isna().all()
    ].tolist()

    X = X.drop(columns=all_missing)

    # Remove constant features
    constant_features = [
        col for col in X.columns
        if X[col].nunique(dropna=True) <= 1
    ]

    X = X.drop(columns=constant_features)

    # Validate remaining feature types
    non_numeric = X.select_dtypes(
        exclude=["number"]
    ).columns.tolist()

    if non_numeric:
        raise TypeError(f"Non-numeric columns: {non_numeric}")

    # Build cleaned dataset
    cleaned = pd.concat(
        [
            metadata.reset_index(drop=True),
            X.reset_index(drop=True)
        ],
        axis=1
    )

    cleaned["timestamp"] = pd.to_datetime(
        cleaned["timestamp"],
        utc=True,
        errors="coerce"
    )

    cleaned = cleaned.sort_values(
        ["session", "node_id", "timestamp"]
    ).reset_index(drop=True)

    # Export
    output_file = OUTPUT_DIR / "dataset_clean.csv"
    cleaned.to_csv(output_file, index=False)

    # Feature configuration
    config = {
        "target": TARGET,
        "metadata": METADATA,
        "features": X.columns.tolist(),
        "excluded_explicitly": excluded,
        "removed_all_missing": all_missing,
        "removed_constant": constant_features,
        "boolean_converted": bool_columns
    }

    with open(
        OUTPUT_DIR / "feature_config.json",
        "w",
        encoding="utf-8"
    ) as f:
        json.dump(config, f, indent=4)

    print("\n========== PREPROCESSING REPORT ==========")
    print("Initial rows:", len(df))
    print("Final shape:", cleaned.shape)
    print("Features retained:", X.shape[1])
    print("Explicitly excluded:", excluded)
    print("All-missing removed:", len(all_missing))
    print("Constant removed:", len(constant_features))
    print("Boolean converted:", bool_columns)

    print("\nRemaining missing values:")
    print(
        X.isna().sum()
        .loc[lambda s: s > 0]
        .sort_values(ascending=False)
    )

    print("\nTarget distribution:")
    print(cleaned[TARGET].value_counts())

    print("\nSaved:", output_file)


if __name__ == "__main__":
    main()
