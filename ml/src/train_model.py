
from pathlib import Path
import json
import warnings

import joblib
import numpy as np
import pandas as pd

from sklearn.impute import SimpleImputer
from sklearn.preprocessing import StandardScaler
from sklearn.pipeline import Pipeline

from sklearn.linear_model import LogisticRegression
from sklearn.ensemble import (
    RandomForestClassifier,
    ExtraTreesClassifier,
    HistGradientBoostingClassifier
)

from sklearn.metrics import (
    accuracy_score,
    precision_score,
    recall_score,
    f1_score,
    classification_report,
    confusion_matrix
)

warnings.filterwarnings("ignore")


# =====================================================
# PATHS
# =====================================================

ROOT = Path(__file__).resolve().parents[1]

DATA_FILE = ROOT / "data/processed/dataset_features.csv"
CONFIG_FILE = ROOT / "data/processed/feature_engineering_config.json"

MODELS_DIR = ROOT / "models"
RESULTS_DIR = ROOT / "results"

MODELS_DIR.mkdir(exist_ok=True)
RESULTS_DIR.mkdir(exist_ok=True)


# =====================================================
# CONFIGURATION
# =====================================================

TARGET = "scenario"

TRAIN_SESSIONS = ["session1", "session2"]
VALIDATION_SESSION = "session3"

FINAL_TRAIN_SESSIONS = [
    "session1",
    "session2",
    "session3"
]

TEST_SESSION = "session4"

RANDOM_STATE = 42


# =====================================================
# LOAD DATA
# =====================================================

def load_data():

    df = pd.read_csv(DATA_FILE)

    with open(CONFIG_FILE, "r") as f:
        config = json.load(f)

    feature_columns = config["feature_columns"]

    print("Dataset shape:", df.shape)
    print("Features disponibles:", len(feature_columns))

    return df, feature_columns


# =====================================================
# PREPARE DATA
# =====================================================

def prepare_xy(df, feature_columns):

    X = df[feature_columns].copy()
    y = df[TARGET].copy()

    # Vérifier les valeurs infinies.
    X = X.replace([np.inf, -np.inf], np.nan)

    # Supprimer les colonnes entièrement vides,
    # uniquement à partir du jeu d'entraînement.
    return X, y


# =====================================================
# MODEL DEFINITIONS
# =====================================================

def build_models():

    models = {

        "LogisticRegression": Pipeline([
            ("imputer", SimpleImputer(strategy="median")),
            ("scaler", StandardScaler()),
            ("classifier", LogisticRegression(
                max_iter=3000,
                random_state=RANDOM_STATE
            ))
        ]),

        "RandomForest": Pipeline([
            ("imputer", SimpleImputer(strategy="median")),
            ("classifier", RandomForestClassifier(
                n_estimators=300,
                max_depth=None,
                min_samples_split=2,
                min_samples_leaf=1,
                random_state=RANDOM_STATE,
                n_jobs=-1
            ))
        ]),

        "ExtraTrees": Pipeline([
            ("imputer", SimpleImputer(strategy="median")),
            ("classifier", ExtraTreesClassifier(
                n_estimators=300,
                max_depth=None,
                min_samples_split=2,
                min_samples_leaf=1,
                random_state=RANDOM_STATE,
                n_jobs=-1
            ))
        ]),

        "HistGradientBoosting": Pipeline([
            ("imputer", SimpleImputer(strategy="median")),
            ("classifier", HistGradientBoostingClassifier(
                max_iter=200,
                learning_rate=0.1,
                max_leaf_nodes=15,
                random_state=RANDOM_STATE
            ))
        ])
    }

    return models


# =====================================================
# METRICS
# =====================================================

def calculate_metrics(y_true, y_pred):

    return {
        "accuracy": accuracy_score(y_true, y_pred),

        "precision_macro": precision_score(
            y_true,
            y_pred,
            average="macro",
            zero_division=0
        ),

        "recall_macro": recall_score(
            y_true,
            y_pred,
            average="macro",
            zero_division=0
        ),

        "f1_macro": f1_score(
            y_true,
            y_pred,
            average="macro",
            zero_division=0
        ),

        "f1_weighted": f1_score(
            y_true,
            y_pred,
            average="weighted",
            zero_division=0
        )
    }


# =====================================================
# VALIDATION
# =====================================================

def validate_models(df, feature_columns):

    train_df = df[
        df["session"].isin(TRAIN_SESSIONS)
    ].copy()

    val_df = df[
        df["session"] == VALIDATION_SESSION
    ].copy()

    X_train, y_train = prepare_xy(
        train_df, feature_columns
    )

    X_val, y_val = prepare_xy(
        val_df, feature_columns
    )

    # Écarter les colonnes entièrement vides dans le train.
    valid_features = X_train.columns[
        X_train.notna().any()
    ].tolist()

    X_train = X_train[valid_features]
    X_val = X_val[valid_features]

    print("\n========== VALIDATION SPLIT ==========")

    print("Train:", X_train.shape)
    print("Validation:", X_val.shape)

    print("\nTrain sessions:")
    print(train_df["session"].value_counts())

    print("\nValidation session:")
    print(val_df["session"].value_counts())

    print("\nTrain classes:")
    print(y_train.value_counts())

    print("\nValidation classes:")
    print(y_val.value_counts())

    models = build_models()

    results = []
    fitted_models = {}

    for name, model in models.items():

        print(f"\nTraining: {name}")

        model.fit(X_train, y_train)

        predictions = model.predict(X_val)

        metrics = calculate_metrics(
            y_val,
            predictions
        )

        metrics["model"] = name

        results.append(metrics)
        fitted_models[name] = model

        print("Accuracy:", round(metrics["accuracy"], 4))
        print("Macro F1:", round(metrics["f1_macro"], 4))

    results_df = pd.DataFrame(results)

    results_df = results_df.sort_values(
        "f1_macro",
        ascending=False
    )

    results_df.to_csv(
        RESULTS_DIR / "validation_metrics.csv",
        index=False
    )

    print("\n========== VALIDATION RESULTS ==========")
    print(results_df.to_string(index=False))

    best_model_name = results_df.iloc[0]["model"]

    print("\nBest model:", best_model_name)

    return (
        best_model_name,
        valid_features,
        results_df
    )


# =====================================================
# FINAL TRAINING AND TEST
# =====================================================

def final_training(df, feature_columns, best_model_name):

    train_df = df[
        df["session"].isin(FINAL_TRAIN_SESSIONS)
    ].copy()

    test_df = df[
        df["session"] == TEST_SESSION
    ].copy()

    X_train, y_train = prepare_xy(
        train_df, feature_columns
    )

    X_test, y_test = prepare_xy(
        test_df, feature_columns
    )

    valid_features = X_train.columns[
        X_train.notna().any()
    ].tolist()

    X_train = X_train[valid_features]
    X_test = X_test[valid_features]

    print("\n========== FINAL TRAINING ==========")

    print("Training shape:", X_train.shape)
    print("Test shape:", X_test.shape)

    print("\nFinal training sessions:")
    print(train_df["session"].value_counts())

    print("\nTest session:")
    print(test_df["session"].value_counts())

    models = build_models()

    best_model = models[best_model_name]

    print("\nTraining final model:", best_model_name)

    best_model.fit(X_train, y_train)

    predictions = best_model.predict(X_test)

    metrics = calculate_metrics(
        y_test,
        predictions
    )

    print("\n========== FINAL TEST METRICS ==========")

    for name, value in metrics.items():
        print(f"{name}: {value:.4f}")

    # Classification report
    report = classification_report(
        y_test,
        predictions,
        output_dict=True,
        zero_division=0
    )

    with open(
        RESULTS_DIR / "classification_report.json",
        "w"
    ) as f:
        json.dump(report, f, indent=4)

    # Confusion matrix
    labels = sorted(y_test.unique())

    cm = confusion_matrix(
        y_test,
        predictions,
        labels=labels
    )

    cm_df = pd.DataFrame(
        cm,
        index=labels,
        columns=labels
    )

    cm_df.to_csv(
        RESULTS_DIR / "confusion_matrix.csv"
    )

    print("\nConfusion Matrix:")
    print(cm_df)

    # Save test metrics
    pd.DataFrame([metrics]).assign(
        model=best_model_name
    ).to_csv(
        RESULTS_DIR / "test_metrics.csv",
        index=False
    )

    # Save complete pipeline
    joblib.dump(
        best_model,
        MODELS_DIR / "best_model.joblib"
    )

    # Save model metadata
    metadata = {
        "model": best_model_name,
        "target": TARGET,
        "classes": labels,
        "features": valid_features,
        "training_sessions": FINAL_TRAIN_SESSIONS,
        "test_session": TEST_SESSION,
        "metrics": metrics
    }

    with open(
        MODELS_DIR / "model_metadata.json",
        "w"
    ) as f:
        json.dump(metadata, f, indent=4)

    # Feature importance for tree models
    classifier = best_model.named_steps["classifier"]

    if hasattr(classifier, "feature_importances_"):

        importance_df = pd.DataFrame({
            "feature": valid_features,
            "importance": classifier.feature_importances_
        }).sort_values(
            "importance",
            ascending=False
        )

        importance_df.to_csv(
            RESULTS_DIR / "feature_importance.csv",
            index=False
        )

        print("\nTop 15 features:")
        print(importance_df.head(15).to_string(index=False))

    print("\nSaved model:")
    print(MODELS_DIR / "best_model.joblib")

    print("\nResults directory:")
    print(RESULTS_DIR)


# =====================================================
# MAIN
# =====================================================

def main():

    df, feature_columns = load_data()

    # Vérification des sessions attendues
    expected_sessions = set(
        TRAIN_SESSIONS
        + [VALIDATION_SESSION, TEST_SESSION]
    )

    available_sessions = set(df["session"].unique())

    if not expected_sessions.issubset(available_sessions):
        raise ValueError(
            f"Sessions manquantes: "
            f"{expected_sessions - available_sessions}"
        )

    # Validation et sélection du modèle
    (
        best_model_name,
        valid_features,
        validation_results
    ) = validate_models(
        df,
        feature_columns
    )

    # Réentraînement et évaluation finale
    final_training(
        df,
        valid_features,
        best_model_name
    )


if __name__ == "__main__":
    main()
