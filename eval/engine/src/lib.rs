//! Adapters linked into the shared native harness.

harness::adapters! {
    pub Adapters {
        source captures::Captures,
        target contracts::Contracts,
        source documentation::Documentation,
        source intent::Intent,
        target omnia_target::Omnia,
        source screenshots::Screenshots,
        source typescript::Typescript,
        target vectis::Vectis,
    }
}
