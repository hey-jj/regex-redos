//! Boundary tests for the limit comparison and the star-height counter.
//!
//! These pin behavior the canonical oracle exercises only implicitly.

use safe_regex_rs::{safe_regex, Options, DEFAULT_LIMIT};

#[test]
fn default_limit_is_25() {
    assert_eq!(DEFAULT_LIMIT, 25);
    assert_eq!(Options::default().limit, 25);
}

#[test]
fn default_limit_matches_explicit_25() {
    // 26 repetitions: unsafe either way.
    let over = "a?".repeat(26);
    assert_eq!(
        safe_regex(&over, Options::default()),
        safe_regex(&over, Options { limit: 25 })
    );
    assert!(!safe_regex(&over, Options::default()));

    // 25 repetitions: safe either way.
    let at = "a?".repeat(25);
    assert_eq!(
        safe_regex(&at, Options::default()),
        safe_regex(&at, Options { limit: 25 })
    );
    assert!(safe_regex(&at, Options::default()));
}

#[test]
fn limit_sweep_is_strict() {
    // Ten repetitions. Safe when the limit is at least 10, unsafe below.
    let pat = "a?".repeat(10);
    for &(limit, expect) in &[
        (0usize, false),
        (1, false),
        (9, false),
        (10, true),
        (11, true),
        (25, true),
    ] {
        assert_eq!(safe_regex(&pat, Options { limit }), expect, "limit {limit}");
    }
}

#[test]
fn star_height_ladder() {
    // Strict greater-than-1 comparison. Height 0 and 1 are safe, 2 and up unsafe.
    assert!(safe_regex("a", Options::default())); // height 0
    assert!(safe_regex("a*", Options::default())); // height 1
    assert!(!safe_regex("(a*)*", Options::default())); // height 2
    assert!(!safe_regex("((a*)*)*", Options::default())); // height 3
}

#[test]
fn empty_pattern_is_safe() {
    assert!(safe_regex("", Options::default()));
}

#[test]
fn limit_zero_with_zero_reps_is_safe() {
    // Zero repetitions, limit zero. The strict comparison 0 > 0 is false.
    assert!(safe_regex("abc", Options { limit: 0 }));
    // One repetition trips a zero limit.
    assert!(!safe_regex("a*", Options { limit: 0 }));
}

#[test]
fn bounded_quantifiers_count_like_unbounded() {
    // {0} still counts as one repetition. Nested inside another, star height 2.
    assert!(!safe_regex("(a{0}){2}", Options::default()));
    // A single {0} is height 1, safe.
    assert!(safe_regex("a{0}", Options::default()));
}
