//! A Wasm-free Specify CLI over statically linked first-party adapters.

#![cfg(not(target_arch = "wasm32"))]

mod model;

use std::path::PathBuf;
use std::process::ExitCode;

use native::{Catalog, DynModel, ExecutionPaths};

use crate::model::DevModel;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<ExitCode> {
    let root = PathBuf::from(".").canonicalize()?;
    let catalog = Catalog::builder()
        .source::<documentation::Adapter>()
        .source::<intent::Adapter>()
        .target::<contracts::Adapter>()
        .build()?;
    let paths = ExecutionPaths::operator(root.clone());
    let model = DynModel::new(DevModel::new(&root));

    Ok(native::command::run(paths, model, catalog, std::env::args().collect()).await)
}
