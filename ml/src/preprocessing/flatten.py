import pandas as pd


def flatten_observations(df: pd.DataFrame) -> pd.DataFrame:
    """
    Convert nested monitoring observations
    into a flat tabular DataFrame.
    """

    flat_df = pd.json_normalize(
        df.to_dict(orient="records"),
        sep="_"
    )

    return flat_df