//! `specify-dev` — native dev shim and eval harness.
//! Three modes: CLI (`command`), HTTP (`serve`), and live-model eval (`trial` / `scenario`).

mod command;
mod grade;
mod http;
mod telemetry;
mod trial;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("serve") => match http::serve(&argv[1..]).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("specify-dev: {err:#}");
                ExitCode::FAILURE
            }
        },
        Some("eval") => match trial::run(&argv[1..]).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("specify-dev: {err:#}");
                ExitCode::FAILURE
            }
        },
        _ => ExitCode::from(command::run(argv).await),
    }
}
