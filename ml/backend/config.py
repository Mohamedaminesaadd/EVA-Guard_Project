
import json
import os
from pathlib import Path


# ============================================================
# PROJECT PATHS
# ============================================================

BACKEND_DIR = Path(__file__).resolve().parent
ML_DIR = BACKEND_DIR.parent
PROJECT_ROOT = ML_DIR.parent

RUST_DIR = PROJECT_ROOT / "monitoring-rust"

DATA_DIR = ML_DIR / "data"
PROCESSED_DIR = DATA_DIR / "processed"
MODELS_DIR = ML_DIR / "models"
RESULTS_DIR = ML_DIR / "results"


# ============================================================
# RUST INPUT
# ============================================================

RUST_DATA_DIR = Path(
    os.getenv(
        "EVA_RUST_DATA_DIR",
        str(RUST_DIR / "data")
    )
)

LIVE_OBSERVATIONS_FILE = RUST_DATA_DIR / "live_observations.jsonl"


# ============================================================
# MACHINE LEARNING MODEL
# ============================================================

MODEL_FILE = Path(
    os.getenv(
        "EVA_MODEL_PATH",
        str(MODELS_DIR / "best_model.joblib")
    )
)

MODEL_METADATA_FILE = MODELS_DIR / "model_metadata.json"

FEATURE_CONFIG_FILE = PROCESSED_DIR / "feature_config.json"

FEATURE_ENGINEERING_CONFIG_FILE = (
    PROCESSED_DIR / "feature_engineering_config.json"
)


# ============================================================
# PREDICTION OUTPUT
# ============================================================

PREDICTIONS_FILE = RESULTS_DIR / "live_predictions.jsonl"


# ============================================================
# MONITORING CONFIGURATION
# ============================================================

COLLECTION_INTERVAL_SECONDS = 5

MAX_INTERVAL_SECONDS = 10

REQUIRED_COMPONENTS = {
    "geth",
    "beacon",
    "validator"
}

SUPPORTED_SCENARIOS = [
    "NORMAL",
    "ECLIPSE",
    "RPC_FLOODING"
]


# ============================================================
# FEATURE CONFIGURATION
# ============================================================

with open(FEATURE_CONFIG_FILE, "r", encoding="utf-8") as file:
    FEATURE_CONFIG = json.load(file)

with open(FEATURE_ENGINEERING_CONFIG_FILE, "r", encoding="utf-8") as file:
    FEATURE_ENGINEERING_CONFIG = json.load(file)

with open(MODEL_METADATA_FILE, "r", encoding="utf-8") as file:
    MODEL_METADATA = json.load(file)


FEATURE_COLUMNS = MODEL_METADATA["features"]

TARGET_COLUMN = MODEL_METADATA["target"]

MODEL_CLASSES = MODEL_METADATA["classes"]

SEGMENTATION_THRESHOLD_SECONDS = (
    FEATURE_ENGINEERING_CONFIG.get(
        "segmentation_threshold_seconds",
        MAX_INTERVAL_SECONDS
    )
)


# ============================================================
# VALIDATION
# ============================================================

def validate_config():

    required_paths = {
        "Live observations": LIVE_OBSERVATIONS_FILE,
        "Model": MODEL_FILE,
        "Model metadata": MODEL_METADATA_FILE,
        "Feature config": FEATURE_CONFIG_FILE,
        "Feature engineering config": FEATURE_ENGINEERING_CONFIG_FILE
    }

    for name, path in required_paths.items():
        if not path.exists():
            raise FileNotFoundError(
                f"{name} not found: {path}"
            )

    if len(FEATURE_COLUMNS) != 69:
        raise ValueError(
            f"Expected 69 features, got {len(FEATURE_COLUMNS)}"
        )

    if TARGET_COLUMN != "scenario":
        raise ValueError(
            f"Unexpected target column: {TARGET_COLUMN}"
        )

    if not MODEL_CLASSES:
        raise ValueError("Model classes are missing.")

    RESULTS_DIR.mkdir(parents=True, exist_ok=True)


if __name__ == "__main__":

    print("========== EVA-GUARD CONFIGURATION ==========")

    print("ML directory:", ML_DIR)
    print("Rust directory:", RUST_DIR)
    print("Live observations:", LIVE_OBSERVATIONS_FILE)
    print("Model:", MODEL_FILE)
    print("Predictions:", PREDICTIONS_FILE)

    print("Features:", len(FEATURE_COLUMNS))
    print("Classes:", MODEL_CLASSES)
    print("Collection interval:", COLLECTION_INTERVAL_SECONDS)
    print("Segmentation threshold:", SEGMENTATION_THRESHOLD_SECONDS)

    validate_config()

    print("\nConfiguration validated successfully.")
