
from pathlib import Path
import sys

# Resolve project directories
ML_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = ML_ROOT.parent

sys.path.insert(0, str(ML_ROOT))

from src.ingestion.jsonl_loader import load_all_datasets
from src.preprocessing.flatten import flatten_observations
from src.preprocessing.validation import validate_dataset
from src.preprocessing.missing_values import (
    analyze_missing_values,
    analyze_missing_by_component,
    save_missing_reports
)
from src.preprocessing.outliers import (
    detect_outliers_iqr,
    investigate_outliers_by_scenario
)
from src.preprocessing.synchronization import synchronize_observations


DATA_DIR = ML_ROOT / "data"
INTERIM_DIR = DATA_DIR / "interim"
REPORT_DIR = ML_ROOT / "reports" / "preprocessing"

INTERIM_DIR.mkdir(parents=True, exist_ok=True)
REPORT_DIR.mkdir(parents=True, exist_ok=True)


def run_preprocessing():

    print("\n" + "=" * 60)
    print("EVA-GUARD - PREPROCESSING PIPELINE")
    print("=" * 60)

    # --------------------------------------------------
    # STEP 1: DATA INGESTION
    # --------------------------------------------------

    print("\n[1/6] Loading raw datasets...")

    df = load_all_datasets()

    print("Raw dataset shape:", df.shape)

    # --------------------------------------------------
    # STEP 2: FLATTEN NESTED JSON
    # --------------------------------------------------

    print("\n[2/6] Flattening nested observations...")

    flat_df = flatten_observations(df)

    print("Flattened dataset shape:", flat_df.shape)

    # --------------------------------------------------
    # STEP 3: DATA VALIDATION
    # --------------------------------------------------

    print("\n[3/6] Validating dataset...")

    validation_report = validate_dataset(flat_df)

    for key, value in validation_report.items():
        print(f"{key}: {value}")

    # Save validation report
    import json

    with open(
        REPORT_DIR / "validation_report.json",
        "w",
        encoding="utf-8"
    ) as file:
        json.dump(validation_report, file, indent=4)

    # --------------------------------------------------
    # STEP 4: MISSING VALUE ANALYSIS
    # --------------------------------------------------

    print("\n[4/6] Analyzing missing values...")

    missing_report = analyze_missing_values(flat_df)

    component_missing_report = analyze_missing_by_component(
        flat_df
    )

    save_missing_reports(
        missing_report,
        component_missing_report,
        REPORT_DIR
    )

    print("Missing-value report saved.")

    # --------------------------------------------------
    # STEP 5: OUTLIER INVESTIGATION
    # --------------------------------------------------

    print("\n[5/6] Investigating outliers...")

    numeric_columns = flat_df.select_dtypes(
        include="number"
    ).columns.tolist()

    metric_columns = [
        col for col in numeric_columns
        if col.startswith(("metrics_", "system_"))
    ]

    outlier_report = detect_outliers_iqr(
        flat_df,
        metric_columns
    )

    scenario_report = investigate_outliers_by_scenario(
        flat_df,
        metric_columns
    )

    outlier_report.to_csv(
        REPORT_DIR / "outlier_report.csv",
        index=False
    )

    scenario_report.to_csv(
        REPORT_DIR / "outliers_by_scenario.csv",
        index=False
    )

    print("Outlier reports saved.")

    # --------------------------------------------------
    # STEP 6: TEMPORAL SYNCHRONIZATION
    # --------------------------------------------------

    print("\n[6/6] Synchronizing monitoring components...")

    synchronized_df = synchronize_observations(
        df,
        tolerance="2s"
    )

    print("Synchronized dataset shape:", synchronized_df.shape)

    # --------------------------------------------------
    # SAVE RESULTS
    # --------------------------------------------------

    output_path = INTERIM_DIR / "synchronized_dataset.parquet"

    synchronized_df.to_parquet(
        output_path,
        index=False
    )

    print("\n" + "=" * 60)
    print("PREPROCESSING COMPLETED")
    print("=" * 60)

    print("Raw observations:", len(df))
    print("Flattened observations:", len(flat_df))
    print("Synchronized observations:", len(synchronized_df))
    print("Output:", output_path)

    return synchronized_df


if __name__ == "__main__":
    run_preprocessing()
