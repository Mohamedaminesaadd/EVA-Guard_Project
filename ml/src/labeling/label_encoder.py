
from pathlib import Path

import pandas as pd

ML_ROOT = Path(__file__).resolve().parents[2]

INPUT_PATH = ML_ROOT / "data" / "processed" / "features_dataset.parquet"
OUTPUT_PATH = ML_ROOT / "data" / "processed" / "labeled_dataset.parquet"

LABEL_MAPPING = {
    "NORMAL": 0,
    "RPC_FLOODING": 1,
    "ECLIPSE": 2,
}


def verify_labels(df):
    if "scenario" not in df.columns:
        raise ValueError("Missing target column: scenario")

    if df["scenario"].isna().any():
        raise ValueError("Missing scenario labels detected.")

    actual = set(df["scenario"].unique())
    expected = set(LABEL_MAPPING.keys())

    if actual != expected:
        raise ValueError(
            f"Unexpected labels. Expected {expected}, got {actual}"
        )

    print("\nLabel distribution:")
    print(df["scenario"].value_counts())


def encode_labels(df):
    result = df.copy()
    result["label"] = result["scenario"].map(LABEL_MAPPING)

    if result["label"].isna().any():
        raise ValueError("Label encoding produced missing values.")

    return result


def main():
    df = pd.read_parquet(INPUT_PATH)

    verify_labels(df)

    labeled_df = encode_labels(df)

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    labeled_df.to_parquet(OUTPUT_PATH, index=False)

    print("\nEncoded labels:")
    print(labeled_df["label"].value_counts().sort_index())

    print(f"\nSaved dataset: {OUTPUT_PATH}")
    print("Dataset shape:", labeled_df.shape)


if __name__ == "__main__":
    main()
