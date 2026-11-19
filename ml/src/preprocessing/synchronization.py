
import pandas as pd


def synchronize_observations(
    df: pd.DataFrame,
    tolerance: str = "2s"
) -> pd.DataFrame:
    """
    Synchronize Geth, Beacon and Validator observations.

    Observations are grouped by node and scenario.
    The nearest timestamps are matched within the tolerance.
    """

    data = df.copy()

    data["timestamp"] = pd.to_datetime(
        data["timestamp"],
        utc=True,
        errors="coerce"
    )

    data = data.dropna(
        subset=["timestamp", "node_id", "component", "scenario"]
    )

    # Each component must have at most one observation
    # for a given node, scenario and timestamp.
    duplicate_keys = [
        "node_id",
        "scenario",
        "timestamp",
        "component"
    ]

    if data.duplicated(duplicate_keys).any():
        raise ValueError(
            "Duplicate component observations detected. "
            "Investigate duplicates before synchronization."
        )

    # Expand nested metrics and system dictionaries.
    metrics = pd.json_normalize(
        data["metrics"].apply(
            lambda x: x if isinstance(x, dict) else {}
        )
    ).add_prefix("metrics_")

    system = pd.json_normalize(
        data["system"].apply(
            lambda x: x if isinstance(x, dict) else {}
        )
    ).add_prefix("system_")

    metadata = data[
        [
            "timestamp",
            "node_id",
            "scenario",
            "component",
            "collection_interval"
        ]
    ].reset_index(drop=True)

    expanded = pd.concat(
        [metadata, metrics, system],
        axis=1
    )

    # Prefix each component's metrics.
    component_frames = {}

    for component in expanded["component"].unique():

        component_df = expanded[
            expanded["component"] == component
        ].copy()

        component_df = component_df.drop(
            columns=["component"]
        )

        feature_columns = [
            col for col in component_df.columns
            if col not in [
                "timestamp",
                "node_id",
                "scenario",
                "collection_interval"
            ]
        ]

        component_df = component_df.rename(
            columns={
                col: f"{component}_{col}"
                for col in feature_columns
            }
        )

        component_frames[component] = component_df

    # Use Geth as the reference timeline.
    if "geth" not in component_frames:
        raise ValueError("No Geth observations found.")

    result = component_frames["geth"].copy()

    # Match Beacon and Validator to the nearest Geth timestamp.
    for component in ["beacon", "validator"]:

        if component not in component_frames:
            continue

        other = component_frames[component].copy()

        result = pd.merge_asof(
            result.sort_values("timestamp"),
            other.sort_values("timestamp"),
            on="timestamp",
            by=["node_id", "scenario"],
            direction="nearest",
            tolerance=pd.Timedelta(tolerance)
        )

    return result.reset_index(drop=True)
