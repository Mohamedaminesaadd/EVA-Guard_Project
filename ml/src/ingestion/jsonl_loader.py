
import json
from pathlib import Path

import pandas as pd


# Path to the datasets directory
PROJECT_ROOT = Path(__file__).resolve().parents[3]
DATASET_DIR = PROJECT_ROOT / "datasets"


def load_jsonl(file_path: Path) -> list:
    """
    Load JSONL observations from one dataset file.
    """

    records = []

    with file_path.open("r", encoding="utf-8") as file:

        for line_number, line in enumerate(file, start=1):

            if not line.strip():
                continue

            try:
                record = json.loads(line)

                # Keep track of the original dataset
                record["_source_file"] = file_path.name

                records.append(record)

            except json.JSONDecodeError as error:

                print(
                    f"Invalid JSON: {file_path.name}, "
                    f"line {line_number}: {error}"
                )

    return records


def load_all_datasets() -> pd.DataFrame:
    """
    Load all JSONL datasets.
    """

    all_records = []

    files = sorted(DATASET_DIR.glob("*.jsonl"))

    if not files:
        raise FileNotFoundError(
            f"No JSONL datasets found in {DATASET_DIR}"
        )

    for file_path in files:

        records = load_jsonl(file_path)

        print(
            f"{file_path.name}: "
            f"{len(records)} observations"
        )

        all_records.extend(records)

    df = pd.DataFrame(all_records)

    print("\nTotal observations:", len(df))

    return df
