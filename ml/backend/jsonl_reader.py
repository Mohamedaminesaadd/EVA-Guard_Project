
import json
import logging
from pathlib import Path
from typing import Any

logger = logging.getLogger(__name__)


class JSONLReader:
    """
    Incremental JSONL reader.

    Reads only new complete JSON lines without rereading
    previously processed observations.

    Handles:
    - Partial lines
    - File truncation
    - File replacement / rotation
    - Invalid JSON
    - Invalid UTF-8
    """

    def __init__(
        self,
        file_path: Path,
        start_at_end: bool = True
    ):
        self.file_path = Path(file_path)
        self.offset = 0

        self._file_identity = None

        if self.file_path.exists():
            stat = self.file_path.stat()

            self._file_identity = (
                stat.st_dev,
                stat.st_ino
            )

            if start_at_end:
                self.offset = stat.st_size

        logger.info(
            "[JSONL READER] Initialized: %s | offset=%d",
            self.file_path,
            self.offset
        )

    def read_new_lines(self) -> list[dict[str, Any]]:
        """
        Return newly appended complete JSON objects.
        """

        if not self.file_path.exists():
            return []

        observations = []

        try:
            with self.file_path.open("rb") as file:

                stat = self.file_path.stat()

                current_identity = (
                    stat.st_dev,
                    stat.st_ino
                )

                # Detect file replacement or rotation.
                if current_identity != self._file_identity:
                    logger.warning(
                        "[JSONL READER] File replaced. Resetting offset."
                    )

                    self.offset = 0
                    self._file_identity = current_identity

                # Detect truncation.
                if stat.st_size < self.offset:
                    logger.warning(
                        "[JSONL READER] File truncated. Resetting offset."
                    )

                    self.offset = 0

                file.seek(self.offset)

                data = file.read()

                if not data:
                    return []

                # Process only complete lines.
                last_newline = data.rfind(b"\n")

                if last_newline == -1:
                    # Keep the offset unchanged.
                    # The incomplete line will be read next time.
                    return []

                complete_data = data[:last_newline + 1]

                # Advance offset only for consumed bytes.
                self.offset += len(complete_data)

                for line_number, line in enumerate(
                    complete_data.splitlines(),
                    start=1
                ):
                    line = line.strip()

                    if not line:
                        continue

                    try:
                        observation = json.loads(
                            line.decode("utf-8")
                        )

                        if not isinstance(observation, dict):
                            logger.warning(
                                "[JSONL READER] Ignoring non-object JSON."
                            )
                            continue

                        observations.append(observation)

                    except (json.JSONDecodeError, UnicodeDecodeError) as error:
                        logger.warning(
                            "[JSONL READER] Invalid JSON line: %s",
                            error
                        )

        except OSError:
            logger.exception(
                "[JSONL READER] Error reading file: %s",
                self.file_path
            )
            return []

        if observations:
            logger.debug(
                "[JSONL READER] Read %d new observations. Offset=%d",
                len(observations),
                self.offset
            )

        return observations
