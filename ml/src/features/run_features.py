
from pathlib import Path
import sys
import json
import pandas as pd

# --------------------------------------------------
# PROJECT CONFIGURATION
# --------------------------------------------------

ML_ROOT = Path(__file__).resolve().parents[2]

sys.path.insert(0, str(ML_ROOT))

INPUT_PATH = (
    ML_ROOT / "data" / "interim" /
    "synchronized_dataset.parquet"
)

OUTPUT_DIR = ML_ROOT / "data" / "processed"
REPORT_DIR = ML_ROOT / "reports" / "features"

OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
REPORT_DIR.mkdir(parents=True, exist_ok=True)

OUTPUT_PATH = OUTPUT_DIR / "features_dataset.parquet"


# --------------------------------------------------
# IMPORT FEATURE MODULES
# --------------------------------------------------

from src.features.temporal_features import add_temporal_features
from src.features.network_features import add_network_features
from src.features.validator_features import add_validator_features
from src.features.system_features import add_system_features

from src.features.feature_validation import (
    analyze_feature_quality,
    detect_constant_features,
    detect_high_missing_features,
    detect_correlated_features,
    prepare_xy
)


# --------------------------------------------------
# MAIN PIPELINE
# --------------------------------------------------

def run_feature_pipeline():

    print("\n" + "=" * 60)
    print("EVA-GUARD - FEATURE ENGINEERING PIPELINE")
    print("=" * 60)

    # STEP 1: LOAD DATASET
    print("\n[1/6] Loading synchronized dataset...")

    if not INPUT_PATH.exists():
        raise FileNotFoundError(
            f"Input dataset not found: {INPUT_PATH}"
        )

    df = pd.read_parquet(INPUT_PATH)

    print("Input shape:", df.shape)

    features_df = df.copy()

    # STEP 2: TEMPORAL FEATURES
    print("\n[2/6] Creating temporal features...")

    temporal_metrics = [
        "geth_metrics_peer_count",
        "geth_metrics_rpc_latency_ms",
        "geth_system_cpu_usage_percent",
        "beacon_metrics_peer_count",
        "validator_metrics_missed_attestations_total"
    ]

    features_df = add_temporal_features(
        features_df,
        metrics=temporal_metrics
    )

    # STEP 3: NETWORK FEATURES
    print("\n[3/6] Creating network features...")

    features_df = add_network_features(features_df)

    # STEP 4: VALIDATOR FEATURES
    print("\n[4/6] Creating validator features...")

    features_df = add_validator_features(features_df)

    # STEP 5: SYSTEM FEATURES
    print("\n[5/6] Creating system features...")

    features_df = add_system_features(features_df)

    print("Feature dataset shape:", features_df.shape)

    # STEP 6: FEATURE VALIDATION
    print("\n[6/6] Validating generated features...")

    quality_report = analyze_feature_quality(features_df)

    constant_features = detect_constant_features(features_df)

    high_missing_features = detect_high_missing_features(
        features_df,
        threshold=50
    )

    correlation_report = detect_correlated_features(
        features_df,
        threshold=0.95
    )

    # Save reports
    quality_report.to_csv(
        REPORT_DIR / "feature_quality_report.csv",
        index=False
    )

    correlation_report.to_csv(
        REPORT_DIR / "correlation_report.csv",
        index=False
    )

    validation_summary = {
        "input_rows": len(df),
        "output_rows": len(features_df),
        "input_columns": len(df.columns),
        "output_columns": len(features_df.columns),
        "generated_features": len(features_df.columns) - len(df.columns),
        "constant_features": constant_features,
        "high_missing_features": high_missing_features,
        "highly_correlated_pairs": len(correlation_report)
    }

    with open(
        REPORT_DIR / "feature_validation.json",
        "w",
        encoding="utf-8"
    ) as file:
        json.dump(validation_summary, file, indent=4)

    # Save final feature dataset
    features_df.to_parquet(
        OUTPUT_PATH,
        index=False
    )

    # Prepare X and y for future ML training
    X, y = prepare_xy(features_df)

    print("\n" + "=" * 60)
    print("FEATURE ENGINEERING COMPLETED")
    print("=" * 60)

    print("Input shape:", df.shape)
    print("Output shape:", features_df.shape)
    print("Generated features:", len(features_df.columns) - len(df.columns))
    print("X shape:", X.shape)
    print("y shape:", y.shape)

    print("\nScenario distribution:")
    print(y.value_counts())

    print("\nConstant features:", len(constant_features))
    print("High missing features:", len(high_missing_features))
    print("Highly correlated pairs:", len(correlation_report))

    print("\nSaved dataset:", OUTPUT_PATH)
    print("Saved reports:", REPORT_DIR)

    return features_df, X, y


if __name__ == "__main__":
    run_feature_pipeline()
