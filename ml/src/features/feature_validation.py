
import pandas as pd
import numpy as np


TARGET_COLUMN = "scenario"

METADATA_COLUMNS = [
    "timestamp",
    "node_id",
    "scenario",
    "collection_interval",
    "collection_interval_x",
    "collection_interval_y",
]


def get_feature_columns(df: pd.DataFrame) -> list:
    """
    Return numeric feature columns, excluding metadata and target.
    """

    excluded = set(METADATA_COLUMNS)

    return [
        col for col in df.select_dtypes(include=np.number).columns
        if col not in excluded
    ]


def analyze_feature_quality(df: pd.DataFrame) -> pd.DataFrame:
    """
    Analyze missing values, unique values and variance.
    """

    features = get_feature_columns(df)
    report = []

    for feature in features:

        values = pd.to_numeric(
            df[feature],
            errors="coerce"
        )

        report.append({
            "feature": feature,
            "missing_count": int(values.isna().sum()),
            "missing_percentage": float(values.isna().mean() * 100),
            "unique_values": int(values.nunique()),
            "variance": float(values.var()) if values.notna().any() else np.nan,
            "minimum": values.min(),
            "maximum": values.max()
        })

    return pd.DataFrame(report).sort_values(
        "missing_percentage",
        ascending=False
    )


def detect_constant_features(df: pd.DataFrame) -> list:
    """
    Detect numeric features with only one unique value.
    """

    features = get_feature_columns(df)

    return [
        col for col in features
        if df[col].nunique(dropna=True) <= 1
    ]


def detect_high_missing_features(
    df: pd.DataFrame,
    threshold: float = 50.0
) -> list:
    """
    Identify features whose missing percentage exceeds threshold.
    """

    features = get_feature_columns(df)

    return [
        col for col in features
        if df[col].isna().mean() * 100 > threshold
    ]


def detect_correlated_features(
    df: pd.DataFrame,
    threshold: float = 0.95
) -> pd.DataFrame:
    """
    Find highly correlated numeric feature pairs.
    """

    features = get_feature_columns(df)

    numeric_df = df[features].apply(
        pd.to_numeric,
        errors="coerce"
    )

    correlation = numeric_df.corr()

    upper_triangle = correlation.where(
        np.triu(
            np.ones(correlation.shape),
            k=1
        ).astype(bool)
    )

    pairs = []

    for column in upper_triangle.columns:

        for index in upper_triangle.index:

            value = upper_triangle.loc[index, column]

            if pd.notna(value) and abs(value) >= threshold:

                pairs.append({
                    "feature_1": index,
                    "feature_2": column,
                    "correlation": value
                })

    return pd.DataFrame(pairs)


def prepare_xy(df: pd.DataFrame):
    """
    Separate numeric input features X and target y.
    """

    if TARGET_COLUMN not in df.columns:
        raise ValueError(
            f"Target column '{TARGET_COLUMN}' not found."
        )

    features = get_feature_columns(df)

    X = df[features].copy()
    y = df[TARGET_COLUMN].copy()

    return X, y
