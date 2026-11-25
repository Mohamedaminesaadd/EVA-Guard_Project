use std::fs;
use std::io;
use std::path::Path;

/// `st` value of an ESTABLISHED socket in `/proc/<pid>/net/tcp{,6}`.
const TCP_ESTABLISHED: &str = "01";

/// Index of the `st` column: `sl local_address rem_address st ...`.
const STATE_COLUMN: usize = 3;

pub struct ConnectionsCollector;

impl ConnectionsCollector {
    /// Counts established TCP connections (IPv4 and IPv6) in the network
    /// namespace of a container.
    ///
    /// The host PID must belong to the target container. A container in
    /// host network mode shares the host namespace, so the count is then
    /// the whole host's.
    ///
    /// Reads `/proc`, which is quick but synchronous: call it from
    /// `spawn_blocking` when running inside an async task.
    pub fn collect(container_pid: u64) -> io::Result<u64> {
        Self::collect_from(Path::new("/proc"), container_pid)
    }

    /// Checks that `pid` still runs inside the container `container_id`.
    ///
    /// A PID read from `docker inspect` goes stale when the container stops
    /// or restarts, and the kernel may give it to an unrelated process: its
    /// sockets must then not be counted. Docker places every container in a
    /// cgroup whose path contains the full container ID, for both cgroup v1
    /// and v2 (`/system.slice/docker-<id>.scope`, `/docker/<id>`).
    pub fn pid_belongs_to_container(
        proc_root: &Path,
        pid: u64,
        container_id: &str,
    ) -> io::Result<bool> {
        if pid == 0 || container_id.is_empty() {
            return Ok(false);
        }

        let cgroup = fs::read_to_string(proc_root.join(pid.to_string()).join("cgroup"))?;

        Ok(cgroup.contains(container_id))
    }

    pub(crate) fn collect_from(proc_root: &Path, container_pid: u64) -> io::Result<u64> {
        if container_pid == 0 {
            // Docker reports PID 0 for a container that is not running.
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "container PID 0: the container is not running",
            ));
        }

        let net_dir = proc_root.join(container_pid.to_string()).join("net");

        let tcp = count_established_in(&net_dir.join("tcp"))?;

        // The IPv6 table does not exist on hosts booted with ipv6.disable=1.
        let tcp6 = match count_established_in(&net_dir.join("tcp6")) {
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error),
        };

        tcp.checked_add(tcp6)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "connection count overflow"))
    }
}

/// Reads one `/proc/net/tcp*` table and counts its ESTABLISHED sockets.
///
/// The error keeps its kind but names the file, so a log line is enough
/// to see which table could not be read.
fn count_established_in(path: &Path) -> io::Result<u64> {
    let table = fs::read_to_string(path)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))?;

    Ok(count_established(&table))
}

/// Counts the ESTABLISHED sockets of a `/proc/net/tcp*` table.
///
/// The first line is the column header. Lines too short to have a state
/// column are ignored.
fn count_established(table: &str) -> u64 {
    table
        .lines()
        .skip(1)
        .filter(|line| line.split_whitespace().nth(STATE_COLUMN) == Some(TCP_ESTABLISHED))
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::container::test_support::TempDir;

    const HEADER: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode";

    fn row(index: usize, state: &str) -> String {
        format!(
            "   {index}: 0100007F:1F90 0100007F:E2A4 {state} 00000000:00000000 00:00000000 00000000  1000        0 {} 1 0000000000000000 100 0 0 10 0",
            20_000 + index
        )
    }

    fn table(states: &[&str]) -> String {
        let mut lines = vec![HEADER.to_string()];
        lines.extend(states.iter().enumerate().map(|(i, s)| row(i, s)));
        lines.join("\n") + "\n"
    }

    /// Builds `<root>/<pid>/net/{tcp,tcp6}`; None skips the file.
    fn fake_proc(pid: u64, tcp: Option<&str>, tcp6: Option<&str>) -> TempDir {
        let root = TempDir::new("proc");
        let net = root.path().join(pid.to_string()).join("net");
        fs::create_dir_all(&net).expect("create net dir");

        if let Some(content) = tcp {
            fs::write(net.join("tcp"), content).expect("write tcp");
        }

        if let Some(content) = tcp6 {
            fs::write(net.join("tcp6"), content).expect("write tcp6");
        }

        root
    }

    // ---- pid_belongs_to_container -------------------------------------------------

    fn proc_with_cgroup(pid: u64, cgroup: &str) -> TempDir {
        let root = TempDir::new("proc-cgroup");
        let dir = root.path().join(pid.to_string());
        fs::create_dir_all(&dir).expect("create pid dir");
        fs::write(dir.join("cgroup"), cgroup).expect("write cgroup");
        root
    }

    #[test]
    fn a_pid_in_the_container_cgroup_belongs_to_it() {
        let v2 = proc_with_cgroup(42, "0::/system.slice/docker-abc123def.scope\n");
        let v1 = proc_with_cgroup(43, "12:pids:/docker/abc123def\n11:cpu:/docker/abc123def\n");

        assert!(
            ConnectionsCollector::pid_belongs_to_container(v2.path(), 42, "abc123def").unwrap()
        );
        assert!(
            ConnectionsCollector::pid_belongs_to_container(v1.path(), 43, "abc123def").unwrap()
        );
    }

    #[test]
    fn a_reused_pid_of_another_cgroup_does_not_belong_to_it() {
        let root = proc_with_cgroup(42, "0::/user.slice/user-1000.slice/session-2.scope\n");

        assert!(
            !ConnectionsCollector::pid_belongs_to_container(root.path(), 42, "abc123def").unwrap()
        );
    }

    #[test]
    fn a_vanished_pid_is_an_error_and_pid_zero_never_belongs() {
        let root = TempDir::new("proc-empty");

        assert!(ConnectionsCollector::pid_belongs_to_container(root.path(), 42, "abc").is_err());
        assert!(!ConnectionsCollector::pid_belongs_to_container(root.path(), 0, "abc").unwrap());
        assert!(!ConnectionsCollector::pid_belongs_to_container(root.path(), 42, "").unwrap());
    }

    // ---- count_established -----------------------------------------------------

    #[test]
    fn counts_only_established_sockets() {
        // 01 ESTABLISHED, 0A LISTEN, 06 TIME_WAIT, 08 CLOSE_WAIT.
        let content = table(&["0A", "01", "06", "01", "08", "01"]);

        assert_eq!(count_established(&content), 3);
    }

    #[test]
    fn header_only_table_has_no_connection() {
        assert_eq!(count_established(&table(&[])), 0);
    }

    #[test]
    fn empty_content_has_no_connection() {
        assert_eq!(count_established(""), 0);
    }

    #[test]
    fn the_header_line_is_never_counted() {
        // A header that looks like data must still be skipped.
        let content = format!("0: a b 01\n{}", row(0, "0A"));

        assert_eq!(count_established(&content), 0);
    }

    #[test]
    fn malformed_lines_are_ignored() {
        let content = format!("{HEADER}\n\n   garbage\n0: a b\n{}\n", row(1, "01"));

        assert_eq!(count_established(&content), 1);
    }

    #[test]
    fn state_must_match_exactly() {
        // 0x10 and 0x11 are not ESTABLISHED even though they contain a 1.
        let content = table(&["10", "11", "1", "001"]);

        assert_eq!(count_established(&content), 0);
    }

    // ---- collect_from ------------------------------------------------------------

    #[test]
    fn adds_ipv4_and_ipv6_connections() {
        let proc = fake_proc(
            4242,
            Some(&table(&["01", "0A", "01"])),
            Some(&table(&["01", "06"])),
        );

        assert_eq!(
            ConnectionsCollector::collect_from(proc.path(), 4242).unwrap(),
            3
        );
    }

    #[test]
    fn missing_ipv6_table_counts_as_zero() {
        let proc = fake_proc(4242, Some(&table(&["01", "01"])), None);

        assert_eq!(
            ConnectionsCollector::collect_from(proc.path(), 4242).unwrap(),
            2
        );
    }

    #[test]
    fn missing_ipv4_table_is_an_error() {
        let proc = fake_proc(4242, None, Some(&table(&["01"])));

        let error = ConnectionsCollector::collect_from(proc.path(), 4242).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn error_names_the_file_that_failed() {
        let proc = fake_proc(4242, None, None);

        let error = ConnectionsCollector::collect_from(proc.path(), 4242).unwrap_err();

        assert!(error.to_string().contains("tcp"), "{error}");
        assert!(error.to_string().contains("4242"), "{error}");
    }

    #[test]
    fn unknown_pid_is_an_error() {
        let proc = fake_proc(4242, Some(&table(&["01"])), None);

        let error = ConnectionsCollector::collect_from(proc.path(), 9999).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn pid_zero_is_rejected() {
        let proc = fake_proc(0, Some(&table(&["01"])), None);

        let error = ConnectionsCollector::collect_from(proc.path(), 0).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn other_processes_tables_are_not_mixed_in() {
        let proc = fake_proc(100, Some(&table(&["01"])), None);
        let other = proc.path().join("200").join("net");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("tcp"), table(&["01", "01", "01"])).unwrap();

        assert_eq!(
            ConnectionsCollector::collect_from(proc.path(), 100).unwrap(),
            1
        );
    }

    // ---- the real /proc ----------------------------------------------------------

    #[cfg(target_os = "linux")]
    #[test]
    fn counts_real_connections_of_the_current_process() {
        use std::net::{TcpListener, TcpStream};

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let client = TcpStream::connect(listener.local_addr().unwrap()).expect("connect");
        let (server, _) = listener.accept().expect("accept");

        let count = ConnectionsCollector::collect(u64::from(std::process::id()))
            .expect("read the current process tables");

        // Both ends of the connection are ESTABLISHED sockets of this
        // namespace; other tests may add more.
        assert!(count >= 2, "expected at least 2, got {count}");

        drop((client, server));
    }
}
