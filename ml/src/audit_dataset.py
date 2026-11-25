
from pathlib import Path
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]
DATASET_PATH = ROOT / "data/raw/dataset_merged.csv"
REPORT_DIR = ROOT / "data/processed"

TARGET = "scenario"

df = pd.read_csv(DATASET_PATH)
REPORT_DIR.mkdir(parents=True, exist_ok=True)

print("\n========== DATASET ==========")
print("Shape:", df.shape)
print("Memory:", round(df.memory_usage(deep=True).sum() / 1024**2, 2), "MB")

print("\n========== TARGET DISTRIBUTION ==========")
print(df[TARGET].value_counts(dropna=False))

print("\n========== SESSION DISTRIBUTION ==========")
print(pd.crosstab(df["session"], df[TARGET]))

print("\n========== NODE DISTRIBUTION ==========")
print(pd.crosstab(df["node_id"], df[TARGET]))

print("\n========== DUPLICATES ==========")
keys = ["timestamp", "node_id", "session"]
print("Duplicate observations:", df.duplicated(keys).sum())

print("\n========== MISSING VALUES ==========")
missing = pd.DataFrame({
    "missing_count": df.isna().sum(),
    "missing_percent": df.isna().mean() * 100
}).sort_values("missing_percent", ascending=False)

print(missing[missing["missing_count"] > 0].to_string())

print("\n========== CONSTANT FEATURES ==========")
excluded = {"timestamp", "node_id", "session", TARGET}

constant = [
    col for col in df.columns
    if col not in excluded and df[col].nunique(dropna=True) <= 1
]
print(constant)

print("\n========== TIMESTAMP VALIDATION ==========")
df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True, errors="coerce")
print("Invalid timestamps:", df["timestamp"].isna().sum())

print("\n========== SAVING REPORTS ==========")
missing.to_csv(REPORT_DIR / "missing_values.csv")

pd.crosstab(
    df["session"], df[TARGET]
).to_csv(REPORT_DIR / "class_distribution_by_session.csv")

print("Reports saved in:", REPORT_DIR)
