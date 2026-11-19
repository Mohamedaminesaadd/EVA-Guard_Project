
from pathlib import Path
import pandas as pd

# Locate the ML project root
ML_ROOT = Path(__file__).resolve().parents[2]

DATASET_PATH = (
    ML_ROOT / "data" / "interim" / "synchronized_dataset.parquet"
)

print("Dataset path:", DATASET_PATH)

# Check that the file exists
if not DATASET_PATH.exists():
    raise FileNotFoundError(
        f"Dataset not found: {DATASET_PATH}\n"
        "Run the preprocessing pipeline first."
    )

# Load dataset
df = pd.read_parquet(DATASET_PATH)

print("\nDataset shape:", df.shape)

print("\nAvailable columns:")
for column in df.columns:
    print(column)

# Inspect important metric groups
keywords = [
    "rpc",
    "peer",
    "attestation",
    "proposal",
    "sync",
    "cpu",
    "memory",
    "txpool",
    "network"
]

for keyword in keywords:

    print(f"\n========== {keyword.upper()} ==========")

    matching_columns = [
        col for col in df.columns
        if keyword in col.lower()
    ]

    for col in matching_columns:
        print(col)
