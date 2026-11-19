
from src.ingestion.jsonl_loader import load_all_datasets


if __name__ == "__main__":

    df = load_all_datasets()

    print("\n========== DATASET PREVIEW ==========")

    print(df.head())

    print("\n========== SHAPE ==========")

    print(df.shape)

    print("\n========== COLUMNS ==========")

    print(df.columns.tolist())

    print("\n========== SCENARIOS ==========")

    if "scenario" in df.columns:
        print(df["scenario"].value_counts())

    print("\n========== SOURCE FILES ==========")

    print(df["_source_file"].value_counts())

    print("\n========== COMPONENTS ==========")

    if "component" in df.columns:
        print(df["component"].value_counts())
