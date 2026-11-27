
import asyncio
from datetime import datetime, timezone

from agentic.schemas import Incident, AttackType
from agentic.detection import HypothesisAgent


async def main():

    incident = Incident(
        incident_id="TEST-INC-001",
        node_id="node1",
        attack_type=AttackType.RPC_FLOODING,
        ml_prediction="RPC_FLOODING",
        ml_confidence=0.96,
        probabilities={
            "NORMAL": 0.01,
            "ECLIPSE": 0.03,
            "RPC_FLOODING": 0.96
        },
        prediction_timestamp=datetime.now(timezone.utc),
        observation_snapshot={
            "geth_rpc_requests_per_sec": 1500,
            "geth_rpc_latency_ms": 850,
            "geth_rpc_failed_requests_total": 250,
            "system_cpu_usage_percent": 85,
            "system_tcp_established_connections": 450
        }
    )

    agent = HypothesisAgent()

    hypothesis = await agent.ainvestigate(incident)

    print("\n========== HYPOTHESIS REPORT ==========")
    print(hypothesis.model_dump_json(indent=2))


if __name__ == "__main__":
    asyncio.run(main())
