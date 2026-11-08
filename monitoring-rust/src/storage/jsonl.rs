
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::models::observation::Observation;

/// Errors that can occur while storing observations.
#[derive(Debug)]
pub enum JsonlStorageError {
    Io(io::Error),
    Serialization(serde_json::Error),
}

impl std::fmt::Display for JsonlStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "Storage I/O error: {}", error),
            Self::Serialization(error) => {
                write!(f, "Observation serialization error: {}", error)
            }
        }
    }
}

impl std::error::Error for JsonlStorageError {}

impl From<io::Error> for JsonlStorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for JsonlStorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

/// JSON Lines storage for monitoring observations.
pub struct JsonlStorage {
    writer: BufWriter<File>,
}

impl JsonlStorage {
    /// Open or create a JSONL file.
    /// Existing content is preserved. New observations are appended.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let path = path.as_ref();

        // Create parent directories if necessary.
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    /// Serialize and append one observation.
    pub fn append(
        &mut self,
        observation: &Observation,
    ) -> Result<(), JsonlStorageError> {
        serde_json::to_writer(&mut self.writer, observation)?;

        // One observation = one line.
        self.writer.write_all(b"\n")?;

        // Flush after each observation.
        self.writer.flush()?;

        Ok(())
    }

    /// Explicitly flush pending data.
    pub fn flush(&mut self) -> Result<(), io::Error> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_append_observation() {
        let path = "data/test_observations.jsonl";

        // Remove previous test file if it exists.
        let _ = fs::remove_file(path);

        let mut storage = JsonlStorage::new(path)
            .expect("Failed to create JSONL storage");

        let observation =
            Observation::new("geth-node-01", Duration::from_secs(5));

        storage
            .append(&observation)
            .expect("Failed to append observation");

        storage.flush().expect("Failed to flush storage");

        // Read generated file.
        let content = fs::read_to_string(path)
            .expect("Failed to read JSONL file");

        // Verify one JSON line was written.
        assert_eq!(content.lines().count(), 1);

        // Verify valid JSON.
        let parsed: Observation =
            serde_json::from_str(content.trim())
                .expect("Invalid JSON observation");

        assert_eq!(parsed.node_id, "geth-node-01");
        assert_eq!(parsed.collection_interval, 5);

        // Cleanup.
        drop(storage);
        fs::remove_file(path)
            .expect("Failed to remove test file");
    }

    #[test]
    fn test_multiple_observations_and_persistence() {
        let path = "data/test_persistence.jsonl";

        let _ = fs::remove_file(path);

        // First session: write two observations.
        {
            let mut storage = JsonlStorage::new(path)
                .expect("Failed to create storage");

            let observation1 =
                Observation::new("geth-node-01", Duration::from_secs(5));

            let observation2 =
                Observation::new("geth-node-01", Duration::from_secs(5));

            storage.append(&observation1).unwrap();
            storage.append(&observation2).unwrap();
        }

        // Second session: reopen and append another observation.
        {
            let mut storage = JsonlStorage::new(path)
                .expect("Failed to reopen storage");

            let observation3 =
                Observation::new("geth-node-01", Duration::from_secs(5));

            storage.append(&observation3).unwrap();
        }

        // Verify all three observations are present.
        let content = fs::read_to_string(path)
            .expect("Failed to read JSONL file");

        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 3);

        for line in &lines {
            let parsed: Observation =
                serde_json::from_str(line)
                    .expect("Invalid JSON line");

            assert_eq!(parsed.node_id, "geth-node-01");
        }

        fs::remove_file(path)
            .expect("Failed to remove test file");
    }
}
