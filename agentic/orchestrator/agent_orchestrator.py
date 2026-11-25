
import asyncio
import logging
from datetime import datetime, timezone
from typing import Any

from langgraph.graph import StateGraph, START, END

from agentic.detection.hypothesis_agent import HypothesisAgent
from agentic.confirmation.confirmation_agent import ConfirmationAgent

from agentic.schemas import Incident

from .workflow_state import WorkflowState


logger = logging.getLogger(__name__)


class AgentOrchestrator:

    def __init__(
        self,
        hypothesis_agent=None,
        confirmation_agent=None,
        max_retries=1
    ):
        self.hypothesis_agent = (
            hypothesis_agent or HypothesisAgent()
        )

        self.confirmation_agent = (
            confirmation_agent or ConfirmationAgent()
        )

        self.max_retries = max_retries

        self.graph = self._build_graph()

        logger.info("EVA-Guard Orchestrator initialized.")

    def _add_history(self, state, step, status, details=None):
        history = list(state.get("history", []))

        history.append({
            "step": step,
            "status": status,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "details": details
        })

        return history

    async def _hypothesis_node(self, state: WorkflowState):

        incident = state["incident"]

        logger.info(
            "Running Hypothesis Agent for incident %s",
            incident.incident_id
        )

        try:
            hypothesis = await self.hypothesis_agent.ainvestigate(
                incident
            )

            return {
                "hypothesis": hypothesis,
                "current_step": "HYPOTHESIS_COMPLETED",
                "retry_count": 0,
                "error": None,
                "history": self._add_history(
                    state,
                    "HYPOTHESIS_AGENT",
                    "SUCCESS",
                    "Hypothesis generated successfully."
                )
            }

        except Exception as exc:
            retry_count = state.get("retry_count", 0) + 1

            logger.exception("Hypothesis Agent failed.")

            return {
                "retry_count": retry_count,
                "error": str(exc),
                "current_step": "HYPOTHESIS_FAILED",
                "history": self._add_history(
                    state,
                    "HYPOTHESIS_AGENT",
                    "ERROR",
                    str(exc)
                )
            }

    def _route_after_hypothesis(self, state: WorkflowState):

        if state.get("hypothesis") is not None:
            return "validate"

        if state.get("retry_count", 0) <= self.max_retries:
            return "retry"

        return "failed"

    async def _validate_hypothesis_node(self, state: WorkflowState):

        incident = state["incident"]
        hypothesis = state["hypothesis"]

        errors = []

        if hypothesis.incident_id != incident.incident_id:
            errors.append("Incident ID mismatch.")

        if hypothesis.attack_hypothesis != incident.attack_type:
            errors.append("Attack type mismatch.")

        if not hypothesis.reasoning:
            errors.append("Hypothesis reasoning is missing.")

        if errors:
            error_message = " ".join(errors)

            return {
                "error": error_message,
                "current_step": "HYPOTHESIS_VALIDATION_FAILED",
                "history": self._add_history(
                    state,
                    "VALIDATE_HYPOTHESIS",
                    "ERROR",
                    error_message
                )
            }

        return {
            "error": None,
            "current_step": "HYPOTHESIS_VALIDATED",
            "history": self._add_history(
                state,
                "VALIDATE_HYPOTHESIS",
                "SUCCESS",
                "Hypothesis structure and incident consistency validated."
            )
        }

    def _route_after_validation(self, state: WorkflowState):

        if state.get("error"):
            return "failed"

        return "confirmation"

    async def _confirmation_node(self, state: WorkflowState):

        incident = state["incident"]
        hypothesis = state["hypothesis"]

        logger.info(
            "Running Confirmation Agent for incident %s",
            incident.incident_id
        )

        try:
            confirmation = await self.confirmation_agent.aconfirm(
                incident=incident,
                hypothesis=hypothesis
            )

            return {
                "confirmation": confirmation,
                "retry_count": 0,
                "error": None,
                "current_step": "CONFIRMATION_COMPLETED",
                "history": self._add_history(
                    state,
                    "CONFIRMATION_AGENT",
                    "SUCCESS",
                    f"Verdict: {confirmation.verdict.value}"
                )
            }

        except Exception as exc:
            retry_count = state.get("retry_count", 0) + 1

            logger.exception("Confirmation Agent failed.")

            return {
                "retry_count": retry_count,
                "error": str(exc),
                "current_step": "CONFIRMATION_FAILED",
                "history": self._add_history(
                    state,
                    "CONFIRMATION_AGENT",
                    "ERROR",
                    str(exc)
                )
            }

    def _route_after_confirmation(self, state: WorkflowState):

        if state.get("confirmation") is not None:
            return "evaluate"

        if state.get("retry_count", 0) <= self.max_retries:
            return "retry"

        return "failed"

    async def _evaluate_verdict_node(self, state: WorkflowState):

        confirmation = state["confirmation"]

        verdict = confirmation.verdict.value

        return {
            "verdict": verdict,
            "current_step": "VERDICT_EVALUATED",
            "history": self._add_history(
                state,
                "EVALUATE_VERDICT",
                "SUCCESS",
                f"Final verdict: {verdict}"
            )
        }

    async def _build_report_node(self, state: WorkflowState):

        incident = state["incident"]
        hypothesis = state.get("hypothesis")
        confirmation = state.get("confirmation")

        if confirmation is not None:
            status = "COMPLETED"
            verdict = confirmation.verdict.value
        elif hypothesis is not None:
            status = "PARTIAL"
            verdict = "INCONCLUSIVE"
        else:
            status = "FAILED"
            verdict = "INCONCLUSIVE"

        report = {
            "incident_id": incident.incident_id,
            "node_id": incident.node_id,
            "attack_type": incident.attack_type.value,
            "investigation_status": status,
            "final_verdict": verdict,
            "ml_prediction": incident.ml_prediction,
            "ml_confidence": incident.ml_confidence,
            "hypothesis": (
                hypothesis.model_dump(mode="json")
                if hypothesis else None
            ),
            "confirmation": (
                confirmation.model_dump(mode="json")
                if confirmation else None
            ),
            "error": state.get("error"),
            "history": state.get("history", []),
            "generated_at": datetime.now(timezone.utc).isoformat()
        }

        logger.info(
            "Investigation report generated for %s: %s",
            incident.incident_id,
            status
        )

        return {
            "report": report,
            "verdict": verdict,
            "investigation_status": status,
            "current_step": "REPORT_GENERATED",
            "history": self._add_history(
                state,
                "BUILD_REPORT",
                "SUCCESS",
                f"Report generated with status {status}."
            )
        }

    def _build_graph(self):

        workflow = StateGraph(WorkflowState)

        workflow.add_node(
            "hypothesis_agent",
            self._hypothesis_node
        )

        workflow.add_node(
            "validate_hypothesis",
            self._validate_hypothesis_node
        )

        workflow.add_node(
            "confirmation_agent",
            self._confirmation_node
        )

        workflow.add_node(
            "evaluate_verdict",
            self._evaluate_verdict_node
        )

        workflow.add_node(
            "build_report",
            self._build_report_node
        )

        workflow.add_edge(START, "hypothesis_agent")

        workflow.add_conditional_edges(
            "hypothesis_agent",
            self._route_after_hypothesis,
            {
                "retry": "hypothesis_agent",
                "validate": "validate_hypothesis",
                "failed": "build_report"
            }
        )

        workflow.add_conditional_edges(
            "validate_hypothesis",
            self._route_after_validation,
            {
                "confirmation": "confirmation_agent",
                "failed": "build_report"
            }
        )

        workflow.add_conditional_edges(
            "confirmation_agent",
            self._route_after_confirmation,
            {
                "retry": "confirmation_agent",
                "evaluate": "evaluate_verdict",
                "failed": "build_report"
            }
        )

        workflow.add_edge(
            "evaluate_verdict",
            "build_report"
        )

        workflow.add_edge(
            "build_report",
            END
        )

        return workflow.compile()

    async def run_investigation(
        self,
        incident: Incident
    ) -> dict[str, Any]:

        initial_state: WorkflowState = {
            "incident": incident,
            "retry_count": 0,
            "max_retries": self.max_retries,
            "history": [],
            "current_step": "STARTED",
            "investigation_status": "RUNNING",
            "error": None
        }

        logger.info(
            "Starting investigation for incident %s",
            incident.incident_id
        )

        final_state = await self.graph.ainvoke(initial_state)

        return final_state["report"]

    async def run_parallel(
        self,
        incidents: list[Incident]
    ) -> list[dict[str, Any]]:

        tasks = [
            self.run_investigation(incident)
            for incident in incidents
        ]

        results = await asyncio.gather(
            *tasks,
            return_exceptions=True
        )

        reports = []

        for incident, result in zip(incidents, results):

            if isinstance(result, Exception):
                reports.append({
                    "incident_id": incident.incident_id,
                    "node_id": incident.node_id,
                    "investigation_status": "FAILED",
                    "final_verdict": "INCONCLUSIVE",
                    "error": str(result)
                })
            else:
                reports.append(result)

        return reports
