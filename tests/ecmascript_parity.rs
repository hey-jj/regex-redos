//! ECMAScript parsing parity for inputs the conformance suite does not reach.
//!
//! For a string pattern the public path mirrors `new RegExp(pattern)`. A pattern
//! the engine rejects must report unsafe (`false`). A pattern the engine accepts
//! must analyze normally. The conformance suite covers valid linear and
//! exponential patterns. It does not cover invalid quantifier placement or empty
//! character classes. This file pins those cases against the engine.
//!
//! The expected column is the result of `new RegExp(pattern)` followed by the
//! two heuristics, computed with a JavaScript engine. It is the ground truth.
//!
//! Two groups split by current status.
//!
//! - [`literal_and_lookahead_edges`] passes today. Malformed braces stay
//!   literal, and a quantified lookahead is valid without the `u` flag. These
//!   guard against regressions in the lenient direction.
//! - [`invalid_syntax_must_be_unsafe`] and [`empty_class_is_safe`] fail today.
//!   They are marked `#[ignore]` and document four known parser gaps. Remove the
//!   ignore once the parser rejects these forms (or accepts the empty class).
//!   The four gaps:
//!   - quantifiers on anchors and stacked quantifiers
//!   - empty class `[]` and `[^]` wrongly rejected
//!   - brace range `{n,m}` with `n > m` wrongly accepted
//!   - a leading well-formed quantifier `{2}` read as literal

use safe_regex_rs::{safe_regex, Options};

/// One row: a pattern and the engine-derived verdict.
struct Row {
    /// The regex pattern text, exactly as passed to `new RegExp`.
    pat: &'static str,
    /// Expected result. `true` is safe, `false` is unsafe or unparseable.
    expect: bool,
}

/// Assert every row under the default limit.
fn check(rows: &[Row]) {
    for r in rows {
        assert_eq!(
            safe_regex(r.pat, Options::default()),
            r.expect,
            "pattern {:?}",
            r.pat
        );
    }
}

#[test]
fn literal_and_lookahead_edges() {
    // A brace that is not a valid quantifier stays literal, so the pattern is
    // valid and safe. A lookahead takes a quantifier without the `u` flag, so
    // `(?=a)*` builds and reads as safe. These already hold.
    let rows = [
        Row {
            pat: "a{",
            expect: true,
        },
        Row {
            pat: "{",
            expect: true,
        },
        Row {
            pat: "a{}",
            expect: true,
        },
        Row {
            pat: "{2",
            expect: true,
        },
        Row {
            pat: "a{}b",
            expect: true,
        },
        // `[]]` is an empty class followed by a literal `]`. `[^]]` is a negated
        // empty class followed by a literal `]`. Both are valid and safe.
        Row {
            pat: "[]]",
            expect: true,
        },
        Row {
            pat: "[^]]",
            expect: true,
        },
        // Quantified lookahead is valid in non-unicode mode.
        Row {
            pat: "(?=a)*",
            expect: true,
        },
        Row {
            pat: "(?=a){2}",
            expect: true,
        },
    ];
    check(&rows);
}

#[test]
#[ignore = "known parser gaps: invalid ECMAScript reported safe (see build tracking)"]
fn invalid_syntax_must_be_unsafe() {
    // Every pattern here throws in `new RegExp`, so the engine path returns
    // false. The current parser accepts them and returns true.
    let rows = [
        // Brace range with lower bound above upper bound. SyntaxError.
        Row {
            pat: "x{2,1}",
            expect: false,
        },
        Row {
            pat: "a{5,3}",
            expect: false,
        },
        Row {
            pat: "a{10,2}",
            expect: false,
        },
        Row {
            pat: "a{2,1}?",
            expect: false,
        },
        // Stacked quantifiers. SyntaxError.
        Row {
            pat: "x{5}{2}",
            expect: false,
        },
        Row {
            pat: "a{2}{3}",
            expect: false,
        },
        Row {
            pat: "a*{2}",
            expect: false,
        },
        Row {
            pat: "a+{2}",
            expect: false,
        },
        Row {
            pat: "a?{2}",
            expect: false,
        },
        Row {
            pat: r"\b{2}",
            expect: false,
        },
        // Quantifier on an anchor or word boundary. SyntaxError.
        Row {
            pat: "^*",
            expect: false,
        },
        Row {
            pat: "$*",
            expect: false,
        },
        Row {
            pat: "^+",
            expect: false,
        },
        Row {
            pat: "$+",
            expect: false,
        },
        Row {
            pat: r"\b*",
            expect: false,
        },
        Row {
            pat: r"\B*",
            expect: false,
        },
        Row {
            pat: r"\b?",
            expect: false,
        },
        // Quantifier on a lookbehind. SyntaxError, unlike lookahead.
        Row {
            pat: "(?<=a)*",
            expect: false,
        },
        Row {
            pat: "(?<!a)*",
            expect: false,
        },
        // Well-formed brace quantifier with no atom to bind. SyntaxError.
        Row {
            pat: "{2}",
            expect: false,
        },
        Row {
            pat: "{2,}",
            expect: false,
        },
        Row {
            pat: "{2,3}",
            expect: false,
        },
        Row {
            pat: "{3,5}",
            expect: false,
        },
    ];
    check(&rows);
}

#[test]
#[ignore = "known parser gap: empty class [] and [^] wrongly rejected (see build tracking)"]
fn empty_class_is_safe() {
    // `[]` matches nothing and `[^]` matches anything. Both are valid
    // ECMAScript and safe. The current parser rejects them.
    let rows = [
        Row {
            pat: "[]",
            expect: true,
        },
        Row {
            pat: "[^]",
            expect: true,
        },
        Row {
            pat: "a[]b",
            expect: true,
        },
        Row {
            pat: "[]a",
            expect: true,
        },
        Row {
            pat: "[^]x",
            expect: true,
        },
        Row {
            pat: "([])*",
            expect: true,
        },
        Row {
            pat: "([^])+",
            expect: true,
        },
        Row {
            pat: "[^]{2,}",
            expect: true,
        },
    ];
    check(&rows);
}
