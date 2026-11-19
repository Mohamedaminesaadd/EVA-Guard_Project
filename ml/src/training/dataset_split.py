
from pathlib import Path
import json

import pandas as pd

ML_ROOT = Path(__file__).resolve().parents[2]

INPUT_PATH = ML_ROOT / "data" / "processed" / "labeled_dataset.parquet"
OUTPUT_DIR = ML_ROOT / "data" / "splits"

TARGET = "label"

# These columns are metadata, not model inputs.
EXCLUDED_COLUMNS = {
    "scenario",
    "label",
    "timestamp",
    "node_id",
    "collection_interval",
    "collection_interval_x",
    "collection_interval_y",
    "elapsed_seconds",
}

TRAIN_RATIO = 0.60
VALIDATION_RATIO = 0.20


def chronological_split(df):
    train_parts = []
    validation_parts = []
    test_parts = []

    for scenario, group in df.groupby("scenario"):
        group = group.copy()

        group["timestamp"] = pd.to_datetime(
            group["timestamp"], utc=True, errors="coerce"
        )

        if group["timestamp"].isna().any():
            raise ValueError(f"Invalid timestamps in {scenario}")

        timestamps = sorted(group["timestamp"].unique())

        if len(timestamps) < 3:
            raise ValueError(
                f"Not enough timestamps for scenario {scenario}"
            )

        n = len(timestamps)
        train_end = int(n * TRAIN_RATIO)
        validation_end = int(
            n * (TRAIN_RATIO + VALIDATION_RATIO)
        )

        train_times = timestamps[:train_end]
        validation_times = timestamps[train_end:validation_end]
        test_times = timestamps[validation_end:]

        train_parts.append(
            group[group["timestamp"].isin(train_times)]
        )

        validation_parts.append(
            group[group["timestamp"].isin(validation_times)]
        )

        test_parts.append(
            group[group["timestamp"].isin(test_times)]
        )

        print(f"\nScenario: {scenario}")
        print("Total timestamps:", n)
        print("Train timestamps:", len(train_times))
        print("Validation timestamps:", len(validation_times))
        print("Test timestamps:", len(test_times))

    train_df = pd.concat(train_parts, ignore_index=True)
    validation_df = pd.concat(validation_parts, ignore_index=True)
    test_df = pd.concat(test_parts, ignore_index=True)

    return train_df, validation_df, test_df


def prepare_features(df):
    excluded = EXCLUDED_COLUMNS

    feature_columns = [
        col
        for col in df.select_dtypes(include="number").columns
        if col not in excluded
    ]

    if not feature_columns:
        raise ValueError("No numerical features available.")

    X = df[feature_columns].copy()
    y = df[TARGET].copy()

    return X, y, feature_columns


def validate_splits(train, validation, test):
    splits = {
        "train": train,
        "validation": validation,
        "test": test,
    }

    print("\n===== SPLIT VALIDATION =====")

    for name, df in splits.items():
        print(f"\n{name.upper()}:")
        print("Shape:", df.shape)
        print("Class distribution:")
        print(df[TARGET].value_counts().sort_index())

    # Ensure chronological partitions do not share timestamps
    train_times = set(train["timestamp"])
    validation_times = set(validation["timestamp"])
    test_times = set(test["timestamp"])

    assert train_times.isdisjoint(validation_times)
    assert train_times.isdisjoint(test_times)
    assert validation_times.isdisjoint(test_times)

    print("\nNo shared timestamps between splits.")


def main():
    df = pd.read_parquet(INPUT_PATH)

    required = {"timestamp", "scenario", TARGET}
    missing = required - set(df.columns)

    if missing:
        raise ValueError(f"Missing required columns: {missing}")

    if df[TARGET].isna().any():
        raise ValueError("Missing target labels.")

    train_df, validation_df, test_df = chronological_split(df)

    validate_splits(train_df, validation_df, test_df)

    X_train, y_train, features = prepare_features(train_df)
    X_validation, y_validation, _ = prepare_features(validation_df)
    X_test, y_test, _ = prepare_features(test_df)

    # Ensure identical feature columns and order
    X_validation = X_validation[features]
    X_test = X_test[features]

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    X_train.assign(label=y_train).to_parquet(
        OUTPUT_DIR / "train.parquet", index=False
    )

    X_validation.assign(label=y_validation).to_parquet(
        OUTPUT_DIR / "validation.parquet", index=False
    )

    X_test.assign(label=y_test).to_parquet(
        OUTPUT_DIR / "test.parquet", index=False
    )

    manifest = {
        "strategy": "chronological_split_per_scenario",
        "train_ratio": TRAIN_RATIO,
        "validation_ratio": VALIDATION_RATIO,
        "test_ratio": 1 - TRAIN_RATIO - VALIDATION_RATIO,
        "feature_count": len(features),
        "features": features,
        "label_mapping": {
            "NORMAL": 0,
            "RPC_FLOODING": 1,
            "ECLIPSE": 2,
        },
    }

    with (OUTPUT_DIR / "split_manifest.json").open(
        "w", encoding="utf-8"
    ) as file:
        json.dump(manifest, file, indent=4)

    print("\n===== DATASET PREPARATION COMPLETED =====")
    print("Features:", len(features))
    print("Train:", X_train.shape)
    print("Validation:", X_validation.shape)
    print("Test:", X_test.shape)
    print("Output:", OUTPUT_DIR)


if __name__ == "__main__":
    main()
