//! Append-only JSONL storage: one dataset file per experimental scenario.
//!
//! ```text
//! <data_dir>/normal.jsonl        one `Observation` per line
//! <data_dir>/eclipse.jsonl
//! <data_dir>/rpc_flooding.jsonl
//! ```
//!
//! Each line is a complete, compact JSON object ending with `\n`. JSON escapes
//! line breaks inside strings, so a line never contains another `\n`.
//!
//! # Guarantees
//!
//! - **One write per observation.** The line is serialized in memory first and
//!   then handed to the OS with a single `write_all` on a file opened with
//!   `O_APPEND`. Lines appended at the same time (threads or processes, on a
//!   local filesystem) never interleave, whatever their size, and a
//!   serialization error leaves the file untouched.
//! - **A damaged tail stays isolated.** If an earlier write was cut short (disk
//!   full, crash) the file can end without `\n`. The next append starts a new
//!   line first, so only that one line is unreadable and the observation that
//!   follows is intact. Readers should skip a line that does not parse.
//! - **Appends of one process are serialized.** Checking the tail and writing
//!   happen under a lock, so the check never sees a line another thread is
//!   still writing. Several *processes* appending to the same file stay safe
//!   (no torn line), but may rarely leave a blank line: readers should skip
//!   those too.
//! - **The data directory may disappear while running** (for example when it is
//!   wiped between two scenarios): the next append creates it again.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use thiserror::Error;

use crate::models::observation::{Observation, Scenario};

/// Serializes the appends of this process (see the module documentation).
///
/// It guards no data, so a lock poisoned by a panicking thread is still usable.
static APPEND_LOCK: Mutex<()> = Mutex::new(());

/// Errors that can occur while storing observations.
#[derive(Debug, Error)]
pub enum JsonlStorageError {
    #[error("Storage I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Observation serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// JSONL storage separated by experimental scenario.
#[derive(Debug, Clone)]
pub struct JsonlStorage {
    data_dir: PathBuf,
}

impl JsonlStorage {
    /// Creates the data directory (and its parents) if it does not exist.
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self, io::Error> {
        let data_dir = data_dir.as_ref().to_path_buf();

        fs::create_dir_all(&data_dir)?;

        Ok(Self { data_dir })
    }

    /// Returns the dataset path for an observation's scenario.
    pub fn dataset_path(&self, observation: &Observation) -> PathBuf {
        self.scenario_path(observation.scenario)
    }

    /// Returns the dataset path of a scenario (`<data_dir>/<dataset_name>.jsonl`).
    pub fn scenario_path(&self, scenario: Scenario) -> PathBuf {
        self.data_dir
            .join(format!("{}.jsonl", scenario.dataset_name()))
    }

    /// Serializes and appends one observation to its scenario dataset.
    ///
    /// The observation is written as a single line, all at once: it is either
    /// serialized and appended, or the file is left as it was (see the module
    /// documentation for the cases of a write that fails halfway).
    pub fn append(&self, observation: &Observation) -> Result<(), JsonlStorageError> {
        let _serialized = APPEND_LOCK.lock().unwrap_or_else(PoisonError::into_inner);

        append_line(&self.dataset_path(observation), observation)
    }

    /// Forces the datasets written so far to stable storage (`fsync`).
    ///
    /// Every [`append`](Self::append) already hands its line to the operating
    /// system, so a crash of this process loses nothing. Call this before a
    /// planned shutdown, or at the end of a scenario, to also survive a power
    /// loss. A dataset that does not exist yet is skipped.
    pub fn flush(&self) -> Result<(), io::Error> {
        for scenario in Scenario::ALL {
            // Append mode: the handle is writable on every platform, which
            // `sync_all` requires on Windows, and it never changes the file.
            match OpenOptions::new()
                .append(true)
                .open(self.scenario_path(scenario))
            {
                Ok(file) => file.sync_all()?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }

        Ok(())
    }

    /// Returns the data directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

/// Serializes `value` as one compact JSON line ending with `\n`.
fn encode_line<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut line = serde_json::to_vec(value)?;
    line.push(b'\n');
    Ok(line)
}

/// Appends `value` to `path` as one line, in a single write.
fn append_line<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), JsonlStorageError> {
    // Serialize before touching the file: a failure here writes nothing.
    let line = encode_line(value)?;

    let mut file = open_dataset(path)?;
    let previous_line_closed = ends_with_newline(&mut file)?;

    write_line(&mut file, &line, previous_line_closed)?;

    Ok(())
}

/// Writes `line` with one `write` call.
///
/// When the previous line was left open (cut short), a `\n` goes first in the
/// same call, so that line stays one bad line and `line` starts on its own.
fn write_line(out: &mut impl Write, line: &[u8], previous_line_closed: bool) -> io::Result<()> {
    if previous_line_closed {
        return out.write_all(line);
    }

    let mut buffer = Vec::with_capacity(line.len() + 1);
    buffer.push(b'\n');
    buffer.extend_from_slice(line);

    out.write_all(&buffer)
}

/// Opens a dataset for appending, creating it (and its directory) if needed.
///
/// The handle is also readable: `ends_with_newline` looks at the last byte.
fn open_dataset(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).append(true);

    match options.open(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // The data directory was removed while running.
            if let Some(directory) = path.parent() {
                fs::create_dir_all(directory)?;
            }
            options.open(path)
        }
        result => result,
    }
}

/// True when the file is empty or its last byte is `\n`.
///
/// Only the read position moves: with `O_APPEND` a write always goes to the end.
fn ends_with_newline(file: &mut (impl Read + Seek)) -> io::Result<bool> {
    let length = file.seek(SeekFrom::End(0))?;

    if length == 0 {
        return Ok(true);
    }

    file.seek(SeekFrom::Start(length - 1))?;

    let mut last = [0u8; 1];

    match file.read_exact(&mut last) {
        Ok(()) => Ok(last[0] == b'\n'),
        // The file was truncated since we measured it: nothing to close.
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(true),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::observation::{Component, ComponentMetrics};
    use serde::ser::{Error as _, SerializeStruct, Serializer};
    use serde_json::Value;
    use std::collections::{BTreeSet, HashMap};
    use std::error::Error as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::Duration;

    /// Unique directory under the system temp dir, removed on drop (even when
    /// the test fails). The tests never touch the project's `data/` directory.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);

            let path = std::env::temp_dir().join(format!(
                "eva-guard-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            // A previous run killed before its cleanup may have left this name.
            let _ = fs::remove_dir_all(&path);

            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn observation(node_id: &str, scenario: Scenario) -> Observation {
        Observation::new(
            node_id,
            scenario,
            Duration::from_secs(5),
            ComponentMetrics::empty(Component::Geth),
        )
    }

    fn create_observation(scenario: Scenario) -> Observation {
        observation("node-01", scenario)
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    /// A value whose serialization fails after it has already produced output.
    struct FailsMidway;

    impl Serialize for FailsMidway {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            struct Failing;

            impl Serialize for Failing {
                fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                    Err(S::Error::custom("cannot be serialized"))
                }
            }

            let mut state = serializer.serialize_struct("FailsMidway", 2)?;
            state.serialize_field("first", &1)?;
            state.serialize_field("second", &Failing)?;
            state.end()
        }
    }

    // ------------------------------------------------------------------ paths

    #[test]
    fn selects_correct_dataset_for_each_scenario() {
        let dir = TempDir::new("paths");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        assert_eq!(
            storage.dataset_path(&create_observation(Scenario::Normal)),
            dir.path().join("normal.jsonl")
        );
        assert_eq!(
            storage.dataset_path(&create_observation(Scenario::Eclipse)),
            dir.path().join("eclipse.jsonl")
        );
        assert_eq!(
            storage.dataset_path(&create_observation(Scenario::RpcFlooding)),
            dir.path().join("rpc_flooding.jsonl")
        );
    }

    #[test]
    fn every_scenario_has_its_own_dataset_file() {
        let dir = TempDir::new("distinct");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        let paths: BTreeSet<PathBuf> = Scenario::ALL
            .into_iter()
            .map(|scenario| storage.scenario_path(scenario))
            .collect();

        assert_eq!(paths.len(), Scenario::ALL.len());
        assert!(paths.iter().all(|path| path.starts_with(dir.path())));
    }

    #[test]
    fn new_creates_missing_nested_directories() {
        let dir = TempDir::new("nested");
        let nested = dir.path().join("a").join("b").join("data");

        let storage = JsonlStorage::new(&nested).unwrap();

        assert!(nested.is_dir());
        assert_eq!(storage.data_dir(), nested.as_path());
    }

    #[test]
    fn new_fails_when_the_path_is_a_file() {
        let dir = TempDir::new("not-a-dir");
        fs::create_dir_all(dir.path()).unwrap();
        let file = dir.path().join("data");
        fs::write(&file, b"x").unwrap();

        assert!(JsonlStorage::new(&file).is_err());
    }

    // ----------------------------------------------------------------- append

    #[test]
    fn appends_observation_to_correct_dataset() {
        let dir = TempDir::new("append");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let observation = create_observation(Scenario::Normal);

        storage.append(&observation).unwrap();

        let content = read(&dir.path().join("normal.jsonl"));

        assert_eq!(content.lines().count(), 1);

        let decoded: Observation = serde_json::from_str(content.trim()).unwrap();

        assert_eq!(decoded, observation);
        assert_eq!(decoded.node_id, "node-01");
        assert_eq!(decoded.scenario, Scenario::Normal);
        assert_eq!(decoded.collection_interval, 5);
    }

    #[test]
    fn a_line_is_the_compact_json_followed_by_one_newline() {
        let dir = TempDir::new("format");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let observation = create_observation(Scenario::Normal);

        storage.append(&observation).unwrap();

        let content = read(&dir.path().join("normal.jsonl"));

        assert_eq!(
            content,
            format!("{}\n", serde_json::to_string(&observation).unwrap())
        );
        assert_eq!(content.matches('\n').count(), 1);
    }

    #[test]
    fn a_stored_line_has_the_documented_top_level_keys() {
        let dir = TempDir::new("keys");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        storage
            .append(&create_observation(Scenario::Eclipse))
            .unwrap();

        let line = read(&dir.path().join("eclipse.jsonl"));
        let value: Value = serde_json::from_str(line.trim()).unwrap();
        let object = value.as_object().unwrap();
        let keys: BTreeSet<&str> = object.keys().map(String::as_str).collect();

        assert_eq!(
            keys,
            BTreeSet::from([
                "timestamp",
                "node_id",
                "scenario",
                "collection_interval",
                "component",
                "metrics",
                "system",
            ])
        );
        assert_eq!(object["scenario"], "ECLIPSE");
        assert_eq!(object["component"], "geth");
    }

    #[test]
    fn observations_are_appended_in_order() {
        let dir = TempDir::new("order");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        for index in 0..20 {
            storage
                .append(&observation(&format!("node-{index:02}"), Scenario::Normal))
                .unwrap();
        }

        let ids: Vec<String> = read(&dir.path().join("normal.jsonl"))
            .lines()
            .map(|line| serde_json::from_str::<Observation>(line).unwrap().node_id)
            .collect();
        let expected: Vec<String> = (0..20).map(|index| format!("node-{index:02}")).collect();

        assert_eq!(ids, expected);
    }

    #[test]
    fn separates_observations_by_scenario() {
        let dir = TempDir::new("scenarios");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        for scenario in Scenario::ALL {
            storage.append(&create_observation(scenario)).unwrap();
        }

        for scenario in Scenario::ALL {
            let content = read(&storage.scenario_path(scenario));
            let decoded: Observation = serde_json::from_str(content.trim()).unwrap();

            assert_eq!(content.lines().count(), 1);
            assert_eq!(decoded.scenario, scenario);
        }

        let files = fs::read_dir(dir.path()).unwrap().count();

        assert_eq!(files, Scenario::ALL.len());
    }

    #[test]
    fn preserves_existing_data_when_reopened() {
        let dir = TempDir::new("reopen");
        let observation = create_observation(Scenario::Normal);

        {
            let storage = JsonlStorage::new(dir.path()).unwrap();
            storage.append(&observation).unwrap();
        }

        {
            let storage = JsonlStorage::new(dir.path()).unwrap();
            storage.append(&observation).unwrap();
        }

        assert_eq!(read(&dir.path().join("normal.jsonl")).lines().count(), 2);
    }

    #[test]
    fn line_breaks_and_unicode_in_strings_stay_on_one_line() {
        let dir = TempDir::new("escaping");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let observation = observation("a\nb\r\n\"c\"\t\u{2028}é日本", Scenario::Normal);

        storage.append(&observation).unwrap();

        let content = read(&dir.path().join("normal.jsonl"));

        assert_eq!(content.matches('\n').count(), 1);

        let decoded: Observation = serde_json::from_str(content.trim_end()).unwrap();

        assert_eq!(decoded, observation);
    }

    #[test]
    fn append_to_an_unusable_location_is_an_io_error() {
        let dir = TempDir::new("unusable");
        let data = dir.path().join("data");
        let storage = JsonlStorage::new(&data).unwrap();

        // The data directory is replaced by a regular file.
        fs::remove_dir_all(&data).unwrap();
        fs::write(&data, b"x").unwrap();

        let error = storage
            .append(&create_observation(Scenario::Normal))
            .unwrap_err();

        assert!(matches!(error, JsonlStorageError::Io(_)), "{error}");
    }

    // ------------------------------------------------------ failure behaviour

    #[test]
    fn a_failed_serialization_writes_nothing() {
        let dir = TempDir::new("failed-serialization");
        fs::create_dir_all(dir.path()).unwrap();
        let path = dir.path().join("normal.jsonl");

        // No dataset yet: it must not even be created.
        let error = append_line(&path, &FailsMidway).unwrap_err();

        assert!(matches!(error, JsonlStorageError::Serialization(_)));
        assert!(!path.exists());

        // Existing dataset: it must be left byte for byte as it was.
        fs::write(&path, b"{\"kept\":true}\n").unwrap();

        assert!(append_line(&path, &FailsMidway).is_err());
        assert_eq!(read(&path), "{\"kept\":true}\n");
    }

    #[test]
    fn an_append_after_a_failed_one_is_intact() {
        let dir = TempDir::new("after-failure");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let path = storage.scenario_path(Scenario::Normal);
        let observation = create_observation(Scenario::Normal);

        assert!(append_line(&path, &FailsMidway).is_err());
        storage.append(&observation).unwrap();

        let content = read(&path);

        assert_eq!(content.lines().count(), 1);
        assert_eq!(
            serde_json::from_str::<Observation>(content.trim()).unwrap(),
            observation
        );
    }

    #[test]
    fn a_torn_last_line_is_isolated_from_the_next_observation() {
        let dir = TempDir::new("torn");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let path = storage.scenario_path(Scenario::Normal);
        let first = create_observation(Scenario::Normal);
        let second = observation("node-02", Scenario::Normal);

        // A write cut short by a full disk or a crash: half a line, no newline.
        let whole = serde_json::to_string(&first).unwrap();
        fs::write(&path, &whole.as_bytes()[..whole.len() / 2]).unwrap();

        storage.append(&second).unwrap();

        let content = read(&path);
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(serde_json::from_str::<Observation>(lines[0]).is_err());
        assert_eq!(
            serde_json::from_str::<Observation>(lines[1]).unwrap(),
            second
        );
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn a_complete_last_line_is_not_followed_by_a_blank_line() {
        let dir = TempDir::new("no-blank");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let path = storage.scenario_path(Scenario::Normal);

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();
        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();

        let content = read(&path);

        assert_eq!(content.lines().count(), 2);
        assert!(content.lines().all(|line| !line.is_empty()));
        assert!(!content.contains("\n\n"));
    }

    #[test]
    fn an_existing_empty_dataset_gets_no_leading_blank_line() {
        let dir = TempDir::new("empty-file");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let path = storage.scenario_path(Scenario::Normal);
        fs::write(&path, b"").unwrap();

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();

        let content = read(&path);

        assert!(content.starts_with('{'), "{content:?}");
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn the_data_directory_removed_while_running_is_created_again() {
        let dir = TempDir::new("wiped");
        let data = dir.path().join("data");
        let storage = JsonlStorage::new(&data).unwrap();

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();
        fs::remove_dir_all(&data).unwrap();

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();

        assert_eq!(read(&data.join("normal.jsonl")).lines().count(), 1);
    }

    // ------------------------------------------------------ building blocks

    /// Records the size of each `write` call it receives.
    #[derive(Default)]
    struct CountingWriter {
        calls: Vec<usize>,
        bytes: Vec<u8>,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.calls.push(buffer.len());
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_line_is_handed_over_in_one_write_call() {
        let line = b"{\"a\":1}\n";

        let mut closed = CountingWriter::default();
        write_line(&mut closed, line, true).unwrap();

        assert_eq!(closed.calls, vec![line.len()]);
        assert_eq!(closed.bytes, line);

        let mut open = CountingWriter::default();
        write_line(&mut open, line, false).unwrap();

        // The newline that closes the previous line travels with the new one.
        assert_eq!(open.calls, vec![line.len() + 1]);
        assert_eq!(open.bytes, b"\n{\"a\":1}\n");
    }

    #[test]
    fn ends_with_newline_looks_at_the_last_byte_only() {
        for (content, expected) in [
            (&b""[..], true),
            (b"\n", true),
            (b"a\n", true),
            (b"a\nb\n", true),
            (b"a", false),
            (b"a\nb", false),
            (b"\r", false),
        ] {
            let mut cursor = io::Cursor::new(content);

            assert_eq!(
                ends_with_newline(&mut cursor).unwrap(),
                expected,
                "{content:?}"
            );
        }
    }

    #[test]
    fn ends_with_newline_tolerates_a_file_truncated_meanwhile() {
        /// Reports 10 bytes when measured, then has nothing left to read.
        struct Truncated;

        impl Seek for Truncated {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                Ok(match position {
                    SeekFrom::End(_) => 10,
                    SeekFrom::Start(offset) => offset,
                    SeekFrom::Current(_) => unreachable!(),
                })
            }
        }

        impl Read for Truncated {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Ok(0)
            }
        }

        assert!(ends_with_newline(&mut Truncated).unwrap());
    }

    // ------------------------------------------------------------ concurrency

    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;
    // Well above the 8 KiB buffer of a `BufWriter`: such a line cannot be
    // streamed in one piece, which is what used to tear it.
    const PADDING: usize = 50_000;

    fn node_id_of(thread: usize) -> String {
        format!("thread-{thread}-{}", "x".repeat(PADDING))
    }

    /// Calls `append` from `THREADS` threads, `PER_THREAD` times each, then
    /// returns the content of `dataset`.
    fn hammer(dataset: &Path, append: impl Fn(&Observation) + Send + Sync + 'static) -> String {
        let append = Arc::new(append);

        let handles: Vec<_> = (0..THREADS)
            .map(|thread| {
                let append = Arc::clone(&append);

                std::thread::spawn(move || {
                    let observation = observation(&node_id_of(thread), Scenario::Normal);

                    for _ in 0..PER_THREAD {
                        append(&observation);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        read(dataset)
    }

    /// Every non-blank line decodes and each thread has all of its lines.
    fn assert_lines_intact(content: &str) {
        let mut per_thread: HashMap<String, usize> = HashMap::new();

        for line in content.lines().filter(|line| !line.is_empty()) {
            let decoded: Observation = serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("torn line ({error}): {} bytes", line.len()));

            *per_thread.entry(decoded.node_id).or_default() += 1;
        }

        assert_eq!(per_thread.len(), THREADS);

        for thread in 0..THREADS {
            assert_eq!(per_thread[&node_id_of(thread)], PER_THREAD);
        }
    }

    #[test]
    fn concurrent_writers_never_tear_a_line() {
        // `append_line` takes no lock: this is what several processes do. Only
        // the single `O_APPEND` write keeps the lines whole. A blank line may
        // appear (the tail check can see a line that is half written).
        let dir = TempDir::new("writers");
        fs::create_dir_all(dir.path()).unwrap();
        let path = dir.path().join("normal.jsonl");

        let content = hammer(&path, {
            let path = path.clone();

            move |observation| append_line(&path, observation).unwrap()
        });

        assert_lines_intact(&content);
    }

    #[test]
    fn concurrent_appends_of_one_process_leave_no_blank_line() {
        let dir = TempDir::new("process");
        let storage = Arc::new(JsonlStorage::new(dir.path()).unwrap());

        let content = hammer(&storage.scenario_path(Scenario::Normal), {
            let storage = Arc::clone(&storage);

            move |observation| storage.append(observation).unwrap()
        });

        assert_lines_intact(&content);
        assert!(!content.contains("\n\n"), "blank line in the dataset");
        assert_eq!(content.lines().count(), THREADS * PER_THREAD);
    }

    #[test]
    fn an_append_waits_while_another_one_holds_the_process_lock() {
        let dir = TempDir::new("lock");
        let storage = JsonlStorage::new(dir.path()).unwrap();
        let path = storage.scenario_path(Scenario::Normal);
        let (done, finished) = mpsc::channel();

        let guard = APPEND_LOCK.lock().unwrap_or_else(PoisonError::into_inner);

        let writer = std::thread::spawn(move || {
            storage
                .append(&create_observation(Scenario::Normal))
                .unwrap();
            done.send(()).unwrap();
        });

        // However slow the machine is, a blocked append cannot finish.
        assert!(finished.recv_timeout(Duration::from_millis(300)).is_err());
        assert!(!path.exists(), "the append wrote while the lock was held");

        drop(guard);

        finished.recv_timeout(Duration::from_secs(10)).unwrap();
        writer.join().unwrap();

        assert_eq!(read(&path).lines().count(), 1);
    }

    #[test]
    fn a_thread_that_panicked_holding_the_lock_does_not_disable_storage() {
        let dir = TempDir::new("poisoned");
        let storage = JsonlStorage::new(dir.path()).unwrap();

        let poisoner = std::thread::spawn(|| {
            let _guard = APPEND_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
            panic!("poisoning the append lock on purpose");
        });

        assert!(poisoner.join().is_err());
        assert!(APPEND_LOCK.is_poisoned());

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();

        assert_eq!(
            read(&storage.scenario_path(Scenario::Normal))
                .lines()
                .count(),
            1
        );
    }

    // ------------------------------------------------------------------ flush

    #[test]
    fn flush_works_with_and_without_datasets() {
        let dir = TempDir::new("flush");
        let data = dir.path().join("data");
        let storage = JsonlStorage::new(&data).unwrap();

        // Nothing written yet.
        storage.flush().unwrap();

        storage
            .append(&create_observation(Scenario::Normal))
            .unwrap();
        storage
            .append(&create_observation(Scenario::Eclipse))
            .unwrap();

        let before = read(&data.join("normal.jsonl"));

        storage.flush().unwrap();

        // Flushing never changes the data.
        assert_eq!(read(&data.join("normal.jsonl")), before);
        assert_eq!(read(&data.join("eclipse.jsonl")).lines().count(), 1);
        assert!(!data.join("rpc_flooding.jsonl").exists());

        // Data directory gone: nothing to flush, not an error.
        fs::remove_dir_all(&data).unwrap();
        storage.flush().unwrap();
    }

    // ----------------------------------------------------------------- errors

    #[test]
    fn errors_keep_their_messages_and_expose_their_source() {
        let io_error = JsonlStorageError::from(io::Error::other("disk gone"));

        assert_eq!(io_error.to_string(), "Storage I/O error: disk gone");
        assert!(io_error.source().is_some());

        let json_error = serde_json::from_str::<u8>("x").unwrap_err();
        let serialization = JsonlStorageError::from(json_error);

        assert!(serialization
            .to_string()
            .starts_with("Observation serialization error: "));
        assert!(serialization.source().is_some());
    }

    #[test]
    fn encode_line_ends_with_exactly_one_newline() {
        assert_eq!(encode_line(&1).unwrap(), b"1\n");
        assert_eq!(encode_line("a\nb").unwrap(), b"\"a\\nb\"\n");
        assert!(encode_line(&FailsMidway).is_err());
    }
}
