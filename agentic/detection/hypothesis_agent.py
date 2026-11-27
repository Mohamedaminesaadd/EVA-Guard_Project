
import os
import asyncio
import json
import logging

from pathlib import Path
from dotenv import load_dotenv

from langchain_core.prompts import ChatPromptTemplate
from langchain_core.output_parsers import PydanticOutputParser
from langchain_openai import ChatOpenAI

from langchain_ollama import ChatOllama

from knowledge_base.retrieval.query_engine import KnowledgeQueryEngine

from agentic.schemas import (
    Incident,
    Hypothesis,
    Evidence,
    RAGSource
)

from agentic.detection.prompts import (
    SYSTEM_PROMPT,
    USER_PROMPT
)


logger = logging.getLogger("EVA-GUARD.HYPOTHESIS")


class HypothesisAgent:

    def __init__(
        self,
        model: str = "deepseek-chat",
        base_url: str = "http://localhost:11434",
        top_k: int = 5,
        rag_model: str = "qwen2.5:14b"
    ):

        logger.info("Initializing Hypothesis Agent with DeepSeek")

        # ==========================================
        # LOAD ROOT ENVIRONMENT
        # ==========================================

        project_root = Path(__file__).resolve().parents[2]

        load_dotenv(project_root / ".env")

        api_key = os.getenv("token_deepseek")

        if not api_key:
            raise ValueError(
                "token_deepseek not found in root .env file"
            )

        # ==========================================
        # DEEPSEEK LLM
        # ==========================================

        self.llm = ChatOpenAI(
            model=model,
            api_key=api_key,
            base_url="https://api.deepseek.com",
            temperature=0.1,
            max_retries=3,
            timeout=60
        )

        # ==========================================
        # RAG
        # ==========================================

        # Keep the existing RAG implementation.
        # Its internal LLM remains Ollama until
        # query_engine.py is migrated separately.

        self.rag = KnowledgeQueryEngine(
            model=rag_model,
            base_url=base_url,
            top_k=top_k
        )

        self.top_k = top_k

        # ==========================================
        # OUTPUT PARSER
        # ==========================================

        self.parser = PydanticOutputParser(
            pydantic_object=Hypothesis
        )

        # ==========================================
        # PROMPT
        # ==========================================

        self.prompt = ChatPromptTemplate.from_messages([
            ("system", SYSTEM_PROMPT),
            ("human", USER_PROMPT)
        ])

        logger.info("Hypothesis Agent initialized with DeepSeek")

    # --------------------------------------------------
    # METRIC EXTRACTION
    # --------------------------------------------------

    def _flatten_metrics(
        self,
        data: dict,
        prefix: str = ""
    ) -> dict:

        metrics = {}

        for key, value in data.items():

            name = f"{prefix}{key}"

            if isinstance(value, dict):

                metrics.update(
                    self._flatten_metrics(
                        value,
                        prefix=f"{name}."
                    )
                )

            elif isinstance(value, (int, float)) and not isinstance(
                value, bool
            ):

                metrics[name] = value

        return metrics

    # --------------------------------------------------
    # METRIC ANALYSIS
    # --------------------------------------------------

    def _analyze_metrics(
        self,
        incident: Incident
    ) -> list[Evidence]:

        snapshot = incident.observation_snapshot

        metrics = self._flatten_metrics(snapshot)

        attack = incident.attack_type.value

        if attack == "RPC_FLOODING":

            keywords = [
                "rpc",
                "latency",
                "request",
                "failed",
                "tcp",
                "cpu",
                "network"
            ]

        elif attack == "ECLIPSE":

            keywords = [
                "peer",
                "churn",
                "unique_peer",
                "block_age",
                "sync",
                "finality",
                "reorg",
                "propagation",
                "network"
            ]

        else:

            keywords = []

        selected_metrics = {
            name: value
            for name, value in metrics.items()
            if any(
                keyword in name.lower()
                for keyword in keywords
            )
        }

        if not selected_metrics:
            selected_metrics = metrics

        evidence = []

        for name, value in list(selected_metrics.items())[:30]:

            evidence.append(
                Evidence(
                    description=f"Observed monitoring metric: {name}",
                    metric=name,
                    observed_value=value
                )
            )

        return evidence

    # --------------------------------------------------
    # RAG QUESTION GENERATION
    # --------------------------------------------------

    def _build_rag_question(
        self,
        incident: Incident
    ) -> str:

        if incident.attack_type.value == "RPC_FLOODING":

            return (
                "What network behaviors and RPC metrics "
                "are associated with Ethereum RPC flooding? "
                "Explain RPC request rates, latency, failed "
                "requests and relevant network indicators."
            )

        return (
            "What network behaviors and peer connectivity "
            "indicators are associated with Ethereum eclipse "
            "attacks? Explain peer churn, peer diversity, "
            "network isolation and synchronization indicators."
        )

    # --------------------------------------------------
    # RAG RETRIEVAL
    # --------------------------------------------------

    async def _retrieve_knowledge(
        self,
        incident: Incident
    ) -> dict:

        question = self._build_rag_question(incident)

        logger.info("Retrieving RAG knowledge")

        result = await asyncio.to_thread(
            self.rag.query,
            question=question,
            category="attacks",
            k=self.top_k
        )

        return result

    # --------------------------------------------------
    # SOURCE CONVERSION
    # --------------------------------------------------

    def _convert_sources(
        self,
        sources: list[dict]
    ) -> list[RAGSource]:

        converted = []

        for source in sources:

            file_name = source.get("file_name") or "Unknown"

            converted.append(
                RAGSource(
                    source_id=str(source.get("source_id", "")),
                    file_name=file_name,
                    source=source.get("source") or file_name,
                    category=source.get("category"),
                    distance=source.get("distance")
                )
            )

        return converted

    # --------------------------------------------------
    # EVIDENCE VALIDATION
    # --------------------------------------------------

    def _validate_evidence(
        self,
        evidence: list[Evidence],
        available_metrics: list[Evidence]
    ) -> list[Evidence]:

        allowed = {
            (
                item.metric,
                str(item.observed_value)
            )
            for item in available_metrics
        }

        validated = []

        for item in evidence:

            key = (
                item.metric,
                str(item.observed_value)
            )

            if item.metric and key in allowed:
                validated.append(item)

        return validated

    # --------------------------------------------------
    # MAIN INVESTIGATION
    # --------------------------------------------------

    async def ainvestigate(
        self,
        incident: Incident
    ) -> Hypothesis:

        logger.info(
            "Starting investigation for incident %s",
            incident.incident_id
        )

        # ==========================================
        # STEP 1: ANALYZE MONITORING METRICS
        # ==========================================

        metric_evidence = self._analyze_metrics(
            incident
        )

        logger.info(
            "Extracted %s monitoring metrics",
            len(metric_evidence)
        )

        # ==========================================
        # STEP 2: RETRIEVE TECHNICAL KNOWLEDGE
        # ==========================================

        rag_result = await self._retrieve_knowledge(
            incident
        )

        rag_sources = self._convert_sources(
            rag_result.get("sources", [])
        )

        # ==========================================
        # STEP 3: PREPARE INVESTIGATION CONTEXT
        # ==========================================

        context = {
            "incident_id": incident.incident_id,
            "node_id": incident.node_id,
            "ml_prediction": incident.ml_prediction,
            "ml_confidence": incident.ml_confidence,
            "attack_type": incident.attack_type.value,
            "probabilities": incident.probabilities,
            "observation_snapshot": incident.observation_snapshot,

            "metric_evidence": [
                item.model_dump()
                for item in metric_evidence
            ],

            "rag_question": rag_result.get("question", ""),
            "rag_answer": rag_result.get("answer", ""),

            "rag_sources": [
                item.model_dump()
                for item in rag_sources
            ]
        }

        prompt_context = {
            **context,

            "observation_snapshot": json.dumps(
                context["observation_snapshot"],
                indent=2,
                default=str
            ),

            "probabilities": json.dumps(
                context["probabilities"],
                indent=2,
                default=str
            ),

            "metric_evidence": json.dumps(
                context["metric_evidence"],
                indent=2,
                default=str
            ),

            "rag_sources": json.dumps(
                context["rag_sources"],
                indent=2,
                default=str
            ),

            "format_instructions": (
                self.parser.get_format_instructions()
            )
        }

        messages = self.prompt.format_messages(
            **prompt_context
        )

        # ==========================================
        # STEP 4: GENERATE STRUCTURED HYPOTHESIS
        # ==========================================

        logger.info("Generating hypothesis with DeepSeek")

        response = await self.llm.ainvoke(messages)

        hypothesis = self.parser.parse(
            str(response.content)
        )

        # ==========================================
        # STEP 5: VERIFY INCIDENT IDENTITY
        # ==========================================

        if hypothesis.incident_id != incident.incident_id:

            raise ValueError(
                "Generated hypothesis incident_id mismatch"
            )

        if hypothesis.attack_hypothesis != incident.attack_type:

            raise ValueError(
                "Generated hypothesis attack type mismatch"
            )

        # ==========================================
        # STEP 6: VALIDATE METRIC EVIDENCE
        # ==========================================

        hypothesis.supporting_evidence = (
            self._validate_evidence(
                hypothesis.supporting_evidence,
                metric_evidence
            )
        )

        hypothesis.contradicting_evidence = (
            self._validate_evidence(
                hypothesis.contradicting_evidence,
                metric_evidence
            )
        )

        # ==========================================
        # STEP 7: ATTACH ACTUAL RAG SOURCES
        # ==========================================

        hypothesis.rag_sources = rag_sources

        logger.info(
            "Investigation completed for incident %s",
            incident.incident_id
        )

        return hypothesis
