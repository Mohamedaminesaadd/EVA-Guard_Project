from pydantic import Field

from .common import (
    BaseSchema,
    Evidence,
    RAGSource,
    Verdict
)


class Confirmation(BaseSchema):
    incident_id: str

    agent: str = "CONFIRMATION_AGENT"

    verdict: Verdict

    confidence: float = Field(ge=0.0, le=1.0)

    supporting_evidence: list[Evidence] = Field(
        default_factory=list
    )

    contradicting_evidence: list[Evidence] = Field(
        default_factory=list
    )

    alternative_explanations: list[str] = Field(
        default_factory=list
    )

    rag_sources: list[RAGSource] = Field(
        default_factory=list
    )

    reasoning: str

    recommendations: list[str] = Field(
        default_factory=list
    )

    status: str = "COMPLETED"