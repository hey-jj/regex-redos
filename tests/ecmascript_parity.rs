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
//! Three groups, one per parsing rule.
//!
//! - [`literal_and_lookahead_edges`] guards the lenient direction. Malformed
//!   braces stay literal, and a quantified lookahead is valid without the `u`
//!   flag.
//! - [`invalid_syntax_must_be_unsafe`] pins the four families the engine
//!   rejects: quantifiers on anchors or word boundaries, stacked quantifiers,
//!   a brace range `{n,m}` with `n > m`, and a leading well-formed quantifier
//!   `{2}` with no atom to bind.
//! - [`empty_class_is_safe`] pins the empty class `[]` and the negated empty
//!   class `[^]`, both valid and safe.
//! - [`invalid_named_capture_names_are_unsafe`] checks named capture syntax and
//!   duplicate name rejection.
//! - [`valid_named_capture_names_stay_safe`] checks accepted identifier names.

use regex_redos::{safe_regex, Options};

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

#[test]
fn reversed_class_ranges_are_unsafe() {
    let rows = [
        Row {
            pat: "[z-a]",
            expect: false,
        },
        Row {
            pat: r"[b-\x61]",
            expect: false,
        },
        Row {
            pat: r"[\x7a-a]",
            expect: false,
        },
        Row {
            pat: r"[b-\u0061]",
            expect: false,
        },
        Row {
            pat: r"[\u007a-a]",
            expect: false,
        },
        Row {
            pat: r"[z-\x]",
            expect: false,
        },
        Row {
            pat: r"[z-\u{61}]",
            expect: false,
        },
        Row {
            pat: r"[\u{7a}-a]",
            expect: false,
        },
        Row {
            pat: r"[z-\u{z}]",
            expect: false,
        },
        Row {
            pat: r"[\c1-\x10]",
            expect: false,
        },
        Row {
            pat: r"[\c-a]",
            expect: false,
        },
        Row {
            pat: r"[b-\c]",
            expect: false,
        },
        Row {
            pat: r"[z-\8]",
            expect: false,
        },
        Row {
            pat: r"[\x22-\41]",
            expect: false,
        },
        Row {
            pat: "[\u{1d49c}-\u{1d4b5}]",
            expect: false,
        },
        Row {
            pat: r"[a-\400]",
            expect: false,
        },
        Row {
            pat: r"[\377-\400]",
            expect: false,
        },
    ];
    check(&rows);
}

#[test]
fn valid_class_range_edges_stay_safe() {
    let rows = [
        Row {
            pat: r"[\w-a]",
            expect: true,
        },
        Row {
            pat: r"[a-z\d-_]",
            expect: true,
        },
        Row {
            pat: r"[\n-a]",
            expect: true,
        },
        Row {
            pat: r"[\t-a]",
            expect: true,
        },
        Row {
            pat: r"[\u000a-a]",
            expect: true,
        },
        Row {
            pat: r"[\x]",
            expect: true,
        },
        Row {
            pat: r"[\x0]",
            expect: true,
        },
        Row {
            pat: r"[\x1z]",
            expect: true,
        },
        Row {
            pat: r"[\uz]",
            expect: true,
        },
        Row {
            pat: r"[a-\x]",
            expect: true,
        },
        Row {
            pat: r"[a-\u{7a}]",
            expect: true,
        },
        Row {
            pat: r"[\c1-\x20]",
            expect: true,
        },
        Row {
            pat: r"[\00-\07]",
            expect: true,
        },
        Row {
            pat: r"[\8-a]",
            expect: true,
        },
        Row {
            pat: r"[\11-\x20]",
            expect: true,
        },
        Row {
            pat: r"[\1-\x05]",
            expect: true,
        },
        Row {
            pat: r"[\uD800]",
            expect: true,
        },
        Row {
            pat: r"[\uDC00]",
            expect: true,
        },
        Row {
            pat: r"[a-\uD800]",
            expect: true,
        },
        Row {
            pat: r"[\uD800-\uDBFF]",
            expect: true,
        },
        Row {
            pat: r"[\uDC00-\uDFFF]",
            expect: true,
        },
        Row {
            pat: r"[\uD800\uDC00-\uDC01]",
            expect: true,
        },
        Row {
            pat: "[\u{1f600}-\\uDE01]",
            expect: true,
        },
        Row {
            pat: r"[\400-a]",
            expect: true,
        },
        Row {
            pat: r"[\777-\778]",
            expect: true,
        },
    ];
    check(&rows);
}

#[test]
fn invalid_named_capture_names_are_unsafe() {
    let rows = [
        Row {
            pat: r"(?<1>a)",
            expect: false,
        },
        Row {
            pat: r"(?<a-b>a)",
            expect: false,
        },
        Row {
            pat: r"(?<a>a)(?<a>b)",
            expect: false,
        },
        Row {
            pat: r"(?<a>a)(?<\u0061>b)",
            expect: false,
        },
        Row {
            pat: r"(?<a>x)|(?<a>y)",
            expect: false,
        },
        Row {
            pat: r"(?<\u0345>a)",
            expect: false,
        },
        Row {
            pat: r"(?<\u200c>a)",
            expect: false,
        },
        Row {
            pat: r"(?<\uD800>a)",
            expect: false,
        },
        Row {
            pat: r"(?<\uDC00>a)",
            expect: false,
        },
        Row {
            pat: r"(?<\uD801>a)",
            expect: false,
        },
    ];
    check(&rows);
}

#[test]
fn valid_named_capture_names_stay_safe() {
    let rows = [
        Row {
            pat: r"(?<a\u203f>a)",
            expect: true,
        },
        Row {
            pat: r"(?<_a>a)",
            expect: true,
        },
        Row {
            pat: r"(?<$a>a)",
            expect: true,
        },
        Row {
            pat: r"(?<\u{61}>a)",
            expect: true,
        },
        Row {
            pat: "(?<\u{10400}>a)",
            expect: true,
        },
        Row {
            pat: r"(?<\u{10400}>a)",
            expect: true,
        },
        Row {
            pat: r"(?<a\u200c>a)",
            expect: true,
        },
        Row {
            pat: r"(?<a\u200d>a)",
            expect: true,
        },
        Row {
            pat: r"(?<\uD801\uDC00>a)",
            expect: true,
        },
    ];
    check(&rows);
}
