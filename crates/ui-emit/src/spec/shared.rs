//! Helpers the composition checks share.
//!
//! The schema validators stay with ui-ir, which owns the schemas (`ui-ir check`).

/// Escapes a JSON Pointer reference token (`~` becomes `~0`, `/` becomes `~1`).
pub fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_tokens_are_escaped() {
        assert_eq!(escape_pointer_token("a/b"), "a~1b");
        assert_eq!(escape_pointer_token("a~b"), "a~0b");
    }
}
