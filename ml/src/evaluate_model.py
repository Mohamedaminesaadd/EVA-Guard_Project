
from pathlib import Path
import json

import joblib
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns

from sklearn.metrics import (
    accuracy_score,
    precision_score,
    recall_score,
    f1_score,
    classification_report,
    confusion_matrix
)

from sklearn.inspection import permutation_importance


# =====================================================
# PATHS
# =====================================================

ROOT = Path(__file__).resolve().parents[1]

DATA_FILE = ROOT / "data/processed/dataset_features.csv"

MODEL_FILE = ROOT / "models/best_model.joblib"
METADATA_FILE = ROOT / "models/model_metadata.json"

RESULTS_DIR = ROOT / "results"
RESULTS_DIR.mkdir(exist_ok=True)

TEST_SESSION = "session4"
RANDOM_STATE = 42


# =====================================================
# LOAD DATA AND MODEL
# =====================================================

def load_data_and_model():

    print("\n========== LOADING DATA AND MODEL ==========")

    df = pd.read_csv(DATA_FILE)

    with open(METADATA_FILE, "r") as f:
        metadata = json.load(f)

    model = joblib.load(MODEL_FILE)

    features = metadata["features"]
    target = metadata["target"]

    print("Dataset shape:", df.shape)
    print("Model:", metadata["model"])
    print("Number of features:", len(features))
    print("Target:", target)

    return df, model, metadata, features, target


# =====================================================
# PREPARE TEST DATA
# =====================================================

def prepare_test_data(df, features, target):

    test_df = df[
        df["session"] == TEST_SESSION
    ].copy()

    if test_df.empty:
        raise ValueError(
            f"No observations found for {TEST_SESSION}"
        )

    X_test = test_df[features].copy()
    y_test = test_df[target].copy()

    X_test = X_test.replace(
        [np.inf, -np.inf],
        np.nan
    )

    print("\n========== TEST DATA ==========")

    print("Test session:", TEST_SESSION)
    print("Test shape:", X_test.shape)

    print("\nClass distribution:")
    print(y_test.value_counts())

    return test_df, X_test, y_test


# =====================================================
# PREDICTIONS
# =====================================================

def predict(model, X_test):

    print("\n========== PREDICTIONS ==========")

    y_pred = model.predict(X_test)

    print("Predictions completed:", len(y_pred))

    return y_pred


# =====================================================
# CLASSIFICATION REPORT
# =====================================================

def evaluate_classification(y_test, y_pred):

    print("\n========== CLASSIFICATION REPORT ==========")

    report = classification_report(
        y_test,
        y_pred,
        output_dict=True,
        zero_division=0
    )

    report_df = pd.DataFrame(report).transpose()

    print(report_df.round(4))

    report_df.to_csv(
        RESULTS_DIR / "classification_report.csv"
    )

    with open(
        RESULTS_DIR / "classification_report.json",
        "w"
    ) as f:
        json.dump(report, f, indent=4)

    metrics = {
        "accuracy": accuracy_score(y_test, y_pred),

        "precision_macro": precision_score(
            y_test,
            y_pred,
            average="macro",
            zero_division=0
        ),

        "recall_macro": recall_score(
            y_test,
            y_pred,
            average="macro",
            zero_division=0
        ),

        "f1_macro": f1_score(
            y_test,
            y_pred,
            average="macro",
            zero_division=0
        )
    }

    print("\nGlobal metrics:")

    for name, value in metrics.items():
        print(f"{name}: {value:.4f}")

    pd.DataFrame([metrics]).to_csv(
        RESULTS_DIR / "evaluation_metrics.csv",
        index=False
    )

    return report_df, metrics


# =====================================================
# CONFUSION MATRIX
# =====================================================

def evaluate_confusion_matrix(y_test, y_pred):

    print("\n========== CONFUSION MATRIX ==========")

    labels = sorted(y_test.unique())

    cm = confusion_matrix(
        y_test,
        y_pred,
        labels=labels
    )

    cm_df = pd.DataFrame(
        cm,
        index=labels,
        columns=labels
    )

    print(cm_df)

    cm_df.to_csv(
        RESULTS_DIR / "confusion_matrix.csv"
    )

    plt.figure(figsize=(9, 7))

    sns.heatmap(
        cm_df,
        annot=True,
        fmt="d",
        cmap="Blues",
        linewidths=0.5,
        cbar=True
    )

    plt.title("EVA-Guard - Confusion Matrix")
    plt.xlabel("Predicted Scenario")
    plt.ylabel("Actual Scenario")

    plt.tight_layout()

    plt.savefig(
        RESULTS_DIR / "confusion_matrix.png",
        dpi=300
    )

    plt.close()

    return cm_df


# =====================================================
# LOGISTIC REGRESSION COEFFICIENTS
# =====================================================

def analyze_coefficients(model, features):

    print("\n========== LOGISTIC REGRESSION COEFFICIENTS ==========")

    classifier = model.named_steps["classifier"]

    if not hasattr(classifier, "coef_"):
        print("Model does not expose linear coefficients.")
        return None

    coefficients = classifier.coef_
    classes = classifier.classes_

    rows = []

    for class_index, class_name in enumerate(classes):

        for feature_index, feature_name in enumerate(features):

            rows.append({
                "scenario": class_name,
                "feature": feature_name,
                "coefficient": coefficients[
                    class_index,
                    feature_index
                ],
                "absolute_coefficient": abs(
                    coefficients[
                        class_index,
                        feature_index
                    ]
                )
            })

    coef_df = pd.DataFrame(rows)

    coef_df = coef_df.sort_values(
        ["scenario", "absolute_coefficient"],
        ascending=[True, False]
    )

    coef_df.to_csv(
        RESULTS_DIR / "logistic_coefficients.csv",
        index=False
    )

    for class_name in classes:

        print(f"\nTop features for {class_name}:")

        print(
            coef_df[
                coef_df["scenario"] == class_name
            ][
                ["feature", "coefficient"]
            ].head(10).to_string(index=False)
        )

    # Visualisation des 15 coefficients les plus importants
    top_features = (
        coef_df.groupby("feature")["absolute_coefficient"]
        .mean()
        .nlargest(15)
        .index
    )

    plot_df = coef_df[
        coef_df["feature"].isin(top_features)
    ]

    pivot_df = plot_df.pivot(
        index="feature",
        columns="scenario",
        values="coefficient"
    )

    plt.figure(figsize=(11, 8))

    sns.heatmap(
        pivot_df,
        cmap="coolwarm",
        center=0,
        annot=False
    )

    plt.title("Logistic Regression - Feature Coefficients")
    plt.tight_layout()

    plt.savefig(
        RESULTS_DIR / "coefficient_heatmap.png",
        dpi=300
    )

    plt.close()

    return coef_df


# =====================================================
# PERMUTATION IMPORTANCE
# =====================================================

def analyze_permutation_importance(
    model,
    X_test,
    y_test
):

    print("\n========== PERMUTATION IMPORTANCE ==========")

    result = permutation_importance(
        model,
        X_test,
        y_test,
        scoring="f1_macro",
        n_repeats=10,
        random_state=RANDOM_STATE,
        n_jobs=-1
    )

    importance_df = pd.DataFrame({
        "feature": X_test.columns,
        "importance_mean": result.importances_mean,
        "importance_std": result.importances_std
    })

    importance_df = importance_df.sort_values(
        "importance_mean",
        ascending=False
    )

    importance_df.to_csv(
        RESULTS_DIR / "permutation_importance.csv",
        index=False
    )

    print("\nTop 20 important features:")

    print(
        importance_df.head(20).to_string(index=False)
    )

    # Plot
    top = importance_df.head(20).sort_values(
        "importance_mean"
    )

    plt.figure(figsize=(10, 8))

    plt.barh(
        top["feature"],
        top["importance_mean"],
        xerr=top["importance_std"]
    )

    plt.xlabel("Decrease in Macro F1-score")
    plt.title("EVA-Guard - Permutation Feature Importance")

    plt.tight_layout()

    plt.savefig(
        RESULTS_DIR / "permutation_importance.png",
        dpi=300
    )

    plt.close()

    return importance_df


# =====================================================
# ERROR ANALYSIS
# =====================================================

def analyze_errors(test_df, X_test, y_test, y_pred):

    print("\n========== ERROR ANALYSIS ==========")

    error_df = test_df.copy()

    error_df["predicted_scenario"] = y_pred

    error_df["correct_prediction"] = (
        error_df["scenario"]
        == error_df["predicted_scenario"]
    )

    errors = error_df[
        ~error_df["correct_prediction"]
    ].copy()

    print("Total observations:", len(error_df))
    print("Correct predictions:", len(error_df) - len(errors))
    print("Incorrect predictions:", len(errors))

    print("\nError distribution:")

    if not errors.empty:

        print(
            errors.groupby(
                ["scenario", "predicted_scenario"]
            ).size().rename("count")
        )

    else:
        print("No classification errors.")

    # Save all predictions
    error_df.to_csv(
        RESULTS_DIR / "predictions_test.csv",
        index=False
    )

    # Save incorrect predictions
    errors.to_csv(
        RESULTS_DIR / "misclassified_samples.csv",
        index=False
    )

    return errors


# =====================================================
# MAIN
# =====================================================

def main():

    df, model, metadata, features, target = (
        load_data_and_model()
    )

    test_df, X_test, y_test = prepare_test_data(
        df,
        features,
        target
    )

    y_pred = predict(model, X_test)

    evaluate_classification(
        y_test,
        y_pred
    )

    evaluate_confusion_matrix(
        y_test,
        y_pred
    )

    analyze_coefficients(
        model,
        features
    )

    analyze_permutation_importance(
        model,
        X_test,
        y_test
    )

    analyze_errors(
        test_df,
        X_test,
        y_test,
        y_pred
    )

    print("\n========== EVALUATION COMPLETED ==========")

    print("Results saved in:", RESULTS_DIR)


if __name__ == "__main__":
    main()
