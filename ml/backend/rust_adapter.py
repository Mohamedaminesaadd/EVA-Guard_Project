
from typing import Any, Dict, List

import numpy as np

from .config import FEATURE_CONFIG


class RustAdapter:
    """
    Converts aggregated Rust observations into the
    raw ML feature format used during training.
    """

    def __init__(self):

        self.feature_columns = FEATURE_CONFIG["features"]

        if len(self.feature_columns) != 26:
            raise ValueError(
                f"Expected 26 raw features, "
                f"got {len(self.feature_columns)}"
            )

    @staticmethod
    def _to_numeric(value: Any) -> float:
        """
        Convert a value to float.
        Missing and invalid values become NaN.
        """

        if value is None:
            return np.nan

        if isinstance(value, bool):
            return float(value)

        try:
            result = float(value)

            if not np.isfinite(result):
                return np.nan

            return result

        except (TypeError, ValueError):
            return np.nan

    def adapt(
        self,
        observation: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        Convert one aggregated observation into
        the 26 raw ML features.
        """

        if not isinstance(observation, dict):
            raise TypeError("Observation must be a dictionary.")

        adapted = {
            "timestamp": observation.get("timestamp"),
            "node_id": observation.get("node_id")
        }

        for column in self.feature_columns:

            value = observation.get(column)

            adapted[column] = self._to_numeric(value)

        self._validate(adapted)

        return adapted

    def adapt_many(
        self,
        observations: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:
        """
        Adapt multiple aggregated observations.
        """

        return [
            self.adapt(observation)
            for observation in observations
        ]

    def _validate(
        self,
        observation: Dict[str, Any]
    ) -> None:
        """
        Validate feature names and structure.
        """

        missing_columns = [
            column
            for column in self.feature_columns
            if column not in observation
        ]

        if missing_columns:
            raise ValueError(
                f"Missing features: {missing_columns}"
            )

        if not observation.get("node_id"):
            raise ValueError("node_id is missing.")

        if not observation.get("timestamp"):
            raise ValueError("timestamp is missing.")

    def get_feature_columns(self) -> List[str]:

        return self.feature_columns.copy()
