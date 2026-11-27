
from pathlib import Path
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]
FILE = ROOT / "data/processed/dataset_clean.csv"

df = pd.read_csv(FILE)
df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True)

missing_columns = df.columns[df.isna().any()].tolist()

print("========== MISSING VALUES BY SESSION ==========")
print(df.groupby("session")[missing_columns].apply(
    lambda x: x.isna().sum()
))

print("\n========== MISSING VALUES BY SCENARIO ==========")
print(df.groupby("scenario")[missing_columns].apply(
    lambda x: x.isna().sum()
))

print("\n========== MISSING VALUES BY NODE ==========")
print(df.groupby("node_id")[missing_columns].apply(
    lambda x: x.isna().sum()
))

print("\n========== MISSING ROWS ==========")

for col in missing_columns:
    missing = df[df[col].isna()][
        ["timestamp", "session", "node_id", "scenario"]
    ]

    print(f"\n--- {col} ---")
    print("Missing:", len(missing))
    print(missing.head(15).to_string(index=False))

print("\n========== FIRST OBSERVATIONS PER NODE ==========")

for (session, node), group in df.groupby(["session", "node_id"]):
    group = group.sort_values("timestamp")

    print(f"\n{session} / {node}")
    print("First timestamp:", group["timestamp"].iloc[0])
    print("Missing counts:")
    print(group[missing_columns].isna().sum().to_dict())
