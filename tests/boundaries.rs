//! Boundary tests for the limit comparison and the star-height counter.
//!
//! These pin behavior the canonical oracle exercises only implicitly.

use redos_check::{safe_regex, Options, DEFAULT_LIMIT};

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
        safe_regex(&over, Options::new(25))
    );
    assert!(!safe_regex(&over, Options::default()));

    // 25 repetitions: safe either way.
    let at = "a?".repeat(25);
    assert_eq!(
        safe_regex(&at, Options::default()),
        safe_regex(&at, Options::new(25))
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
        assert_eq!(
            safe_regex(&pat, Options::new(limit)),
            expect,
            "limit {limit}"
        );
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
    assert!(safe_regex("abc", Options::new(0)));
    // One repetition trips a zero limit.
    assert!(!safe_regex("a*", Options::new(0)));
}

#[test]
fn bounded_quantifiers_count_like_unbounded() {
    // {0} still counts as one repetition. Nested inside another, star height 2.
    assert!(!safe_regex("(a{0}){2}", Options::default()));
    // A single {0} is height 1, safe.
    assert!(safe_regex("a{0}", Options::default()));
}

#[test]
fn brace_range_lower_above_upper_is_unsafe() {
    // `{n,m}` with n > m is a syntax error in ECMAScript, so it reads unsafe.
    assert!(!safe_regex("x{2,1}", Options::default()));
    assert!(!safe_regex("a{5,3}", Options::default()));
    // Equal and ascending bounds stay valid and safe.
    assert!(safe_regex("x{5,10}", Options::default()));
    assert!(safe_regex("a{3,5}", Options::default()));
    assert!(safe_regex("a{3,3}", Options::default()));
}

#[test]
fn leading_brace_quantifier_is_unsafe() {
    // A well-formed brace quantifier with no atom to bind is a syntax error.
    assert!(!safe_regex("{2}", Options::default()));
    assert!(!safe_regex("{2,}", Options::default()));
    assert!(!safe_regex("{2,3}", Options::default()));
    // A malformed brace stays a literal and is safe.
    assert!(safe_regex("{", Options::default()));
    assert!(safe_regex("a{", Options::default()));
    assert!(safe_regex("a{}b", Options::default()));
}

#[test]
fn quantifier_on_zero_width_assertion_is_unsafe() {
    // Anchors and word boundaries cannot take a quantifier.
    assert!(!safe_regex("^*", Options::default()));
    assert!(!safe_regex("$+", Options::default()));
    assert!(!safe_regex(r"\b*", Options::default()));
    assert!(!safe_regex(r"\B?", Options::default()));
    assert!(!safe_regex(r"\b{2}", Options::default()));
    // A lookbehind is zero-width too.
    assert!(!safe_regex("(?<=a)*", Options::default()));
    // A lookahead is quantifiable without the unicode flag.
    assert!(safe_regex("(?=a)*", Options::default()));
}

#[test]
fn stacked_quantifier_is_unsafe() {
    // A second quantifier on the same atom is a syntax error.
    assert!(!safe_regex("a*{2}", Options::default()));
    assert!(!safe_regex("x{5}{2}", Options::default()));
    assert!(!safe_regex("a{2}{3}", Options::default()));
    assert!(!safe_regex("a+?*", Options::default()));
    // A lazy marker on a single quantifier is not a stack.
    assert!(safe_regex("a*?", Options::default()));
    assert!(safe_regex("a{2,3}?", Options::default()));
}

#[test]
fn empty_character_class_is_safe() {
    // `[]` matches nothing and `[^]` matches anything. Both are valid and safe.
    assert!(safe_regex("[]", Options::default()));
    assert!(safe_regex("[^]", Options::default()));
    assert!(safe_regex("a[]b", Options::default()));
    // A `]` after a member is still a literal close.
    assert!(safe_regex("[a]", Options::default()));
    // `[]]` is an empty class then a literal `]`.
    assert!(safe_regex("[]]", Options::default()));
}
