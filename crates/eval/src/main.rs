//! Native CLI and live eval over the linked adapters.

fn main() -> std::process::ExitCode {
    harness::entry::main::<Adapters>(Some(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scenarios"
    ))))
}

harness::adapters! {
    Adapters {
        source captures::Adapter,
        target contracts::Adapter,
        source documentation::Adapter,
        source intent::Adapter,
        target omnia_target::Adapter,
        source screenshots::Adapter,
        source typescript::Adapter,
        target vectis::Adapter,
    }
}
