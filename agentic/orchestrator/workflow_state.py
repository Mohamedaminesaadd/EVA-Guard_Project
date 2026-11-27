from typing import Any, TypedDict

from agentic.schemas import Incident, Hypothesis, Confirmation


class WorkflowState(TypedDict, total=False):
    incident: Incident
    hypothesis: Hypothesis
    confirmation: Confirmation

    verdict: str
    report: dict[str, Any]

    current_step: str
    history: list[dict[str, Any]]

    retry_count: int
    max_retries: int

    error: str | None
    investigation_status: str