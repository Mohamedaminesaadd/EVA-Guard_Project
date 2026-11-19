
import pandas as pd
import numpy as np


def add_network_features(df: pd.DataFrame) -> pd.DataFrame:
    """
    Create network-related features for EVA-Guard.

    Features:
    - RPC failure rate
    - Peer loss ratio
    - Peer churn ratio
    - Peer IP diversity ratio
    - Network traffic total
    - Network traffic asymmetry
    """

    data = df.copy()

    # --------------------------------------------------
    # 1. RPC FAILURE RATE
    # --------------------------------------------------

    for component in ["geth", "beacon", "validator"]:

        failed_col = (
            f"{component}_metrics_rpc_failed_requests_total"
        )

        total_col = (
            f"{component}_metrics_rpc_requests_total"
        )

        if failed_col in data.columns and total_col in data.columns:

            failed = pd.to_numeric(
                data[failed_col],
                errors="coerce"
            )

            total = pd.to_numeric(
                data[total_col],
                errors="coerce"
            )

            data[f"{component}_rpc_failure_ratio"] = (
                failed / total.where(total > 0)
            )

    # --------------------------------------------------
    # 2. PEER LOSS RATIO
    # --------------------------------------------------

    for component in ["geth", "beacon", "validator"]:

        peer_col = f"{component}_metrics_peer_count"

        disconnected_col = (
            f"{component}_metrics_disconnected_peers"
        )

        if peer_col in data.columns and disconnected_col in data.columns:

            peers = pd.to_numeric(
                data[peer_col],
                errors="coerce"
            )

            disconnected = pd.to_numeric(
                data[disconnected_col],
                errors="coerce"
            )

            data[f"{component}_peer_disconnection_ratio"] = (
                disconnected / peers.where(peers > 0)
            )

    # --------------------------------------------------
    # 3. PEER CHURN RATIO
    # --------------------------------------------------

    for component in ["geth", "beacon", "validator"]:

        churn_col = f"{component}_metrics_peer_churn_count"
        peer_col = f"{component}_metrics_peer_count"

        if churn_col in data.columns and peer_col in data.columns:

            churn = pd.to_numeric(
                data[churn_col],
                errors="coerce"
            )

            peers = pd.to_numeric(
                data[peer_col],
                errors="coerce"
            )

            data[f"{component}_peer_churn_ratio"] = (
                churn / peers.where(peers > 0)
            )

    # --------------------------------------------------
    # 4. PEER IP DIVERSITY
    # --------------------------------------------------

    for component in ["geth", "beacon", "validator"]:

        unique_ips_col = (
            f"{component}_metrics_unique_peer_ips"
        )

        peer_col = f"{component}_metrics_peer_count"

        if unique_ips_col in data.columns and peer_col in data.columns:

            unique_ips = pd.to_numeric(
                data[unique_ips_col],
                errors="coerce"
            )

            peers = pd.to_numeric(
                data[peer_col],
                errors="coerce"
            )

            data[f"{component}_peer_ip_diversity"] = (
                unique_ips / peers.where(peers > 0)
            )

    # --------------------------------------------------
    # 5. NETWORK TRAFFIC
    # --------------------------------------------------

    for component in ["geth", "beacon", "validator"]:

        rx_col = (
            f"{component}_system_network_rx_bytes_per_sec"
        )

        tx_col = (
            f"{component}_system_network_tx_bytes_per_sec"
        )

        if rx_col in data.columns and tx_col in data.columns:

            rx = pd.to_numeric(
                data[rx_col],
                errors="coerce"
            )

            tx = pd.to_numeric(
                data[tx_col],
                errors="coerce"
            )

            data[f"{component}_network_traffic_bytes_per_sec"] = (
                rx + tx
            )

            data[f"{component}_network_traffic_asymmetry"] = (
                (rx - tx).abs() / (rx + tx).where((rx + tx) > 0)
            )

    return data
