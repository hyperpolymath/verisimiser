// SPDX-License-Identifier: MPL-2.0
// Copyright (c) Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
//
// Identifier validation for any user-controlled name flowing into
// generated DDL. The codegen layer interpolates table and column names
// directly into SQL via `format!()`; an unchecked name like
// `posts'); DROP TABLE x;--` would be injected verbatim. Closes #39.

use anyhow::{Result, bail};

/// Allowed: leading letter or underscore, then letters/digits/underscores.
/// Same shape as standard SQL identifiers without quoting. Length 1..=63
/// (Postgres' NAMEDATALEN minus the null terminator).
const MAX_IDENT_LEN: usize = 63;

/// Validate a single identifier (table or column name) for safe
/// interpolation into generated DDL.
///
/// Returns `Ok(ident)` if `ident` matches `^[A-Za-z_][A-Za-z0-9_]*$` and
/// is within the length bound; otherwise an `anyhow::Error` whose message
/// includes the offending identifier (truncated) and the kind label so
/// users can pinpoint the offending schema input.
pub fn validate_identifier<'a>(ident: &'a str, kind: &str) -> Result<&'a str> {
    if ident.is_empty() {
        bail!("invalid {kind}: empty identifier");
    }
    if ident.len() > MAX_IDENT_LEN {
        bail!(
            "invalid {kind} '{}': identifier too long ({} > {} chars)",
            display_truncated(ident),
            ident.len(),
            MAX_IDENT_LEN,
        );
    }
    let mut chars = ident.chars();
    let first = chars.next().expect("non-empty checked above");
    if !(first.is_ascii_alphabetic() || first == '_') {
        bail!(
            "invalid {kind} '{}': must start with a letter or underscore",
            display_truncated(ident),
        );
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            bail!(
                "invalid {kind} '{}': contains disallowed character {:?} \
                 (only [A-Za-z0-9_] permitted)",
                display_truncated(ident),
                c,
            );
        }
    }
    Ok(ident)
}

fn display_truncated(s: &str) -> String {
    const LIMIT: usize = 60;
    if s.len() <= LIMIT {
        s.to_string()
    } else {
        format!("{}…", &s[..LIMIT])
    }
}

#[cfg(test)]
mod tests {
    use super::validate_identifier;

    #[test]
    fn valid_identifiers_pass() {
        for ok in [
            "posts",
            "users",
            "Order",
            "_internal",
            "user_2024",
            "T",
            "a_b_c_d",
        ] {
            validate_identifier(ok, "table").unwrap_or_else(|e| panic!("{ok} should pass: {e}"));
        }
    }

    /// Attack strings that the validator must reject. At least 10 per the
    /// V-L2-G1 acceptance criteria.
    #[test]
    fn injection_strings_rejected() {
        let attacks = [
            "posts'); DROP TABLE x;--",
            "posts; DROP TABLE x;",
            "posts--",
            "1posts",       // leading digit
            "",             // empty
            "posts table",  // space
            "posts;",       // semicolon
            "posts'",       // single quote
            "posts\"x\"",   // double quote
            "posts/*x*/",   // comment
            "posts\nx",     // newline
            "posts\tx",     // tab
            "posts UNION SELECT 1",
            "ünicode",      // non-ASCII
            "posts.col",    // dot
            "posts(",       // paren
        ];
        for attack in attacks {
            let result = validate_identifier(attack, "table");
            assert!(
                result.is_err(),
                "injection string {attack:?} must be rejected"
            );
        }
    }

    /// Long identifiers (over 63 chars) must be rejected.
    #[test]
    fn overlong_identifiers_rejected() {
        let long = "a".repeat(64);
        let err = validate_identifier(&long, "column").unwrap_err();
        assert!(
            err.to_string().contains("too long"),
            "expected length error, got: {err}"
        );
    }

    /// The error message must include the offending identifier so users
    /// can find it in their schema.
    #[test]
    fn error_names_offending_identifier() {
        let err = validate_identifier("bad name", "table").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("bad name"),
            "error must name the offending identifier; got: {msg}"
        );
        assert!(
            msg.contains("table"),
            "error must name the kind; got: {msg}"
        );
    }
}
