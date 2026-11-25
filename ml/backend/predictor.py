
from pathlib import Path
from typing import Any, Dict, List

import joblib
import numpy as np
import pandas as pd

from .config import (
    MODEL_FILE,
    FEATURE_COLUMNS,
    MODEL_CLASSES
)


class EVAGuardPredictor:
    """
    Loads the trained EVA-Guard model and performs
    real-time attack classification.
    """

    def __init__(self, model_path: Path = MODEL_FILE):

        self.model_path = Path(model_path)

        self.model = None
        self.classes = None
        self.feature_columns = FEATURE_COLUMNS.copy()

        self.load_model()

    def load_model(self):
        """
        Load and validate the trained sklearn pipeline.
        """

        if not self.model_path.exists():
            raise FileNotFoundError(
                f"Model not found: {self.model_path}"
            )

        self.model = joblib.load(self.model_path)

        expected_features = getattr(
            self.model,
            "n_features_in_",
            None
        )

        if expected_features != len(self.feature_columns):
            raise ValueError(
                f"Model expects {expected_features} features, "
                f"but configuration contains {len(self.feature_columns)}"
            )

        model_feature_names = getattr(
            self.model,
            "feature_names_in_",
            None
        )

        if model_feature_names is not None:

            if list(model_feature_names) != self.feature_columns:
                raise ValueError(
                    "Feature order does not match the trained model."
                )

        self.classes = list(self.model.classes_)

        if self.classes != MODEL_CLASSES:
            raise ValueError(
                f"Model classes mismatch. "
                f"Expected {MODEL_CLASSES}, got {self.classes}"
            )

        if not hasattr(self.model, "predict_proba"):
            raise ValueError(
                "The loaded model does not support predict_proba()."
            )

        print("[PREDICTOR] Model loaded successfully.")
        print("[PREDICTOR] Model:", type(self.model).__name__)
        print("[PREDICTOR] Features:", len(self.feature_columns))
        print("[PREDICTOR] Classes:", self.classes)

    def _prepare_input(
        self,
        features: Dict[str, Any]
    ) -> pd.DataFrame:
        """
        Prepare one observation in the exact model feature order.
        """

        missing = [
            column
            for column in self.feature_columns
            if column not in features
        ]

        if missing:
            raise ValueError(
                f"Missing model features: {missing}"
            )

        values = {}

        for column in self.feature_columns:

            value = features[column]

            if value is None:
                value = np.nan

            try:
                value = float(value)
            except (TypeError, ValueError):
                raise ValueError(
                    f"Invalid value for feature {column}: {value}"
                )

            if np.isinf(value):
                value = np.nan

            values[column] = value

        return pd.DataFrame(
            [values],
            columns=self.feature_columns
        )

    def predict(
        self,
        features: Dict[str, Any],
        node_id: str = None,
        timestamp: str = None
    ) -> Dict[str, Any]:
        """
        Predict the scenario for one node.
        """

        if self.model is None:
            raise RuntimeError("Model is not loaded.")

        X = self._prepare_input(features)

        predicted_class = self.model.predict(X)[0]

        probabilities_array = self.model.predict_proba(X)[0]

        probabilities = {
            class_name: float(probability)
            for class_name, probability in zip(
                self.classes,
                probabilities_array
            )
        }

        return {
            "timestamp": timestamp,
            "node_id": node_id,
            "prediction": str(predicted_class),
            "confidence": float(max(probabilities_array)),
            "probabilities": probabilities
        }

    def predict_batch(
        self,
        observations: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:
        """
        Predict multiple node observations.
        """

        results = []

        for observation in observations:

            features = observation["features"]

            result = self.predict(
                features=features,
                node_id=observation.get("node_id"),
                timestamp=observation.get("timestamp")
            )

            results.append(result)

        return results
