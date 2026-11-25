
import asyncio
import json
import logging
from datetime import datetime, timezone

from agentic.schemas import Incident, AttackType
from agentic.orchestrator import AgentOrchestrator


logging.basicConfig(level=logging.INFO)


async def main():

    incident = Incident(
        incident_id="TEST-ORCH-001",
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

    print("\n========== EVA-GUARD ORCHESTRATOR TEST ==========")

    orchestrator = AgentOrchestrator(max_retries=1)

    report = await orchestrator.run_investigation(incident)

    print("\n========== FINAL INVESTIGATION REPORT ==========")
    print(json.dumps(report, indent=2, ensure_ascii=False))

    print("\n========== INVESTIGATION SUMMARY ==========")
    print("Incident ID:", report["incident_id"])
    print("Node:", report["node_id"])
    print("Status:", report["investigation_status"])
    print("Verdict:", report["final_verdict"])

    print("\n========== WORKFLOW HISTORY ==========")

    for step in report.get("history", []):
        print(
            step["step"],
            "->",
            step["status"]
        )


if __name__ == "__main__":
    asyncio.run(main())
