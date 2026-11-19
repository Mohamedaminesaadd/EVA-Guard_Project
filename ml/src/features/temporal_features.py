
import pandas as pd


def add_temporal_features(
    df: pd.DataFrame,
    metrics: list[str],
    group_columns: list[str] | None = None
) -> pd.DataFrame:
    """
    Create temporal features:
    - elapsed time
    - previous metric value
    - metric variation
    - metric percentage variation
    """

    data = df.copy()

    if group_columns is None:
        group_columns = ["node_id", "scenario"]

    data["timestamp"] = pd.to_datetime(
        data["timestamp"],
        utc=True,
        errors="coerce"
    )

    data = data.sort_values(
        group_columns + ["timestamp"]
    ).copy()

    groups = data.groupby(group_columns, sort=False)

    data["elapsed_seconds"] = (
        groups["timestamp"]
        .diff()
        .dt.total_seconds()
    )

    for metric in metrics:

        if metric not in data.columns:
            print(f"Skipping missing metric: {metric}")
            continue

        previous = groups[metric].shift(1)

        data[f"{metric}_previous"] = previous

        data[f"{metric}_delta"] = (
            data[metric] - previous
        )

        valid_previous = previous.notna() & (previous != 0)

        data[f"{metric}_pct_change"] = (
            (data[metric] - previous)
            .div(previous.where(valid_previous))
            * 100
        )

    return data
