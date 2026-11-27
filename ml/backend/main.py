
import asyncio
import json
import logging
import time

from collections import OrderedDict, deque
from contextlib import asynccontextmanager
from datetime import datetime, timedelta, timezone

from fastapi import FastAPI, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware

from .config import (
    LIVE_OBSERVATIONS_FILE,
    PREDICTIONS_FILE,
)

from .jsonl_reader import JSONLReader
from .observation_aggregator import ObservationAggregator
from .rust_adapter import RustAdapter
from .feature_builder import FeatureBuilder
from .predictor import EVAGuardPredictor

from agentic.schemas import Incident, AttackType
from agentic.orchestrator import AgentOrchestrator


# ============================================================
# LOGGING
# ============================================================

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s"
)

logger = logging.getLogger("EVA-GUARD")


# ============================================================
# GLOBAL STATE
# ============================================================

reader = None
aggregator = None
adapter = None
feature_builder = None
predictor = None

orchestrator = None

investigation_queue = None
investigation_worker_task = None

prediction_history = deque(maxlen=5000)
latest_predictions = {}

active_attack_by_node = {}

# Consecutive ML detections required before starting an AI investigation.
attack_detection_streak = {}

# Last successful enqueue time, keyed by (node_id, attack_type).
last_agent_invocation = {}

# Read-only views exposed to the frontend (populated after ML inference,
# never consulted by the detection or incident logic).
latest_observations = {}
observation_history = deque(maxlen=2000)
incident_registry = OrderedDict()

background_task = None
is_running = False

orchestrator_unavailable_logged = False


# ============================================================
# CONFIGURATION
# ============================================================

POLL_INTERVAL = 0.25

ALERT_CONFIDENCE_THRESHOLD = 0.60

# AI agents are invoked only after consecutive detections and cooldown.
REQUIRED_CONSECUTIVE_DETECTIONS = 2
AGENT_COOLDOWN_SECONDS = 180  # 3 minutes

INVESTIGATION_QUEUE_SIZE = 100

INVESTIGATION_REPORTS_FILE = (
    PREDICTIONS_FILE.parent / "investigation_reports.jsonl"
)

MAX_TRACKED_INCIDENTS = 1000

# A node is "active" if it sent an observation within this window.
ACTIVE_NODE_WINDOW_SECONDS = 30


# ============================================================
# API READ MODELS (frontend views, no detection logic)
# ============================================================

def utc_now():

    return datetime.now(timezone.utc)


def parse_timestamp(value):

    if not value:
        return None

    try:
        parsed = datetime.fromisoformat(
            str(value).replace("Z", "+00:00")
        )

    except ValueError:
        return None

    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)

    return parsed


def record_observation(observation_snapshot):
    """
    Keeps the latest aggregated observation per node for the API.
    Called after inference; does not influence predictions.
    """

    components = observation_snapshot.get("components") or {}

    view = {
        "timestamp": observation_snapshot.get("timestamp"),
        "node_id": observation_snapshot.get("node_id"),
        "received_at": utc_now().isoformat(),
        "geth": dict(components.get("geth") or {}),
        "beacon": dict(components.get("beacon") or {}),
        "validator": dict(components.get("validator") or {}),
        "system": dict(observation_snapshot.get("system") or {}),
    }

    latest_observations[view["node_id"]] = view

    observation_history.append(view)


def build_node_view(node_id):

    prediction = latest_predictions.get(node_id)
    observation = latest_observations.get(node_id)

    return {
        "node_id": node_id,
        "scenario": prediction["prediction"] if prediction else None,
        "confidence": prediction["confidence"] if prediction else None,
        "last_update": (
            prediction["timestamp"] if prediction
            else observation["timestamp"]
        ),
        "latest_prediction": prediction,
        "observation_timestamp": (
            observation["timestamp"] if observation else None
        ),
        "geth": observation["geth"] if observation else None,
        "beacon": observation["beacon"] if observation else None,
        "validator": observation["validator"] if observation else None,
        "system": observation["system"] if observation else None,
    }


def known_node_ids():

    return sorted(
        set(latest_predictions) | set(latest_observations)
    )


def incident_status_for_verdict(verdict):
    """
    Maps the final verdict to the IncidentStatus enum.
    SUSPECTED / INCONCLUSIVE never become CONFIRMED.
    """

    if verdict == "CONFIRMED":
        return "CONFIRMED"

    if verdict == "REJECTED":
        return "REJECTED"

    return "CLOSED"


def register_incident(incident):
    """
    Tracks a queued incident for the API (copy, the Incident object
    handed to LangGraph is not modified).
    """

    record = incident.model_dump(mode="json")

    record.update({
        "final_verdict": None,
        "report_available": False,
        "investigation_started_at": None,
        "investigation_completed_at": None,
    })

    incident_registry[incident.incident_id] = record

    while len(incident_registry) > MAX_TRACKED_INCIDENTS:
        incident_registry.popitem(last=False)


def update_incident(incident_id, **changes):

    record = incident_registry.get(incident_id)

    if record is not None:
        record.update(changes)


def load_investigation_reports():

    if not INVESTIGATION_REPORTS_FILE.exists():
        return []

    reports = []

    with INVESTIGATION_REPORTS_FILE.open(
        "r",
        encoding="utf-8"
    ) as file:

        for line in file:

            if not line.strip():
                continue

            try:

                reports.append(
                    json.loads(line)
                )

            except json.JSONDecodeError:

                logger.warning(
                    "Invalid investigation report JSON line."
                )

    return reports


def incident_from_report(report):
    """
    Incident view rebuilt from a persisted report (e.g. after a restart).
    Fields the report does not contain are returned as null.
    """

    verdict = report.get("final_verdict")

    return {
        "incident_id": report.get("incident_id"),
        "node_id": report.get("node_id"),
        "attack_type": report.get("attack_type"),
        "ml_prediction": report.get("ml_prediction"),
        "ml_confidence": report.get("ml_confidence"),
        "probabilities": None,
        "prediction_timestamp": None,
        "created_at": None,
        "status": incident_status_for_verdict(verdict),
        "investigation_status": report.get("investigation_status"),
        "final_verdict": verdict,
        "report_available": True,
        "investigation_started_at": None,
        "investigation_completed_at": report.get("generated_at"),
        "observation_snapshot": None,
    }


def collect_incidents():
    """
    Live incidents (memory) merged with incidents known only from
    persisted investigation reports. Newest first.
    """

    incidents = {
        incident_id: dict(record)
        for incident_id, record in incident_registry.items()
    }

    for report in load_investigation_reports():

        incident_id = report.get("incident_id")

        if not incident_id:
            continue

        if incident_id in incidents:
            incidents[incident_id]["report_available"] = True
            continue

        incidents[incident_id] = incident_from_report(report)

    def sort_key(item):
        return (
            item.get("created_at")
            or item.get("investigation_completed_at")
            or ""
        )

    return sorted(
        incidents.values(),
        key=sort_key,
        reverse=True
    )


# ============================================================
# PREDICTION STORAGE
# ============================================================

def save_prediction(prediction):

    PREDICTIONS_FILE.parent.mkdir(
        parents=True,
        exist_ok=True
    )

    with PREDICTIONS_FILE.open(
        "a",
        encoding="utf-8"
    ) as file:

        file.write(
            json.dumps(prediction) + "\n"
        )

    prediction_history.append(prediction)

    node_id = prediction["node_id"]

    latest_predictions[node_id] = prediction


def get_alerts():

    alerts = []

    for prediction in latest_predictions.values():

        if prediction["prediction"] == "NORMAL":
            continue

        if prediction["confidence"] < ALERT_CONFIDENCE_THRESHOLD:
            continue

        alerts.append(prediction)

    return alerts


# ============================================================
# INCIDENT MANAGEMENT
# ============================================================

def create_incident(
    prediction,
    observation_snapshot
):

    attack_name = prediction["prediction"]

    try:
        attack_type = AttackType(attack_name)

    except ValueError:

        logger.warning(
            "Unsupported attack type: %s",
            attack_name
        )

        return None

    timestamp_value = prediction.get("timestamp")

    if not timestamp_value:
        raise ValueError("Prediction timestamp is missing.")

    timestamp = datetime.fromisoformat(
        timestamp_value.replace("Z", "+00:00")
    )

    if timestamp.tzinfo is None:
        timestamp = timestamp.replace(
            tzinfo=timezone.utc
        )

    incident = Incident(
        node_id=prediction["node_id"],
        attack_type=attack_type,
        ml_prediction=attack_name,
        ml_confidence=prediction["confidence"],
        probabilities=prediction["probabilities"],
        prediction_timestamp=timestamp,
        observation_snapshot=dict(observation_snapshot)
    )

    return incident


def enqueue_incident(prediction, observation_snapshot):
    """
    Queue an AI investigation only when the same attack has been
    detected consecutively enough times and its per-node/per-attack
    cooldown has expired. ML inference itself is never delayed.
    """

    global orchestrator_unavailable_logged

    node_id = prediction["node_id"]
    attack_name = prediction["prediction"]
    confidence = float(prediction["confidence"])

    # 1. A normal prediction or low confidence breaks the consecutive streak.
    if (
        attack_name == "NORMAL"
        or confidence < ALERT_CONFIDENCE_THRESHOLD
    ):
        attack_detection_streak.pop(node_id, None)
        active_attack_by_node.pop(node_id, None)
        return

    # 2. Count consecutive detections of the same attack for this node.
    previous = attack_detection_streak.get(node_id)

    if previous is not None and previous["attack_type"] == attack_name:
        previous["count"] += 1
    else:
        attack_detection_streak[node_id] = {
            "attack_type": attack_name,
            "count": 1,
        }

    detection_count = attack_detection_streak[node_id]["count"]

    logger.info(
        "[ATTACK DETECTION] Node=%s | Attack=%s | Consecutive=%d/%d",
        node_id,
        attack_name,
        detection_count,
        REQUIRED_CONSECUTIVE_DETECTIONS,
    )

    if detection_count < REQUIRED_CONSECUTIVE_DETECTIONS:
        return

    # 3. Do not enqueue if the agent system is unavailable.
    if orchestrator is None or investigation_queue is None:
        if not orchestrator_unavailable_logged:
            logger.warning(
                "LangGraph unavailable. ML predictions continue "
                "without AI investigations."
            )
            orchestrator_unavailable_logged = True
        return

    # 4. Apply an independent 3-minute cooldown for each node + attack type.
    investigation_key = (node_id, attack_name)
    now = time.monotonic()
    last_invocation = last_agent_invocation.get(investigation_key)

    if last_invocation is not None:
        elapsed = now - last_invocation
        if elapsed < AGENT_COOLDOWN_SECONDS:
            remaining = AGENT_COOLDOWN_SECONDS - elapsed
            logger.info(
                "[AGENT COOLDOWN] Node=%s | Attack=%s | "
                "Next investigation in %.1f seconds",
                node_id,
                attack_name,
                remaining,
            )
            return

    # 5. Create and enqueue the incident.
    incident = create_incident(prediction, observation_snapshot)
    if incident is None:
        return

    try:
        investigation_queue.put_nowait(incident)
    except asyncio.QueueFull:
        logger.error(
            "Investigation queue full. Incident not queued for node %s",
            node_id,
        )
        return

    # Start cooldown only after successful insertion into the queue.
    last_agent_invocation[investigation_key] = now
    active_attack_by_node[node_id] = attack_name
    register_incident(incident)

    logger.warning(
        "[AGENT INVESTIGATION QUEUED] Incident=%s | Node=%s | "
        "Attack=%s | Confidence=%.4f | Cooldown=%ds",
        incident.incident_id,
        node_id,
        attack_name,
        confidence,
        AGENT_COOLDOWN_SECONDS,
    )


# ============================================================
# INVESTIGATION REPORT STORAGE
# ============================================================

def save_investigation_report(report):

    INVESTIGATION_REPORTS_FILE.parent.mkdir(
        parents=True,
        exist_ok=True
    )

    with INVESTIGATION_REPORTS_FILE.open(
        "a",
        encoding="utf-8"
    ) as file:

        file.write(
            json.dumps(
                report,
                ensure_ascii=False,
                default=str
            ) + "\n"
        )


# ============================================================
# LANGGRAPH INVESTIGATION WORKER
# ============================================================

async def investigation_worker():

    logger.info(
        "LangGraph investigation worker started."
    )

    while True:

        incident = await investigation_queue.get()

        try:

            logger.info(
                "Starting investigation | Incident=%s | Node=%s",
                incident.incident_id,
                incident.node_id
            )

            update_incident(
                incident.incident_id,
                status="INVESTIGATING",
                investigation_status="RUNNING",
                investigation_started_at=utc_now().isoformat()
            )

            report = await orchestrator.run_investigation(
                incident
            )

            save_investigation_report(report)

            update_incident(
                incident.incident_id,
                status=incident_status_for_verdict(
                    report.get("final_verdict")
                ),
                investigation_status=report.get("investigation_status"),
                final_verdict=report.get("final_verdict"),
                report_available=True,
                investigation_completed_at=report.get("generated_at")
            )

            logger.info(
                "Investigation completed | Incident=%s | Verdict=%s",
                incident.incident_id,
                report.get("final_verdict")
            )

            if report.get("investigation_status") == "FAILED":

                active_attack_by_node.pop(
                    incident.node_id,
                    None
                )

        except Exception:

            logger.exception(
                "Investigation failed for incident %s",
                incident.incident_id
            )

            update_incident(
                incident.incident_id,
                status="CLOSED",
                investigation_status="FAILED",
                investigation_completed_at=utc_now().isoformat()
            )

            active_attack_by_node.pop(
                incident.node_id,
                None
            )

        finally:

            investigation_queue.task_done()


# ============================================================
# OBSERVATION PROCESSING
# ============================================================

def process_observation(observation):

    # 1. Aggregate Geth, Beacon and Validator.

    aggregated = aggregator.add_observation(
        observation
    )

    if aggregated is None:
        return None

    # 2. Convert Rust data to ML raw features.

    adapted = adapter.adapt(
        aggregated
    )

    # 3. Build the 69 temporal features.

    features = feature_builder.build(
        adapted
    )

    # 4. Execute ML inference.

    prediction = predictor.predict(
        features=features,
        node_id=adapted["node_id"],
        timestamp=adapted["timestamp"]
    )

    # Return both prediction and original metrics.

    return prediction, aggregated


# ============================================================
# BACKGROUND INFERENCE LOOP
# ============================================================

async def inference_loop():

    global is_running

    logger.info("Live inference started.")

    is_running = True

    while is_running:

        try:

            observations = reader.read_new_lines()

            for observation in observations:

                try:

                    result = process_observation(
                        observation
                    )

                    if result is None:
                        continue

                    prediction, observation_snapshot = result

                    # Save ML prediction.

                    save_prediction(
                        prediction
                    )

                    logger.info(
                        "Node=%s | Prediction=%s | Confidence=%.4f",
                        prediction["node_id"],
                        prediction["prediction"],
                        prediction["confidence"]
                    )

                    # Dispatch attack investigation asynchronously.

                    enqueue_incident(
                        prediction,
                        observation_snapshot
                    )

                    # Expose the metrics to the API (read-only view).

                    record_observation(
                        observation_snapshot
                    )

                except Exception:

                    logger.exception(
                        "Error processing observation"
                    )

        except Exception:

            logger.exception(
                "Inference loop error"
            )

        await asyncio.sleep(
            POLL_INTERVAL
        )


# ============================================================
# APPLICATION LIFESPAN
# ============================================================

@asynccontextmanager
async def lifespan(app: FastAPI):

    global reader
    global aggregator
    global adapter
    global feature_builder
    global predictor

    global orchestrator
    global investigation_queue
    global investigation_worker_task

    global background_task
    global is_running

    logger.info(
        "Initializing EVA-Guard backend."
    )

    # --------------------------------------------------------
    # Initialize ML components.
    # --------------------------------------------------------

    reader = JSONLReader(
        LIVE_OBSERVATIONS_FILE,
        start_at_end=True
    )

    aggregator = ObservationAggregator()

    adapter = RustAdapter()

    feature_builder = FeatureBuilder()

    predictor = EVAGuardPredictor()

    logger.info(
        "ML backend initialized."
    )

    # --------------------------------------------------------
    # Initialize asynchronous queue.
    # --------------------------------------------------------

    investigation_queue = asyncio.Queue(
        maxsize=INVESTIGATION_QUEUE_SIZE
    )

    # --------------------------------------------------------
    # Initialize LangGraph independently.
    # ML must remain operational if agents are unavailable.
    # --------------------------------------------------------

    try:

        orchestrator = AgentOrchestrator(
            max_retries=1
        )

        investigation_worker_task = asyncio.create_task(
            investigation_worker()
        )

        logger.info(
            "LangGraph orchestrator initialized."
        )

    except Exception:

        orchestrator = None

        investigation_worker_task = None

        logger.exception(
            "LangGraph initialization failed. "
            "ML inference will continue independently."
        )

    # --------------------------------------------------------
    # Start ML inference.
    # --------------------------------------------------------

    background_task = asyncio.create_task(
        inference_loop()
    )

    logger.info(
        "EVA-Guard backend started successfully."
    )

    yield

    # ========================================================
    # SHUTDOWN
    # ========================================================

    logger.info(
        "Stopping EVA-Guard backend."
    )

    # Stop inference loop.

    is_running = False

    if background_task:

        background_task.cancel()

        try:
            await background_task

        except asyncio.CancelledError:
            pass

    # Wait for pending investigations.

    if investigation_worker_task:

        try:

            await asyncio.wait_for(
                investigation_queue.join(),
                timeout=30
            )

        except asyncio.TimeoutError:

            logger.warning(
                "Investigation queue shutdown timeout."
            )

        # Stop worker.

        investigation_worker_task.cancel()

        try:
            await investigation_worker_task

        except asyncio.CancelledError:
            pass

    logger.info(
        "EVA-Guard backend stopped."
    )


# ============================================================
# FASTAPI APPLICATION
# ============================================================

app = FastAPI(
    title="EVA-Guard API",
    description="Ethereum attack detection and monitoring API",
    version="1.0.0",
    lifespan=lifespan
)


# ============================================================
# CORS - ANGULAR FRONTEND
# ============================================================

app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:4200",
        "http://127.0.0.1:4200"
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"]
)


# ============================================================
# GENERAL ENDPOINTS
# ============================================================

@app.get("/")
def root():

    return {
        "project": "EVA-Guard",
        "status": "running",
        "service": "ML Inference API"
    }


@app.get("/api/v1/health")
def health():

    return {
        "status": "healthy",
        "model_loaded": predictor is not None,
        "inference_running": is_running,
        "orchestrator_loaded": orchestrator is not None,
        "investigation_worker_running": (
            investigation_worker_task is not None
            and not investigation_worker_task.done()
        ),
        "timestamp": datetime.now(
            timezone.utc
        ).isoformat()
    }


@app.get("/api/v1/status")
def system_status():

    return {
        "status": "running" if is_running else "stopped",
        "model_loaded": predictor is not None,
        "orchestrator_loaded": orchestrator is not None,
        "investigation_queue_size": (
            investigation_queue.qsize()
            if investigation_queue is not None
            else 0
        ),
        "nodes_detected": len(latest_predictions),
        "total_predictions": len(prediction_history),
        "alerts": len(get_alerts()),
        "rust_file": str(LIVE_OBSERVATIONS_FILE),
        "timestamp": datetime.now(
            timezone.utc
        ).isoformat()
    }


# ============================================================
# PREDICTIONS ENDPOINTS
# ============================================================

@app.get("/api/v1/predictions/latest")
def get_latest_predictions():

    return {
        "count": len(latest_predictions),
        "predictions": list(latest_predictions.values())
    }


# Declared before /predictions/{node_id} so "history" is not read as a node id.
@app.get("/api/v1/predictions/history")
def get_predictions_history(
    limit: int = Query(default=100, ge=1, le=1000),
    node_id: str | None = None,
    scenario: str | None = None
):

    return get_predictions(
        limit=limit,
        node_id=node_id,
        scenario=scenario
    )


@app.get("/api/v1/predictions")
def get_predictions(
    limit: int = Query(default=100, ge=1, le=1000),
    node_id: str | None = None,
    scenario: str | None = None
):

    results = list(prediction_history)

    if node_id:

        results = [
            item for item in results
            if item["node_id"] == node_id
        ]

    if scenario:

        results = [
            item for item in results
            if item["prediction"] == scenario
        ]

    results = results[-limit:]

    return {
        "count": len(results),
        "predictions": results
    }


@app.get("/api/v1/predictions/{node_id}")
def get_node_prediction(node_id: str):

    prediction = latest_predictions.get(node_id)

    if prediction is None:

        raise HTTPException(
            status_code=404,
            detail="No prediction found for this node"
        )

    return prediction


# ============================================================
# NODES ENDPOINTS
# ============================================================

@app.get("/api/v1/nodes")
def get_nodes():

    nodes = [
        build_node_view(node_id)
        for node_id in known_node_ids()
    ]

    return {
        "count": len(nodes),
        "nodes": nodes
    }


@app.get("/api/v1/nodes/{node_id}")
def get_node_details(node_id: str):

    if (
        node_id not in latest_predictions
        and node_id not in latest_observations
    ):

        raise HTTPException(
            status_code=404,
            detail="Node not found"
        )

    history = [
        item for item in prediction_history
        if item["node_id"] == node_id
    ]

    return {
        **build_node_view(node_id),
        "history": list(history)
    }


# ============================================================
# MONITORING ENDPOINTS
# ============================================================

@app.get("/api/v1/monitoring/latest")
def get_monitoring_latest():

    observations = [
        latest_observations[node_id]
        for node_id in sorted(latest_observations)
    ]

    return {
        "count": len(observations),
        "observations": observations
    }


@app.get("/api/v1/monitoring/history")
def get_monitoring_history(
    node_id: str | None = None,
    limit: int = Query(default=500, ge=1, le=2000),
    start_time: datetime | None = None,
    end_time: datetime | None = None
):

    if start_time is not None and start_time.tzinfo is None:
        start_time = start_time.replace(tzinfo=timezone.utc)

    if end_time is not None and end_time.tzinfo is None:
        end_time = end_time.replace(tzinfo=timezone.utc)

    results = []

    for item in observation_history:

        if node_id and item["node_id"] != node_id:
            continue

        if start_time or end_time:

            timestamp = parse_timestamp(item["timestamp"])

            if timestamp is None:
                continue

            if start_time and timestamp < start_time:
                continue

            if end_time and timestamp > end_time:
                continue

        results.append(item)

    results = results[-limit:]

    return {
        "count": len(results),
        "observations": results
    }


# ============================================================
# ALERTS ENDPOINTS
# ============================================================

@app.get("/api/v1/alerts")
def get_current_alerts():

    alerts = get_alerts()

    return {
        "count": len(alerts),
        "alerts": alerts
    }


# ============================================================
# INVESTIGATION ENDPOINTS
# ============================================================

@app.get("/api/v1/investigations")
def get_investigations(
    limit: int = Query(default=100, ge=1, le=1000)
):

    reports = load_investigation_reports()[-limit:]

    return {
        "count": len(reports),
        "investigations": reports
    }


@app.get("/api/v1/investigations/{incident_id}")
def get_investigation(incident_id: str):

    for report in reversed(load_investigation_reports()):

        if report.get("incident_id") == incident_id:
            return report

    record = incident_registry.get(incident_id)

    if record is None:

        raise HTTPException(
            status_code=404,
            detail="Investigation not found"
        )

    # Incident known, investigation queued or still running.

    return {
        "incident_id": incident_id,
        "node_id": record.get("node_id"),
        "attack_type": record.get("attack_type"),
        "investigation_status": record.get("investigation_status"),
        "final_verdict": None,
        "ml_prediction": record.get("ml_prediction"),
        "ml_confidence": record.get("ml_confidence"),
        "hypothesis": None,
        "confirmation": None,
        "error": None,
        "history": [],
        "generated_at": None
    }


# ============================================================
# INCIDENT ENDPOINTS
# ============================================================

@app.get("/api/v1/incidents")
def get_incidents(
    limit: int = Query(default=100, ge=1, le=1000),
    node_id: str | None = None
):

    incidents = collect_incidents()

    if node_id:

        incidents = [
            item for item in incidents
            if item.get("node_id") == node_id
        ]

    incidents = [
        {
            key: value
            for key, value in item.items()
            if key != "observation_snapshot"
        }
        for item in incidents[:limit]
    ]

    return {
        "count": len(incidents),
        "incidents": incidents
    }


@app.get("/api/v1/incidents/{incident_id}")
def get_incident(incident_id: str):

    for item in collect_incidents():

        if item.get("incident_id") == incident_id:
            return item

    raise HTTPException(
        status_code=404,
        detail="Incident not found"
    )


# ============================================================
# DASHBOARD STATISTICS
# ============================================================

@app.get("/api/v1/dashboard/summary")
def dashboard_summary():

    predictions = list(
        latest_predictions.values()
    )

    distribution = {
        "NORMAL": 0,
        "ECLIPSE": 0,
        "RPC_FLOODING": 0
    }

    for prediction in predictions:

        scenario = prediction["prediction"]

        if scenario in distribution:
            distribution[scenario] += 1

    total = len(predictions)

    attack_nodes = (
        distribution["ECLIPSE"]
        + distribution["RPC_FLOODING"]
    )

    return {
        "total_nodes": total,
        "normal_nodes": distribution["NORMAL"],
        "attack_nodes": attack_nodes,
        "eclipse_nodes": distribution["ECLIPSE"],
        "rpc_flooding_nodes": distribution["RPC_FLOODING"],
        "alert_count": len(get_alerts()),
        "distribution": distribution,
        "total_predictions": len(prediction_history),
        "last_prediction_at": (
            prediction_history[-1]["timestamp"]
            if prediction_history else None
        )
    }


@app.get("/api/v1/dashboard/overview")
def dashboard_overview():

    now = utc_now()

    active_nodes = 0

    for observation in latest_observations.values():

        received_at = parse_timestamp(observation.get("received_at"))

        if received_at and now - received_at <= timedelta(
            seconds=ACTIVE_NODE_WINDOW_SECONDS
        ):
            active_nodes += 1

    incidents = collect_incidents()

    attack_predictions = sum(
        1 for item in prediction_history
        if item["prediction"] != "NORMAL"
    )

    active_attacks = sum(
        1 for item in latest_predictions.values()
        if item["prediction"] != "NORMAL"
    )

    return {
        "total_nodes": len(known_node_ids()),
        "active_nodes": active_nodes,
        "latest_prediction": (
            prediction_history[-1] if prediction_history else None
        ),
        "total_predictions": len(prediction_history),
        "attack_predictions": attack_predictions,
        "active_attacks": active_attacks,
        "total_incidents": len(incidents),
        "ongoing_investigations": sum(
            1 for item in incidents
            if item.get("investigation_status") in ("NOT_STARTED", "RUNNING")
        ),
        "confirmed_incidents": sum(
            1 for item in incidents
            if item.get("final_verdict") == "CONFIRMED"
        ),
        "model_loaded": predictor is not None,
        "orchestrator_loaded": orchestrator is not None,
        "last_update": (
            prediction_history[-1]["timestamp"]
            if prediction_history else None
        ),
        "timestamp": now.isoformat()
    }


@app.get("/api/v1/dashboard/distribution")
def dashboard_distribution():

    distribution = {
        "NORMAL": 0,
        "ECLIPSE": 0,
        "RPC_FLOODING": 0
    }

    for prediction in latest_predictions.values():

        scenario = prediction["prediction"]

        if scenario in distribution:
            distribution[scenario] += 1

    return distribution


@app.get("/api/v1/dashboard/timeline")
def dashboard_timeline(
    limit: int = Query(default=100, ge=1, le=1000),
    node_id: str | None = None
):

    results = list(prediction_history)

    if node_id:

        results = [
            item for item in results
            if item["node_id"] == node_id
        ]

    return {
        "count": min(len(results), limit),
        "timeline": results[-limit:]
    }
