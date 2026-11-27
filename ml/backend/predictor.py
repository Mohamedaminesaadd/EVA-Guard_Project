
from pathlib import Path
from typing import Any, Dict, List
import logging
import time

import joblib
import numpy as np
import pandas as pd

from .config import (
    MODEL_FILE,
    FEATURE_COLUMNS,
    MODEL_CLASSES,
    ML_DIAGNOSTIC_MODE,
    OOD_Z_THRESHOLD,
    NON_STATIONARY_FEATURES,
    NEUTRALIZE_NON_STATIONARY_FEATURES
)


logger = logging.getLogger(__name__)


class EVAGuardPredictor:
    """
    Loads the trained EVA-Guard sklearn pipeline
    and performs real-time attack classification.
    """

    OOD_WARNING_INTERVAL_SECONDS = 60

    def __init__(self, model_path: Path = MODEL_FILE):

        self.model_path = Path(model_path)

        self.model = None
        self.classes = None
        self.feature_columns = FEATURE_COLUMNS.copy()

        # Pipeline steps used only for diagnostics (scaled values).
        self._preprocessor = None
        self._classifier = None

        # Last out-of-distribution warning per node:
        # node_id -> (feature names, monotonic time).
        self._last_ood_by_node = {}

        self.neutralized_features = (
            [
                column
                for column in NON_STATIONARY_FEATURES
                if column in FEATURE_COLUMNS
            ]
            if NEUTRALIZE_NON_STATIONARY_FEATURES
            else []
        )

        self.load_model()

    # =====================================================
    # MODEL LOADING
    # =====================================================

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
                f"but configuration contains "
                f"{len(self.feature_columns)}"
            )

        model_feature_names = getattr(
            self.model,
            "feature_names_in_",
            None
        )

        if model_feature_names is not None:

            if list(model_feature_names) != self.feature_columns:
                raise ValueError(
                    "Feature order does not match "
                    "the trained model."
                )

        if not hasattr(self.model, "predict_proba"):
            raise ValueError(
                "The loaded model does not support predict_proba()."
            )

        self.classes = list(self.model.classes_)

        if self.classes != MODEL_CLASSES:
            raise ValueError(
                f"Model classes mismatch. "
                f"Expected {MODEL_CLASSES}, "
                f"got {self.classes}"
            )

        logger.info(
            "[PREDICTOR] Model loaded successfully."
        )

        logger.info(
            "[PREDICTOR] Model: %s",
            type(self.model).__name__
        )

        logger.info(
            "[PREDICTOR] Features: %d",
            len(self.feature_columns)
        )

        logger.info(
            "[PREDICTOR] Classes: %s",
            self.classes
        )

        steps = getattr(self.model, "steps", None)

        if steps and len(steps) >= 2:
            self._preprocessor = self.model[:-1]
            self._classifier = steps[-1][1]

        logger.info(
            "[PREDICTOR] Diagnostic mode: %s | OOD |z| threshold: %.1f",
            "ON" if ML_DIAGNOSTIC_MODE else "OFF",
            OOD_Z_THRESHOLD
        )

        if self.neutralized_features:
            logger.warning(
                "[PREDICTOR] Non-stationary features neutralized "
                "(imputed with training median): %s",
                self.neutralized_features
            )

    # =====================================================
    # INPUT PREPARATION
    # =====================================================

    def _prepare_input(
        self,
        features: Dict[str, Any]
    ) -> pd.DataFrame:
        """
        Prepare one observation using the exact
        feature order expected by the trained model.
        """

        if not isinstance(features, dict):
            raise TypeError(
                "Features must be provided as a dictionary."
            )

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
                    f"Invalid value for feature "
                    f"{column}: {value}"
                )

            if not np.isfinite(value):
                value = np.nan

            values[column] = value

        # Opt-in mitigation (see config.NON_STATIONARY_FEATURES).
        for column in self.neutralized_features:
            values[column] = np.nan

        X = pd.DataFrame(
            [values],
            columns=self.feature_columns
        )

        valid_count = int(
            X.notna().sum(axis=1).iloc[0]
        )

        if valid_count == 0:
            raise ValueError(
                "All 69 model features are NaN. "
                "Prediction aborted."
            )

        logger.debug(
            "[PREDICTOR] Valid features: %d/%d",
            valid_count,
            len(self.feature_columns)
        )

        return X

    # =====================================================
    # DIAGNOSTICS
    # =====================================================

    def _scaled_values(self, X: pd.DataFrame):
        """
        Values seen by the classifier (after SimpleImputer and
        StandardScaler). Returns None if the pipeline has no
        preprocessing steps.
        """

        if self._preprocessor is None:
            return None

        try:
            return np.asarray(
                self._preprocessor.transform(X),
                dtype=float
            )[0]

        except Exception:
            logger.debug(
                "[PREDICTOR] Unable to compute scaled values.",
                exc_info=True
            )
            return None

    def _log_diagnostics(
        self,
        X: pd.DataFrame,
        node_id: str,
        timestamp: str,
        predicted_class: str,
        probabilities: Dict[str, float],
        confidence: float
    ):
        row = X.iloc[0]

        missing = [
            column
            for column in self.feature_columns
            if pd.isna(row[column])
            and column not in self.neutralized_features
        ]

        scaled = self._scaled_values(X)

        out_of_distribution = []

        if scaled is not None:
            out_of_distribution = sorted(
                (
                    (column, float(z))
                    for column, z in zip(self.feature_columns, scaled)
                    if np.isfinite(z) and abs(z) > OOD_Z_THRESHOLD
                ),
                key=lambda item: -abs(item[1])
            )

        logger.info(
            "[PREDICTION] Node=%s | Timestamp=%s | Features=%d/%d "
            "(missing=%d, OOD=%d) | Prediction=%s | Confidence=%.4f | %s",
            node_id,
            timestamp,
            len(self.feature_columns) - len(missing)
            - len(self.neutralized_features),
            len(self.feature_columns),
            len(missing),
            len(out_of_distribution),
            predicted_class,
            confidence,
            " ".join(
                f"P({name})={value:.4f}"
                for name, value in probabilities.items()
            )
        )

        # Rate-limited warning per node: immediately when a new feature
        # leaves the training distribution, otherwise at most once per
        # OOD_WARNING_INTERVAL_SECONDS (avoids log flooding).
        ood_names = frozenset(name for name, _ in out_of_distribution)

        previous_names, previous_time = self._last_ood_by_node.get(
            node_id,
            (frozenset(), float("-inf"))
        )

        now = time.monotonic()

        if out_of_distribution and (
            not ood_names.issubset(previous_names)
            or now - previous_time >= self.OOD_WARNING_INTERVAL_SECONDS
        ):

            self._last_ood_by_node[node_id] = (ood_names, now)

            if out_of_distribution:
                logger.warning(
                    "[PREDICTOR] Node=%s | %d feature(s) outside the "
                    "training distribution (|z| > %.1f): %s",
                    node_id,
                    len(out_of_distribution),
                    OOD_Z_THRESHOLD,
                    ", ".join(
                        f"{name}={row[name]:.6g} (z={z:+.1f})"
                        for name, z in out_of_distribution
                    )
                )

        if not ML_DIAGNOSTIC_MODE:
            return

        logger.info(
            "[DIAGNOSTIC] Node=%s | Missing features: %s",
            node_id,
            missing
        )

        logger.info(
            "[DIAGNOSTIC] Node=%s | Features: %s",
            node_id,
            {
                column: (
                    None if pd.isna(row[column])
                    else float(row[column])
                )
                for column in self.feature_columns
            }
        )

        coefficients = getattr(self._classifier, "coef_", None)

        if scaled is None or coefficients is None:
            return

        class_index = self.classes.index(predicted_class)

        contributions = sorted(
            zip(
                self.feature_columns,
                scaled,
                np.asarray(coefficients)[class_index] * scaled
            ),
            key=lambda item: -abs(item[2])
        )

        logger.info(
            "[DIAGNOSTIC] Node=%s | Top logit contributions to %s: %s",
            node_id,
            predicted_class,
            ", ".join(
                f"{name} (z={z:+.2f}, contrib={value:+.2f})"
                for name, z, value in contributions[:8]
            )
        )

    # =====================================================
    # SINGLE PREDICTION
    # =====================================================

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
            raise RuntimeError(
                "Model is not loaded."
            )

        X = self._prepare_input(features)

        predicted_class = self.model.predict(X)[0]

        probabilities_array = self.model.predict_proba(X)[0]

        probabilities = {
            str(class_name): float(probability)
            for class_name, probability in zip(
                self.classes,
                probabilities_array
            )
        }

        result = {
            "timestamp": timestamp,
            "node_id": node_id,
            "prediction": str(predicted_class),
            "confidence": float(
                np.max(probabilities_array)
            ),
            "probabilities": probabilities
        }

        try:
            self._log_diagnostics(
                X,
                node_id=node_id,
                timestamp=timestamp,
                predicted_class=str(predicted_class),
                probabilities=probabilities,
                confidence=result["confidence"]
            )

        except Exception:
            # Diagnostics must never break inference.
            logger.debug(
                "[PREDICTOR] Diagnostic logging failed.",
                exc_info=True
            )

        return result

    # =====================================================
    # BATCH PREDICTION
    # =====================================================

    def predict_batch(
        self,
        observations: List[Dict[str, Any]]
    ) -> List[Dict[str, Any]]:
        """
        Predict multiple node observations independently.
        """

        results = []

        for observation in observations:

            if "features" not in observation:
                raise ValueError(
                    "Observation does not contain 'features'."
                )

            result = self.predict(
                features=observation["features"],
                node_id=observation.get("node_id"),
                timestamp=observation.get("timestamp")
            )

            results.append(result)

        return results

    # =====================================================
    # MODEL INFORMATION
    # =====================================================

    def get_model_info(self) -> Dict[str, Any]:
        """
        Return basic information about the loaded model.
        """

        if self.model is None:
            raise RuntimeError(
                "Model is not loaded."
            )

        return {
            "model_type": type(self.model).__name__,
            "model_path": str(self.model_path),
            "feature_count": len(self.feature_columns),
            "features": self.feature_columns.copy(),
            "classes": self.classes.copy()
        }
