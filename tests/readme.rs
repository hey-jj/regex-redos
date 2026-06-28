//! Parity with the documented examples.
//!
//! The four behaviors shown in the crate documentation must hold exactly.

use safe_regex_rs::{is_safe, safe_regex, Options};

#[test]
fn documented_examples() {
    assert!(!safe_regex("(x+x+)+y", Options::default()));
    assert!(safe_regex("(beep|boop)*", Options::default()));
    assert!(!safe_regex("(a+){10}", Options::default()));
    assert!(safe_regex(
        r"\blocation\s*:[^:\n]+\b(Oakland|San Francisco)\b",
        Options::default()
    ));
}

#[test]
fn is_safe_matches_default_options() {
    for pat in ["(x+x+)+y", "(beep|boop)*", "(a+){10}", "a*", "(a*)*"] {
        assert_eq!(is_safe(pat), safe_regex(pat, Options::default()), "{pat:?}");
    }
}
