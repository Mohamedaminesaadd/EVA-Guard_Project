import asyncio
from datetime import datetime, timezone

from agentic.schemas import Incident, Hypothesis, AttackType, Evidence
from agentic.confirmation import ConfirmationAgent


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

    hypothesis = Hypothesis(
        incident_id="TEST-INC-001",
        attack_hypothesis=AttackType.RPC_FLOODING,
        reasoning="High RPC request rate, latency and failed requests are consistent with possible RPC flooding.",
        supporting_evidence=[
            Evidence(
                description="High RPC request rate observed.",
                metric="geth_rpc_requests_per_sec",
                observed_value=1500
            ),
            Evidence(
                description="High RPC latency observed.",
                metric="geth_rpc_latency_ms",
                observed_value=850
            ),
            Evidence(
                description="Failed RPC requests observed.",
                metric="geth_rpc_failed_requests_total",
                observed_value=250
            )
        ],
        contradicting_evidence=[],
        alternative_explanations=[
            "Legitimate high traffic.",
            "Temporary network congestion."
        ],
        confidence=0.80
    )

    agent = ConfirmationAgent()

    confirmation = await agent.aconfirm(
        incident=incident,
        hypothesis=hypothesis
    )

    print("\n========== CONFIRMATION REPORT ==========")
    print(confirmation.model_dump_json(indent=2))

    print("\n========== FINAL VERDICT ==========")
    print(confirmation.verdict.value)


if __name__ == "__main__":
    asyncio.run(main())