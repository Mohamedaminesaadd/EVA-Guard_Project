from pydantic import Field

from .common import (
    BaseSchema,
    AttackType,
    Evidence,
    RAGSource
)


class Hypothesis(BaseSchema):
    incident_id: str

    agent: str = "HYPOTHESIS_AGENT"

    attack_hypothesis: AttackType

    reasoning: str

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

    confidence: float = Field(ge=0.0, le=1.0)

    status: str = "PENDING_CONFIRMATION"