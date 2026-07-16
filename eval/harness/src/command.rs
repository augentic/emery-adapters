//! One `specify` verb through the shared typed command router.

use std::io;

use anyhow::{Result, ensure};
use omnia_guest::Model;
use omnia_guest::api::Provider;
use omnia_guest::api::invoke::Invoker;
use project::adapter::{Hydrator, Resolver};
use project::handler::Anchor;
use project::seam::{Source, Target};

/// Run one verb through the shared typed command router against
/// `provider`, streaming its output and failing on a non-zero exit.
///
/// # Errors
///
/// Returns a router-assembly failure and any non-zero verb exit.
pub async fn invoke<P>(provider: &P, argv: &[&str]) -> Result<()>
where
    P: Provider + Anchor + Model + Resolver + Hydrator + Source + Target + Clone,
{
    eprintln!("==> specify {}", argv.join(" "));
    let router = transport::command::router(Invoker::new("specify", provider.clone()))
        .map_err(|error| anyhow::anyhow!("building the command router: {error}"))?;
    let mut full: Vec<String> = vec!["specify".to_string()];
    full.extend(argv.iter().map(ToString::to_string));
    let response = router.execute(full).await;
    drop(response.write_to(&mut io::stdout().lock(), &mut io::stderr().lock()));
    ensure!(response.exit == 0, "`specify {}` exited {}", argv.join(" "), response.exit);
    Ok(())
}
