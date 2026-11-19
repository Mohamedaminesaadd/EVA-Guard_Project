
import pandas as pd
import numpy as np


def add_validator_features(df: pd.DataFrame) -> pd.DataFrame:
    """
    Create validator-related features for EVA-Guard.
    """

    data = df.copy()

    prefix = "validator_metrics_"

    # --------------------------------------------------
    # 1. ATTESTATION FAILURE RATIO
    # --------------------------------------------------

    failed_col = prefix + "attestation_failures_total"
    total_col = prefix + "attestations_total"

    if failed_col in data.columns and total_col in data.columns:

        failed = pd.to_numeric(data[failed_col], errors="coerce")
        total = pd.to_numeric(data[total_col], errors="coerce")

        data["validator_attestation_failure_ratio"] = (
            failed / total.where(total > 0)
        )

    # --------------------------------------------------
    # 2. MISSED ATTESTATION RATIO
    # --------------------------------------------------

    missed_col = prefix + "missed_attestations_total"

    if missed_col in data.columns and total_col in data.columns:

        missed = pd.to_numeric(data[missed_col], errors="coerce")
        total = pd.to_numeric(data[total_col], errors="coerce")

        data["validator_missed_attestation_ratio"] = (
            missed / (total + missed).where((total + missed) > 0)
        )

    # --------------------------------------------------
    # 3. BLOCK PROPOSAL FAILURE RATIO
    # --------------------------------------------------

    failed_proposals_col = prefix + "block_proposal_failures_total"
    proposed_col = prefix + "blocks_proposed_total"

    if (
        failed_proposals_col in data.columns
        and proposed_col in data.columns
    ):

        failed = pd.to_numeric(
            data[failed_proposals_col],
            errors="coerce"
        )

        proposed = pd.to_numeric(
            data[proposed_col],
            errors="coerce"
        )

        data["validator_proposal_failure_ratio"] = (
            failed / proposed.where(proposed > 0)
        )

    # --------------------------------------------------
    # 4. MISSED PROPOSAL RATIO
    # --------------------------------------------------

    missed_proposals_col = prefix + "missed_proposals_total"

    if (
        missed_proposals_col in data.columns
        and proposed_col in data.columns
    ):

        missed = pd.to_numeric(
            data[missed_proposals_col],
            errors="coerce"
        )

        proposed = pd.to_numeric(
            data[proposed_col],
            errors="coerce"
        )

        data["validator_missed_proposal_ratio"] = (
            missed / (proposed + missed).where(
                (proposed + missed) > 0
            )
        )

    # --------------------------------------------------
    # 5. VALIDATOR PARTICIPATION RATIO
    # --------------------------------------------------

    enabled_col = prefix + "validators_enabled"
    total_validators_col = prefix + "validators_total"

    if (
        enabled_col in data.columns
        and total_validators_col in data.columns
    ):

        enabled = pd.to_numeric(
            data[enabled_col],
            errors="coerce"
        )

        total = pd.to_numeric(
            data[total_validators_col],
            errors="coerce"
        )

        data["validator_participation_ratio"] = (
            enabled / total.where(total > 0)
        )

    # --------------------------------------------------
    # 6. EFFECTIVENESS FEATURES
    # --------------------------------------------------

    attestation_effectiveness = (
        prefix + "attestation_effectiveness"
    )

    proposal_effectiveness = (
        prefix + "proposal_effectiveness"
    )

    if attestation_effectiveness in data.columns:
        data["validator_attestation_effectiveness"] = pd.to_numeric(
            data[attestation_effectiveness],
            errors="coerce"
        )

    if proposal_effectiveness in data.columns:
        data["validator_proposal_effectiveness"] = pd.to_numeric(
            data[proposal_effectiveness],
            errors="coerce"
        )

    return data
