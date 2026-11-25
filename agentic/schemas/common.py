from enum import Enum
from pydantic import BaseModel, ConfigDict, Field


class BaseSchema(BaseModel):
    model_config = ConfigDict(
        extra="forbid",
        validate_assignment=True
    )

    schema_version: str = "1.0"


class AttackType(str, Enum):
    ECLIPSE = "ECLIPSE"
    RPC_FLOODING = "RPC_FLOODING"


class IncidentStatus(str, Enum):
    NEW = "NEW"
    INVESTIGATING = "INVESTIGATING"
    CONFIRMED = "CONFIRMED"
    REJECTED = "REJECTED"
    CLOSED = "CLOSED"


class InvestigationStatus(str, Enum):
    NOT_STARTED = "NOT_STARTED"
    RUNNING = "RUNNING"
    COMPLETED = "COMPLETED"
    FAILED = "FAILED"


class Verdict(str, Enum):
    CONFIRMED = "CONFIRMED"
    SUSPECTED = "SUSPECTED"
    REJECTED = "REJECTED"
    INCONCLUSIVE = "INCONCLUSIVE"


class RAGSource(BaseSchema):
    source_id: str
    file_name: str
    source: str
    category: str | None = None
    distance: float | None = None


class Evidence(BaseSchema):
    description: str
    metric: str | None = None
    observed_value: str | float | int | None = None
    expected_value: str | float | int | None = None