from datetime import datetime, timezone
from typing import Any
from uuid import uuid4

from pydantic import Field

from .common import (
    BaseSchema,
    AttackType,
    IncidentStatus,
    InvestigationStatus
)


class Incident(BaseSchema):
    incident_id: str = Field(
        default_factory=lambda: str(uuid4())
    )

    node_id: str
    attack_type: AttackType

    ml_prediction: str
    ml_confidence: float = Field(ge=0.0, le=1.0)

    probabilities: dict[str, float] = Field(
        default_factory=dict
    )

    prediction_timestamp: datetime
    created_at: datetime = Field(
        default_factory=lambda: datetime.now(timezone.utc)
    )

    status: IncidentStatus = IncidentStatus.NEW

    investigation_status: InvestigationStatus = (
        InvestigationStatus.NOT_STARTED
    )

    observation_snapshot: dict[str, Any] = Field(
        default_factory=dict
    )