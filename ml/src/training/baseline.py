from pathlib import Path
import json

import joblib
import pandas as pd

from sklearn.dummy import DummyClassifier
from sklearn.metrics import (
    accuracy_score,
    precision_score,
    recall_score,
    f1_score,
    classification_report,
    confusion_matrix,
)

ML_ROOT = Path(__file__).resolve().parents[2]

SPLIT_DIR = ML_ROOT / "data" / "splits"
MODEL_DIR = ML_ROOT / "models"
REPORT_DIR = ML_ROOT / "reports" / "training"

TRAIN_PATH = SPLIT_DIR / "train.parquet"
VALIDATION_PATH = SPLIT_DIR / "validation.parquet"

MODEL_PATH = MODEL_DIR / "baseline_model.joblib"
REPORT_PATH = REPORT_DIR / "baseline_report.json"
CONFUSION_PATH = REPORT_DIR / "baseline_confusion_matrix.csv"

TARGET = "label"

LABEL_NAMES = {
    0: "NORMAL",
    1: "RPC_FLOODING",
    2: "ECLIPSE",
}


def load_datasets():
    train_df = pd.read_parquet(TRAIN_PATH)
    validation_df = pd.read_parquet(VALIDATION_PATH)

    if TARGET not in train_df or TARGET not in validation_df:
        raise ValueError("Missing target column: label")

    X_train = train_df.drop(columns=[TARGET])
    y_train = train_df[TARGET]

    X_validation = validation_df.drop(columns=[TARGET])
    y_validation = validation_df[TARGET]

    if list(X_train.columns) != list(X_validation.columns):
        raise ValueError("Train and validation features do not match.")

    return X_train, y_train, X_validation, y_validation


def main():
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    REPORT_DIR.mkdir(parents=True, exist_ok=True)

    X_train, y_train, X_validation, y_validation = load_datasets()

    print("Train shape:", X_train.shape)
    print("Validation shape:", X_validation.shape)

    # Baseline: always predicts the most frequent training class
    model = DummyClassifier(strategy="most_frequent")

    model.fit(X_train, y_train)

    predictions = model.predict(X_validation)

    results = {
        "model": "DummyClassifier",
        "strategy": "most_frequent",
        "accuracy": float(accuracy_score(y_validation, predictions)),
        "precision_macro": float(
            precision_score(y_validation, predictions,
                            average="macro", zero_division=0)
        ),
        "recall_macro": float(
            recall_score(y_validation, predictions,
                         average="macro", zero_division=0)
        ),
        "f1_macro": float(
            f1_score(y_validation, predictions,
                     average="macro", zero_division=0)
        ),
        "classification_report": classification_report(
            y_validation,
            predictions,
            labels=[0, 1, 2],
            target_names=[
                LABEL_NAMES[0],
                LABEL_NAMES[1],
                LABEL_NAMES[2],
            ],
            output_dict=True,
            zero_division=0,
        ),
    }

    matrix = confusion_matrix(
        y_validation,
        predictions,
        labels=[0, 1, 2],
    )

    joblib.dump(model, MODEL_PATH)

    with REPORT_PATH.open("w", encoding="utf-8") as file:
        json.dump(results, file, indent=4)

    pd.DataFrame(
        matrix,
        index=["actual_NORMAL", "actual_RPC_FLOODING", "actual_ECLIPSE"],
        columns=["pred_NORMAL", "pred_RPC_FLOODING", "pred_ECLIPSE"],
    ).to_csv(CONFUSION_PATH)

    print("\n===== BASELINE RESULTS =====")
    print("Accuracy:", results["accuracy"])
    print("Precision macro:", results["precision_macro"])
    print("Recall macro:", results["recall_macro"])
    print("F1 macro:", results["f1_macro"])

    print("\nClassification report:")
    print(
        classification_report(
            y_validation,
            predictions,
            labels=[0, 1, 2],
            target_names=[
                LABEL_NAMES[0],
                LABEL_NAMES[1],
                LABEL_NAMES[2],
            ],
            zero_division=0,
        )
    )

    print("Model saved:", MODEL_PATH)
    print("Report saved:", REPORT_PATH)
    print("Confusion matrix saved:", CONFUSION_PATH)


if __name__ == "__main__":
    main()