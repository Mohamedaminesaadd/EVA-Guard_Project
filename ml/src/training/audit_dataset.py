
from pathlib import Path
import json

import numpy as np
import pandas as pd

ML_ROOT = Path(__file__).resolve().parents[2]
SPLIT_DIR = ML_ROOT / "data" / "splits"
REPORT_DIR = ML_ROOT / "reports" / "training"

FILES = ["train", "validation", "test"]


def audit_dataset():
    REPORT_DIR.mkdir(parents=True, exist_ok=True)

    datasets = {
        name: pd.read_parquet(SPLIT_DIR / f"{name}.parquet")
        for name in FILES
    }

    train = datasets["train"]

    X = train.drop(columns=["label"])
    report = {}

    # Missing values
    missing = X.isna().sum()
    report["missing_columns"] = missing[missing > 0].to_dict()

    # Infinite values
    numeric = X.select_dtypes(include="number")
    infinite = np.isinf(numeric).sum()
    report["infinite_columns"] = infinite[infinite > 0].to_dict()

    # Constant features
    constant = [
        col for col in X.columns
        if X[col].nunique(dropna=False) <= 1
    ]
    report["constant_features"] = constant

    # Duplicate feature columns
    duplicate_features = X.columns[X.T.duplicated()].tolist()
    report["duplicate_features"] = duplicate_features

    # Highly correlated features
    corr = numeric.corr().abs()
    upper = corr.where(
        np.triu(np.ones(corr.shape), k=1).astype(bool)
    )

    correlated_pairs = []

    for column in upper.columns:
        for other in upper.index:
            value = upper.loc[other, column]

            if pd.notna(value) and value >= 0.95:
                correlated_pairs.append({
                    "feature_1": other,
                    "feature_2": column,
                    "correlation": float(value),
                })

    report["highly_correlated_pairs"] = correlated_pairs

    # Target leakage check
    forbidden = {
        "scenario",
        "label",
        "timestamp",
        "node_id",
        "collection_interval",
        "collection_interval_x",
        "collection_interval_y",
    }

    report["forbidden_features_found"] = [
        col for col in X.columns if col in forbidden
    ]

    # Feature counts
    report["feature_count"] = len(X.columns)

    with (REPORT_DIR / "feature_audit.json").open(
        "w", encoding="utf-8"
    ) as file:
        json.dump(report, file, indent=4, default=str)

    print("\n===== FEATURE AUDIT =====")
    print("Feature count:", report["feature_count"])
    print("Missing columns:", len(report["missing_columns"]))
    print("Infinite columns:", len(report["infinite_columns"]))
    print("Constant features:", len(constant))
    print("Duplicate features:", len(duplicate_features))
    print("Highly correlated pairs:", len(correlated_pairs))
    print("Forbidden features:", report["forbidden_features_found"])

    print("\nReport saved:", REPORT_DIR / "feature_audit.json")


if __name__ == "__main__":
    audit_dataset()
