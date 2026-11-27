
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
# INFERENCE DIAGNOSTICS
# ============================================================

def _env_flag(name: str, default: bool = False) -> bool:
    value = os.getenv(name)

    if value is None:
        return default

    return value.strip().lower() in {"1", "true", "yes", "on"}


# Verbose per-prediction dump (69 values, scaled values and top
# logit contributions). Disabled by default in production.
ML_DIAGNOSTIC_MODE = _env_flag("EVA_ML_DIAGNOSTIC", False)

# A feature whose StandardScaler output exceeds this absolute value
# lies far outside the training distribution (|z| = 10 means ten
# training standard deviations away from the training mean).
OOD_Z_THRESHOLD = float(os.getenv("EVA_OOD_Z_THRESHOLD", "10"))

# Absolute "chain progress" features.
#
# They only grow with the age of the chain / node uptime. In the
# training sessions the scenarios were always recorded in the order
# NORMAL -> ECLIPSE -> RPC_FLOODING, so these values are confounded
# with the label (data leakage). Once the live chain is older than the
# training chain (e.g. justified epoch 126 vs 0-7 in training), their
# scaled values reach |z| ~ 40-95 and saturate the LogisticRegression
# towards ECLIPSE.
#
# Evidence (session4 test set): imputing these features with the
# model's own SimpleImputer medians keeps accuracy at 95.74 %
# (95.56 % with them), i.e. they carry no genuine attack signal.
NON_STATIONARY_FEATURES = [
    "geth_block_number",
    "beacon_head_slot",
    "beacon_finalized_epoch",
    "beacon_justified_epoch",
    "validator_attestations_total",
    "validator_blocks_proposed_total",
    "system_memory_used_mb",
]

# Opt-in mitigation until the model is retrained without these
# features: replace them by NaN so that the trained SimpleImputer
# substitutes its training median. Disabled by default so that the
# model input stays strictly identical to the training preprocessing.
NEUTRALIZE_NON_STATIONARY_FEATURES = _env_flag(
    "EVA_NEUTRALIZE_NON_STATIONARY_FEATURES",
    False
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
