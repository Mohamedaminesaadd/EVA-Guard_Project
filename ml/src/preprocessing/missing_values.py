
import pandas as pd
from pathlib import Path


def analyze_missing_values(df: pd.DataFrame) -> pd.DataFrame:
    """
    Analyze missing values in the flattened dataset.
    """

    report = pd.DataFrame({
        "missing_count": df.isna().sum(),
        "missing_percentage": df.isna().mean() * 100,
        "data_type": df.dtypes.astype(str)
    })

    report = report.sort_values(
        by="missing_percentage",
        ascending=False
    )

    return report


def analyze_missing_by_component(
    df: pd.DataFrame
) -> pd.DataFrame:
    """
    Analyze missing values separately for Geth,
    Beacon and Validator.
    """

    results = []

    for component in df["component"].dropna().unique():

        component_df = df[df["component"] == component]

        for column in df.columns:

            if column in ["component", "scenario"]:
                continue

            missing_count = component_df[column].isna().sum()

            results.append({
                "component": component,
                "column": column,
                "missing_count": int(missing_count),
                "missing_percentage": (
                    component_df[column].isna().mean() * 100
                )
            })

    return pd.DataFrame(results)


def save_missing_reports(
    global_report: pd.DataFrame,
    component_report: pd.DataFrame,
    output_dir: Path
):
    output_dir.mkdir(parents=True, exist_ok=True)

    global_report.to_csv(
        output_dir / "missing_values_global.csv"
    )

    component_report.to_csv(
        output_dir / "missing_values_by_component.csv",
        index=False
    )
