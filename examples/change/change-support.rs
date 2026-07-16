//! Deterministic support commands for the wasm change example's Cargo
//! Make task (see [`README.md`](README.md)):
//!
//! - `port` — print an available loopback `host:port` for the run's
//!   HTTP trigger, so parallel runs never fight over one hard-coded
//!   port.
//! - `verify <workspace> <status-json>` — the completion gate after
//!   `plan execute`: the captured `plan status --format json` is
//!   `drained`, every `plan.yaml` entry is `done`, and the merged
//!   contracts baseline is non-empty and clean under the contracts
//!   adapter's own `validate_baseline` — the same deterministic
//!   grading posture as the native trial, not just "a yaml exists".

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::net::TcpListener;
        use std::path::Path;
        use std::{env, fs};

        use anyhow::{Context as _, Result, bail, ensure};
        use serde::Deserialize;

        fn main() -> Result<()> {
            let mut args = env::args().skip(1);
            match args.next().as_deref() {
                Some("port") => {
                    ensure!(args.next().is_none(), "port accepts no arguments");
                    port()
                }
                Some("verify") => {
                    let workspace = args.next().context("verify needs <workspace>")?;
                    let status = args.next().context("verify needs <status-json>")?;
                    ensure!(args.next().is_none(), "verify accepts exactly two arguments");
                    verify(Path::new(&workspace), Path::new(&status))
                }
                Some(command) => bail!("unknown command `{command}`; expected `port` or `verify`"),
                None => bail!("expected `port` or `verify`"),
            }
        }

        /// Print a currently free loopback address. The listener drops
        /// before the runtime binds, so a raced port is possible but
        /// vanishingly unlikely in the dev loop this serves.
        fn port() -> Result<()> {
            let listener =
                TcpListener::bind(("127.0.0.1", 0)).context("binding an ephemeral port")?;
            println!("{}", listener.local_addr().context("reading the bound address")?);
            Ok(())
        }

        /// The captured `plan status --format json` body — the fields the
        /// gate reads from the engine's kebab-case `StatusBody`.
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct Status {
            plan: String,
            lifecycle: String,
            action: String,
            counts: Counts,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        struct Counts {
            pending: usize,
            in_progress: usize,
            done: usize,
        }

        /// The `plan.yaml` fields the gate reads (per-entry `status`).
        #[derive(Debug, Deserialize)]
        struct PlanFile {
            slices: Vec<PlanEntry>,
        }

        #[derive(Debug, Deserialize)]
        struct PlanEntry {
            name: String,
            status: String,
        }

        fn verify(workspace: &Path, status_path: &Path) -> Result<()> {
            let status: Status = serde_json::from_str(
                &fs::read_to_string(status_path)
                    .with_context(|| format!("reading {}", status_path.display()))?,
            )
            .with_context(|| format!("parsing {}", status_path.display()))?;
            ensure!(
                status.action == "drained",
                "plan `{}` is not drained: next action `{}` ({} pending, {} in-progress, \
                 {} done, lifecycle {})",
                status.plan,
                status.action,
                status.counts.pending,
                status.counts.in_progress,
                status.counts.done,
                status.lifecycle
            );
            ensure!(status.counts.done > 0, "plan `{}` drained with zero done entries", status.plan);

            let plan_path = workspace.join("plan.yaml");
            let plan: PlanFile = serde_saphyr::from_str(
                &fs::read_to_string(&plan_path)
                    .with_context(|| format!("reading {}", plan_path.display()))?,
            )
            .with_context(|| format!("parsing {}", plan_path.display()))?;
            ensure!(!plan.slices.is_empty(), "{} has no entries", plan_path.display());
            for entry in &plan.slices {
                ensure!(
                    entry.status == "done",
                    "plan entry `{}` is `{}`, expected `done`",
                    entry.name,
                    entry.status
                );
            }

            let contracts_dir = workspace.join("contracts");
            ensure!(
                fs::read_dir(&contracts_dir).is_ok_and(|mut dir| dir.next().is_some()),
                "no merged contracts baseline under {}",
                contracts_dir.display()
            );
            let findings = contracts::validate::validate_baseline(&contracts_dir);
            if !findings.is_empty() {
                for finding in &findings {
                    eprintln!(
                        "finding [{}] {}: {}",
                        finding.rule_id,
                        finding.path.display(),
                        finding.detail
                    );
                }
                bail!("contracts baseline validation failed with {} finding(s)", findings.len());
            }

            println!(
                "change example verified: plan `{}` drained ({} entries done), contracts \
                 baseline clean at {}",
                status.plan,
                plan.slices.len(),
                contracts_dir.display()
            );
            Ok(())
        }
    } else {
        fn main() {}
    }
}
