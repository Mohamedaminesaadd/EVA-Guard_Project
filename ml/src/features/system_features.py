
import pandas as pd
import numpy as np


def add_system_features(df: pd.DataFrame) -> pd.DataFrame:
    """
    Create system-level features for EVA-Guard.

    Features:
    - CPU utilization
    - Memory utilization ratio
    - Total network traffic
    - Network traffic asymmetry
    - TCP established connections
    """

    data = df.copy()

    components = ["geth", "beacon", "validator"]

    for component in components:

        prefix = f"{component}_system_"

        # -----------------------------------------
        # 1. CPU USAGE
        # -----------------------------------------

        cpu_col = prefix + "cpu_usage_percent"

        if cpu_col in data.columns:

            data[f"{component}_cpu_utilization"] = (
                pd.to_numeric(data[cpu_col], errors="coerce")
            )

        # -----------------------------------------
        # 2. MEMORY UTILIZATION
        # -----------------------------------------

        used_col = prefix + "memory_used_mb"
        total_col = prefix + "memory_total_mb"

        if used_col in data.columns and total_col in data.columns:

            used = pd.to_numeric(
                data[used_col],
                errors="coerce"
            )

            total = pd.to_numeric(
                data[total_col],
                errors="coerce"
            )

            data[f"{component}_memory_utilization"] = (
                used / total.where(total > 0)
            )

            data[f"{component}_memory_available_mb"] = (
                total - used
            )

        # -----------------------------------------
        # 3. NETWORK TRAFFIC
        # -----------------------------------------

        rx_col = prefix + "network_rx_bytes_per_sec"
        tx_col = prefix + "network_tx_bytes_per_sec"

        if rx_col in data.columns and tx_col in data.columns:

            rx = pd.to_numeric(
                data[rx_col],
                errors="coerce"
            )

            tx = pd.to_numeric(
                data[tx_col],
                errors="coerce"
            )

            data[f"{component}_network_total_bytes_per_sec"] = (
                rx + tx
            )

            data[f"{component}_network_asymmetry"] = (
                (rx - tx).abs() /
                (rx + tx).where((rx + tx) > 0)
            )

        # -----------------------------------------
        # 4. TCP CONNECTIONS
        # -----------------------------------------

        tcp_col = prefix + "tcp_established_connections"

        if tcp_col in data.columns:

            data[f"{component}_tcp_connections"] = (
                pd.to_numeric(
                    data[tcp_col],
                    errors="coerce"
                )
            )

    return data
