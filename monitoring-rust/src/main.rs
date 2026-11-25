//! EVA-Guard monitoring engine: entry point.
//!
//! Usage:
//! - `monitoring-rust` runs the monitoring for one scenario (`EVA_SCENARIO`).
//! - `monitoring-rust --campaign` runs NORMAL, then ECLIPSE, then
//!   RPC_FLOODING back to back, applying and undoing each attack, and writes
//!   the three datasets in one run (`EVA_SCENARIO` is ignored).
//! - `monitoring-rust --campaign --sessions N` runs the whole
//!   NORMAL → ECLIPSE → RPC_FLOODING sequence N times. Each session is written
//!   to its own `sessionX` sub-directory (`data/session1`, `data/session2`, …)
//!   so the sessions stay separate. With one session (the default) the datasets
//!   stay directly in `EVA_DATA_DIR`, exactly as before.
//! - `monitoring-rust --check` only checks that every endpoint answers, then
//!   exits (status 1 if one fails).
//!
//! Environment variables (all optional):
//! - `NODES_CONFIG`: path of the nodes file (default `config/nodes.toml`)
//! - `EVA_SCENARIO`: `NORMAL`, `ECLIPSE` or `RPC_FLOODING` (default `NORMAL`),
//!   single-run only
//! - `EVA_DATA_DIR`: directory of the JSONL datasets (default `data`)
//! - `RUST_LOG`: log filter (default `info`)
//!
//! Campaign only:
//! - `EVA_ATTACK_TARGET`: victim node id (default: the first node)
//! - `EVA_PHASE_SECS`: duration of each scenario phase (default 180)
//! - `EVA_COOLDOWN_SECS`: pause between phases (default 15)
//! - `EVA_FLOOD_CONCURRENCY`: concurrent workers of the RPC flood (default 64)

use std::env;
use std::error::Error;
use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

use monitoring_rust::config::NodesConfig;
use monitoring_rust::models::observation::Scenario;
use monitoring_rust::monitoring::campaign::Campaign;
use monitoring_rust::monitoring::node_manager::NodeManager;
use monitoring_rust::monitoring::preflight::{all_ok, check_nodes, render_report};
use monitoring_rust::monitoring::scheduler::Scheduler;
use monitoring_rust::storage::JsonlStorage;

const ENV_SCENARIO: &str = "EVA_SCENARIO";
const ENV_DATA_DIR: &str = "EVA_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    println!("========================================");
    println!("     EVA-Guard Monitoring Engine");
    println!("========================================");

    // 1. Load configuration
    let config = NodesConfig::from_env()?;
    let scenario = scenario_from(env::var(ENV_SCENARIO).ok().as_deref())?;
    let data_dir = env::var_os(ENV_DATA_DIR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));

    let node_ids: Vec<&str> = config.nodes.iter().map(|node| node.id.as_str()).collect();

    println!("\n[1] Configuration loaded");
    println!("Nodes ({}): {}", node_ids.len(), node_ids.join(", "));
    println!(
        "Collection interval: {:?}",
        config.settings.collection_interval()
    );
    println!("Request timeout: {:?}", config.settings.request_timeout());
    println!("Scenario: {scenario}");

    // 2. Check every endpoint once, and say why one is unreachable
    let cli = Cli::from_args(env::args().skip(1))?;
    let request_timeout = config.settings.request_timeout();
    let manager = NodeManager::new(&config)?;
    let checks = check_nodes(manager.nodes(), request_timeout).await;

    println!("\n[2] {}", render_report(&checks, request_timeout));

    if cli.check_only {
        std::process::exit(if all_ok(&checks) { 0 } else { 1 });
    }

    if !all_ok(&checks) {
        println!(
            "Monitoring continues: a node that is down is part of the dataset. \
             Its metrics are null until it answers.\n"
        );
    }

    if cli.campaign {
        return run_campaign(&config, data_dir, cli.sessions).await;
    }

    // 3. Open the dataset storage
    let storage = JsonlStorage::new(&data_dir)?;

    println!("[3] Storage ready");
    println!("Dataset: {}", storage.scenario_path(scenario).display());

    // 4. Build the scheduler: clients, collectors and time budgets of every node
    let scheduler = Scheduler::from_config(&config, scenario, storage)?;

    println!("\n[4] Monitoring scheduler initialized");

    // 5. Start continuous monitoring
    println!("\n========================================");
    println!("       CONTINUOUS MONITORING");
    println!("========================================");

    println!("Monitoring started...");
    println!("Press Ctrl+C to stop.\n");

    let report = scheduler.run(shutdown_signal()).await;

    println!("\n========================================");
    println!("       MONITORING STOPPED");
    println!("========================================");
    println!("Cycles completed:     {}", report.cycles_completed);
    println!("Cycles skipped (late): {}", report.cycles_skipped);
    println!("Nodes not collected:  {}", report.nodes_unmeasured);
    println!("Observations written: {}", report.observations_written);
    println!("Write failures:       {}", report.write_failures);

    if !report.flushed {
        return Err("the final flush of the dataset failed".into());
    }

    Ok(())
}

/// Runs the full scenario campaign (NORMAL → ECLIPSE → RPC_FLOODING) once per
/// session. With more than one session each session writes to its own
/// `sessionX` sub-directory of `data_dir`; a single session writes to
/// `data_dir` directly (unchanged behaviour). A Ctrl+C / SIGTERM during any
/// session stops the whole run instead of starting the next session.
async fn run_campaign(
    config: &NodesConfig,
    data_dir: PathBuf,
    sessions: usize,
) -> Result<(), Box<dyn Error>> {
    for session in 1..=sessions {
        let session_dir = if sessions == 1 {
            data_dir.clone()
        } else {
            data_dir.join(format!("session{session}"))
        };
        std::fs::create_dir_all(&session_dir)?;

        let campaign = Campaign::from_env(config, session_dir.clone())?;

        if sessions > 1 {
            println!("\n########################################");
            println!("  SESSION {session} / {sessions}");
            println!("########################################");
        }

        println!("[3] Scenario campaign");
        println!("Datasets:  {}", session_dir.display());
        println!("Victim:    {}", campaign.target());
        println!(
            "Per phase: {:?}  (cooldown {:?})",
            campaign.phase_duration(),
            campaign.cooldown()
        );
        let plan: Vec<&str> = campaign
            .scenarios()
            .iter()
            .map(|scenario| scenario.as_str())
            .collect();
        println!("Sequence:  {}", plan.join(" → "));

        println!("\n========================================");
        println!("       SCENARIO CAMPAIGN");
        println!("========================================");
        println!("Running the three phases. Press Ctrl+C to stop early.\n");

        let report = campaign.run(shutdown_signal()).await;

        println!("\n========================================");
        println!("       CAMPAIGN FINISHED");
        println!("========================================");
        for phase in &report.phases {
            match (&phase.run, &phase.skipped) {
                (Some(run), _) => {
                    let attack = match &phase.attack {
                        Some(attack) => {
                            format!(" | attack actions={} errors={}", attack.actions, attack.errors)
                        }
                        None => String::new(),
                    };
                    println!(
                        "{:<13} cycles={:<3} observations={:<4}{}{}",
                        phase.scenario.as_str(),
                        run.cycles_completed,
                        run.observations_written,
                        attack,
                        if phase.interrupted { " (interrupted)" } else { "" },
                    );
                }
                (None, Some(reason)) => {
                    println!("{:<13} SKIPPED: {reason}", phase.scenario.as_str());
                }
                (None, None) => {}
            }
        }

        // A signal during a session stops the whole run, not just this session.
        if report.phases.iter().any(|phase| phase.interrupted) {
            if session < sessions {
                println!("\nRun interrupted — stopping before the remaining sessions.");
            }
            break;
        }
    }

    Ok(())
}

/// Parsed command line. `--check`, `--campaign` and `--sessions N` are the only
/// options; anything else is refused rather than ignored.
#[derive(Debug, PartialEq, Eq)]
struct Cli {
    check_only: bool,
    campaign: bool,
    sessions: usize,
}

impl Default for Cli {
    fn default() -> Self {
        Cli {
            check_only: false,
            campaign: false,
            sessions: 1,
        }
    }
}

impl Cli {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Self, Box<dyn Error>> {
        let mut cli = Cli::default();
        let mut sessions_seen = false;
        let mut args = args;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--check" => cli.check_only = true,
                "--campaign" => cli.campaign = true,
                "--sessions" => {
                    let value = args
                        .next()
                        .ok_or("--sessions needs a number, e.g. --sessions 3")?;
                    cli.sessions = parse_sessions(&value)?;
                    sessions_seen = true;
                }
                other if other.starts_with("--sessions=") => {
                    cli.sessions = parse_sessions(&other["--sessions=".len()..])?;
                    sessions_seen = true;
                }
                other => {
                    return Err(format!(
                        "unknown argument `{other}` \
                         (only --check, --campaign and --sessions exist)"
                    )
                    .into())
                }
            }
        }

        if sessions_seen && !cli.campaign {
            return Err("--sessions only works with --campaign".into());
        }

        Ok(cli)
    }
}

/// `--sessions N`: a whole number, at least 1.
fn parse_sessions(value: &str) -> Result<usize, Box<dyn Error>> {
    match value.parse::<usize>() {
        Ok(count) if count >= 1 => Ok(count),
        Ok(_) => Err("--sessions must be at least 1".into()),
        Err(_) => Err(format!("--sessions needs a whole number, got `{value}`").into()),
    }
}

/// `EVA_SCENARIO`: unset means `NORMAL`; anything else must be a known scenario.
fn scenario_from(value: Option<&str>) -> Result<Scenario, Box<dyn Error>> {
    match value {
        None => Ok(Scenario::Normal),
        Some(value) => Ok(value.parse()?),
    }
}

/// Completes on Ctrl+C or, on Unix, on SIGTERM (`docker stop`, `systemctl stop`).
///
/// A signal handler that cannot be installed never triggers a stop by itself.
async fn shutdown_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(_) => std::future::pending::<()>().await,
            }
        };

        tokio::select! {
            () = ctrl_c => {}
            () = terminate => {}
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> impl Iterator<Item = String> {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn without_arguments_the_monitoring_runs() {
        let cli = Cli::from_args(args(&[])).unwrap();
        assert!(!cli.check_only);
        assert!(!cli.campaign);
    }

    #[test]
    fn check_asks_for_the_endpoint_check_only() {
        assert_eq!(
            Cli::from_args(args(&["--check"])).unwrap(),
            Cli {
                check_only: true,
                campaign: false,
                sessions: 1,
            }
        );
    }

    #[test]
    fn campaign_runs_every_scenario() {
        assert_eq!(
            Cli::from_args(args(&["--campaign"])).unwrap(),
            Cli {
                check_only: false,
                campaign: true,
                sessions: 1,
            }
        );
    }

    #[test]
    fn a_campaign_defaults_to_one_session() {
        assert_eq!(Cli::from_args(args(&["--campaign"])).unwrap().sessions, 1);
    }

    #[test]
    fn sessions_sets_the_count() {
        assert_eq!(
            Cli::from_args(args(&["--campaign", "--sessions", "3"])).unwrap(),
            Cli {
                check_only: false,
                campaign: true,
                sessions: 3,
            }
        );
        assert_eq!(
            Cli::from_args(args(&["--campaign", "--sessions=5"]))
                .unwrap()
                .sessions,
            5
        );
    }

    #[test]
    fn sessions_requires_the_campaign() {
        assert!(Cli::from_args(args(&["--sessions", "3"])).is_err());
    }

    #[test]
    fn sessions_must_be_a_positive_whole_number() {
        assert!(Cli::from_args(args(&["--campaign", "--sessions", "0"])).is_err());
        assert!(Cli::from_args(args(&["--campaign", "--sessions", "two"])).is_err());
        assert!(Cli::from_args(args(&["--campaign", "--sessions"])).is_err());
    }

    #[test]
    fn an_unknown_argument_is_an_error_not_ignored() {
        let error = Cli::from_args(args(&["--chek"])).unwrap_err().to_string();

        assert!(error.contains("--chek"), "{error}");
        assert!(Cli::from_args(args(&["--check", "extra"])).is_err());
    }

    #[test]
    fn no_scenario_means_normal() {
        assert_eq!(scenario_from(None).unwrap(), Scenario::Normal);
    }

    #[test]
    fn scenarios_are_parsed_case_insensitively() {
        assert_eq!(scenario_from(Some("ECLIPSE")).unwrap(), Scenario::Eclipse);
        assert_eq!(scenario_from(Some("eclipse")).unwrap(), Scenario::Eclipse);
        assert_eq!(
            scenario_from(Some("rpc-flooding")).unwrap(),
            Scenario::RpcFlooding
        );
    }

    #[test]
    fn an_unknown_or_empty_scenario_is_an_error_not_a_silent_normal() {
        assert!(scenario_from(Some("attack")).is_err());
        assert!(scenario_from(Some("")).is_err());
    }
}