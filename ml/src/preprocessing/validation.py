
import pandas as pd


REQUIRED_COLUMNS = [
    "timestamp",
    "node_id",
    "scenario",
    "collection_interval",
    "component",
]

VALID_SCENARIOS = {
    "NORMAL",
    "RPC_FLOODING",
    "ECLIPSE",
}

VALID_COMPONENTS = {
    "geth",
    "beacon",
    "validator",
}


def validate_dataset(df: pd.DataFrame) -> dict:
    """
    Validate the structure and basic integrity
    of the flattened monitoring dataset.
    """

    report = {}

    # 1. Required columns
    missing_columns = [
        col for col in REQUIRED_COLUMNS
        if col not in df.columns
    ]

    report["missing_columns"] = missing_columns

    if missing_columns:
        raise ValueError(
            f"Missing required columns: {missing_columns}"
        )

    # 2. Missing essential values
    report["missing_timestamp"] = int(df["timestamp"].isna().sum())
    report["missing_node_id"] = int(df["node_id"].isna().sum())
    report["missing_scenario"] = int(df["scenario"].isna().sum())
    report["missing_component"] = int(df["component"].isna().sum())

    # 3. Validate labels
    report["unknown_scenarios"] = sorted(
        set(df["scenario"].dropna().unique()) - VALID_SCENARIOS
    )

    report["unknown_components"] = sorted(
        set(df["component"].dropna().unique()) - VALID_COMPONENTS
    )

    # 4. Validate timestamps
    timestamps = pd.to_datetime(
        df["timestamp"],
        utc=True,
        errors="coerce"
    )

    report["invalid_timestamps"] = int(timestamps.isna().sum())

    # 5. Validate collection intervals
    intervals = pd.to_numeric(
        df["collection_interval"],
        errors="coerce"
    )

    report["invalid_collection_intervals"] = int(
        (intervals.isna() | (intervals <= 0)).sum()
    )

    # 6. Duplicate observations
    report["exact_duplicates"] = int(
        df.drop(columns=["_source_file"], errors="ignore")
        .duplicated()
        .sum()
    )

    return report
