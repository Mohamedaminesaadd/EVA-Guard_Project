
import pandas as pd
import numpy as np


def detect_outliers_iqr(
    df: pd.DataFrame,
    columns: list[str]
) -> pd.DataFrame:
    """
    Detect potential outliers using the IQR method.
    Does not remove or modify any observations.
    """

    results = []

    for column in columns:

        if column not in df.columns:
            continue

        values = pd.to_numeric(
            df[column],
            errors="coerce"
        ).dropna()

        if values.empty:
            continue

        q1 = values.quantile(0.25)
        q3 = values.quantile(0.75)

        iqr = q3 - q1

        lower_bound = q1 - 1.5 * iqr
        upper_bound = q3 + 1.5 * iqr

        outliers = values[
            (values < lower_bound) |
            (values > upper_bound)
        ]

        results.append({
            "feature": column,
            "q1": q1,
            "q3": q3,
            "iqr": iqr,
            "lower_bound": lower_bound,
            "upper_bound": upper_bound,
            "outlier_count": len(outliers),
            "outlier_percentage": (
                len(outliers) / len(values) * 100
            ),
            "minimum": values.min(),
            "maximum": values.max()
        })

    return pd.DataFrame(results)


def investigate_outliers_by_scenario(
    df: pd.DataFrame,
    columns: list[str]
) -> pd.DataFrame:
    """
    Compare metric distributions across scenarios.
    """

    results = []

    for column in columns:

        if column not in df.columns:
            continue

        for scenario, group in df.groupby("scenario"):

            values = pd.to_numeric(
                group[column],
                errors="coerce"
            ).dropna()

            if values.empty:
                continue

            results.append({
                "feature": column,
                "scenario": scenario,
                "count": len(values),
                "mean": values.mean(),
                "median": values.median(),
                "std": values.std(),
                "min": values.min(),
                "max": values.max()
            })

    return pd.DataFrame(results)
