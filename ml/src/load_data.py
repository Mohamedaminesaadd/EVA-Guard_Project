
from pathlib import Path
import json
import pandas as pd


ROOT_DIR = Path(__file__).resolve().parents[2]
DATA_DIR = ROOT_DIR / "monitoring-rust" / "data"
OUTPUT_DIR = ROOT_DIR / "ml" / "data" / "raw"

GROUP_COLUMNS = ["timestamp", "node_id", "scenario"]


def load_jsonl_files(data_dir: Path) -> pd.DataFrame:
    records = []

    files = sorted(data_dir.glob("session*/*.jsonl"))

    if not files:
        raise FileNotFoundError(
            f"No JSONL files found in {data_dir}"
        )

    for file_path in files:
        session = file_path.parent.name

        with file_path.open("r", encoding="utf-8") as f:
            for line_number, line in enumerate(f, start=1):
                if not line.strip():
                    continue

                try:
                    record = json.loads(line)
                except json.JSONDecodeError as exc:
                    print(
                        f"[WARNING] Invalid JSON: "
                        f"{file_path}:{line_number} - {exc}"
                    )
                    continue

                record["session"] = session
                records.append(record)

    if not records:
        raise ValueError("No valid JSON records found.")

    return pd.DataFrame(records)


def flatten_records(df: pd.DataFrame) -> pd.DataFrame:
    rows = []

    for _, record in df.iterrows():
        component = record.get("component")

        row = {
            "timestamp": record.get("timestamp"),
            "node_id": record.get("node_id"),
            "scenario": record.get("scenario"),
            "session": record.get("session"),
            "component": component,
        }

        metrics = record.get("metrics")
        system = record.get("system")

        if isinstance(metrics, dict):
            for key, value in metrics.items():
                row[f"{component}_{key}"] = value

        if isinstance(system, dict):
            for key, value in system.items():
                row[f"system_{key}"] = value

        rows.append(row)

    return pd.DataFrame(rows)


def merge_components(df: pd.DataFrame) -> pd.DataFrame:
    feature_columns = [
        col for col in df.columns
        if col not in GROUP_COLUMNS + ["session", "component"]
    ]

    merged = (
        df.groupby(GROUP_COLUMNS + ["session"], dropna=False)[feature_columns]
        .first()
        .reset_index()
    )

    return merged


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print("[1] Loading JSONL files...")
    raw_df = load_jsonl_files(DATA_DIR)

    print(f"Loaded records: {len(raw_df)}")
    print("Components:")
    print(raw_df["component"].value_counts(dropna=False))

    print("\n[2] Flattening records...")
    flat_df = flatten_records(raw_df)

    print(f"Flattened records: {len(flat_df)}")

    print("\n[3] Merging components...")
    dataset = merge_components(flat_df)

    dataset["timestamp"] = pd.to_datetime(
        dataset["timestamp"],
        errors="coerce",
        utc=True
    )

    dataset = dataset.sort_values(
        ["session", "node_id", "timestamp"]
    ).reset_index(drop=True)

    output_file = OUTPUT_DIR / "dataset_merged.csv"
    dataset.to_csv(output_file, index=False)

    print("\n[4] Dataset summary")
    print(f"Shape: {dataset.shape}")
    print(f"Sessions: {dataset['session'].nunique()}")
    print(f"Nodes: {dataset['node_id'].nunique()}")
    print("\nClass distribution:")
    print(dataset["scenario"].value_counts(dropna=False))

    print(f"\nMissing values: {dataset.isna().sum().sum()}")
    print(f"\nSaved to: {output_file}")


if __name__ == "__main__":
    main()
