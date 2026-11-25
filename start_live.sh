#!/usr/bin/env bash
# EVA-Guard : détection d'attaques en direct (collecteur Rust + modèle ML) en une commande.
#
#   ./start_live.sh                         # modèle random_forest, affiche toutes les prédictions
#   ONLY_ALERTS=1 ./start_live.sh           # n'affiche que les alertes
#   MODEL=xgboost ./start_live.sh           # autre modèle de ml/models/
#   SKIP_CHECK=1 ./start_live.sh            # ne vérifie pas les nœuds avant de démarrer
#
# Ctrl+C arrête tout proprement (le ML affiche un résumé, le collecteur vide son fichier).
#
# Fichiers produits dans monitoring-rust/live/ :
#   normal.jsonl       métriques collectées (lues par le ML)
#   collector.log      journal du collecteur
#   check.log          résultat de la vérification des nœuds
#   predictions.jsonl  toutes les prédictions affichées
#   archive/           fichiers d'une session précédente (déplacés, jamais supprimés)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$ROOT/monitoring-rust"
ML_DIR="$ROOT/ml"
LIVE_DIR="$RUST_DIR/live"
PYTHON="$ROOT/.venv/bin/python"
BIN="$RUST_DIR/target/release/monitoring-rust"
MODEL="${MODEL:-random_forest}"
INPUT="$LIVE_DIR/normal.jsonl"   # EVA_SCENARIO=NORMAL (défaut) -> normal.jsonl
COLLECTOR_PID=""

say() { printf '\033[1;36m[start_live]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[start_live] ERREUR :\033[0m %s\n' "$*" >&2; exit 1; }

cleanup() {
  trap - INT TERM EXIT
  if [[ -n "$COLLECTOR_PID" ]] && kill -0 "$COLLECTOR_PID" 2>/dev/null; then
    say "Arrêt du collecteur (PID $COLLECTOR_PID)..."
    kill -TERM "$COLLECTOR_PID" 2>/dev/null || true
    wait "$COLLECTOR_PID" 2>/dev/null || true
    tail -n 7 "$LIVE_DIR/collector.log" 2>/dev/null || true
  fi
  say "Terminé. Prédictions : $LIVE_DIR/predictions.jsonl"
}

# --------------------------------------------------------------------------- 1. prérequis
[[ -x "$PYTHON" ]] || die "environnement Python introuvable : $PYTHON"
[[ -f "$ML_DIR/models/$MODEL.joblib" ]] || die "modèle introuvable : ml/models/$MODEL.joblib (lance d'abord l'entraînement : ml/run_all.sh)"
command -v cargo >/dev/null || die "cargo introuvable (Rust n'est pas installé ?)"

say "Compilation du collecteur Rust (rapide si déjà compilé)..."
(cd "$RUST_DIR" && cargo build --release --quiet) || die "la compilation de monitoring-rust a échoué"

# --------------------------------------------------------------------------- 2. réseau Ethereum
mkdir -p "$LIVE_DIR"
if [[ "${SKIP_CHECK:-0}" != "1" ]]; then
  say "Vérification des 5 nœuds Ethereum..."
  if ! (cd "$RUST_DIR" && "$BIN" --check) > "$LIVE_DIR/check.log" 2>&1; then
    grep -E "FAIL|checks failed" "$LIVE_DIR/check.log" | head -n 12 || true
    die "des nœuds ne répondent pas. Le réseau est-il lancé ? (kurtosis enclave ls)
       Si les ports ont changé, mets à jour monitoring-rust/config/nodes.toml
       (ports visibles avec : kurtosis enclave inspect eva-ethereum-v4)"
  fi
  say "Tous les nœuds répondent."
fi

# --------------------------------------------------------------------------- 3. collecteur
if [[ -s "$INPUT" ]]; then
  mkdir -p "$LIVE_DIR/archive"
  stamp="$(date +%Y%m%d_%H%M%S)"
  mv "$INPUT" "$LIVE_DIR/archive/normal_$stamp.jsonl"
  [[ -f "$LIVE_DIR/predictions.jsonl" ]] && mv "$LIVE_DIR/predictions.jsonl" "$LIVE_DIR/archive/predictions_$stamp.jsonl"
  say "Ancienne session archivée dans live/archive/ ($stamp)."
fi

trap cleanup INT TERM EXIT
say "Démarrage du collecteur (journal : live/collector.log)..."
(cd "$RUST_DIR" && EVA_DATA_DIR="$LIVE_DIR" exec "$BIN") > "$LIVE_DIR/collector.log" 2>&1 &
COLLECTOR_PID=$!

say "Attente des premières métriques (environ 5 à 10 s)..."
for _ in $(seq 1 60); do
  [[ -s "$INPUT" ]] && break
  kill -0 "$COLLECTOR_PID" 2>/dev/null || { tail -n 20 "$LIVE_DIR/collector.log"; die "le collecteur s'est arrêté"; }
  sleep 1
done
[[ -s "$INPUT" ]] || die "aucune métrique écrite après 60 s (voir live/collector.log)"

# --------------------------------------------------------------------------- 4. détection ML
ARGS=(--model "$MODEL" --input "$INPUT" --follow --output "$LIVE_DIR/predictions.jsonl")
[[ "${ONLY_ALERTS:-0}" == "1" ]] && ARGS+=(--only-alerts)

say "Détection en direct avec le modèle « $MODEL ». Ctrl+C pour arrêter."
say "Première prédiction par nœud après ~2 cycles (10 s). \"alert\": true = attaque détectée."
echo
cd "$ML_DIR"
"$PYTHON" -m src.inference.stream "${ARGS[@]}" || true
