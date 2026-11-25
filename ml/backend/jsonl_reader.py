
import json
from pathlib import Path


class JSONLReader:

    def __init__(self, file_path: Path, start_at_end=True):
        self.file_path = Path(file_path)
        self.offset = 0

        if start_at_end and self.file_path.exists():
            self.offset = self.file_path.stat().st_size

    def read_new_lines(self):
        if not self.file_path.exists():
            return []

        current_size = self.file_path.stat().st_size

        if current_size < self.offset:
            self.offset = 0

        observations = []

        with self.file_path.open("rb") as file:
            file.seek(self.offset)
            data = file.read()

            last_newline = data.rfind(b"\n")

            if last_newline == -1:
                return []

            complete_data = data[:last_newline + 1]
            self.offset += len(complete_data)

            for line in complete_data.splitlines():
                if not line.strip():
                    continue

                try:
                    observation = json.loads(line.decode("utf-8"))
                    observations.append(observation)

                except (json.JSONDecodeError, UnicodeDecodeError) as error:
                    print(f"[JSONL READER] Invalid line: {error}")

        return observations
