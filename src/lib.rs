//! Static heuristic detector for catastrophic-backtracking (ReDoS) regular
//! expressions.
//!
//! [`safe_regex`] parses a JavaScript-syntax regular expression and applies two
//! syntactic heuristics. It never runs the pattern. The answer is a single
//! boolean: `true` means the pattern looks safe, `false` means it looks
//! vulnerable or could not be parsed.
//!
//! # Heuristics
//!
//! 1. Star height greater than 1. A repetition nested inside another repetition
//!    (for example `(a+)*`) flags the pattern as vulnerable.
//! 2. Repetition count over a limit. When the total number of repetition nodes
//!    exceeds `limit` (default 25), the pattern flags as vulnerable.
//!
//! If either heuristic fires the pattern is unsafe. Unparseable input is also
//! unsafe. Everything else is safe.
//!
//! A repetition is any atom followed by `*`, `+`, `?`, `{n}`, `{n,}`, or
//! `{n,m}`, greedy or lazy. Bound size does not matter. `{0}` counts the same as
//! `*`.
//!
//! The check has false positives and false negatives by design. It trades
//! precision for a fast purely syntactic test. Patterns like `(ab*)+` are linear
//! in practice yet flagged unsafe (star height 2). Patterns like `(a|a)*` are
//! exponential yet reported safe (star height 1).
//!
//! # Examples
//!
//! ```
//! use redos_check::{safe_regex, Options};
//!
//! assert!(safe_regex("(beep|boop)*", Options::default()));
//! assert!(!safe_regex("(x+x+)+y", Options::default()));
//! assert!(!safe_regex("(a+){10}", Options::default()));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod parser;

use parser::{parse, Node};

/// Default repetition limit when [`Options::limit`] is left unset.
pub const DEFAULT_LIMIT: usize = 25;

/// Tuning for the safety check.
///
/// Only [`limit`](Options::limit) is read. It bounds the total number of
/// repetition nodes allowed in a pattern. The default is [`DEFAULT_LIMIT`].
///
/// Build an `Options` with [`Options::new`] or [`Options::default`]. The struct
/// is `#[non_exhaustive]`, so a new field can land without breaking callers.
///
/// # Examples
///
/// ```
/// use redos_check::{safe_regex, Options};
///
/// // 26 repetitions trip the default limit of 25.
/// let pattern = "a?".repeat(26);
/// assert!(!safe_regex(&pattern, Options::default()));
///
/// // Raising the limit makes it safe again.
/// assert!(safe_regex(&pattern, Options::new(30)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Options {
    /// Maximum allowed total repetitions. A pattern with strictly more than this
    /// many repetition nodes is unsafe.
    pub limit: usize,
}

impl Options {
    /// Build options with an explicit repetition limit.
    ///
    /// # Examples
    ///
    /// ```
    /// use redos_check::{safe_regex, Options};
    ///
    /// assert!(safe_regex("a?".repeat(30).as_str(), Options::new(30)));
    /// ```
    pub fn new(limit: usize) -> Self {
        Options { limit }
    }
}

impl Default for Options {
    fn default() -> Self {
        Options {
            limit: DEFAULT_LIMIT,
        }
    }
}

/// Report whether a regular expression looks safe from catastrophic
/// backtracking.
///
/// `re` is a JavaScript-syntax pattern. Pass [`Options::default`] for the
/// standard repetition limit of 25.
///
/// Returns `true` when the pattern is safe under both heuristics. Returns
/// `false` when star height exceeds 1, when the repetition count exceeds
/// `opts.limit`, or when the pattern cannot be parsed. The function never
/// panics on input.
///
/// # Examples
///
/// ```
/// use redos_check::{safe_regex, Options};
///
/// assert!(safe_regex("^a+a+$", Options::default()));
/// assert!(!safe_regex("(a*)*$", Options::default()));
/// assert!(!safe_regex("(a+", Options::default())); // unparseable
/// ```
pub fn safe_regex(re: &str, opts: Options) -> bool {
    let ast = match parse(re) {
        Ok(ast) => ast,
        Err(_) => return false,
    };

    let mut measure = Measure::default();
    measure.walk(&ast, 0);

    if measure.max_depth > 1 {
        return false;
    }
    if measure.total > opts.limit {
        return false;
    }
    true
}

/// Convenience wrapper that uses the default options.
///
/// Equal to `safe_regex(re, Options::default())`.
///
/// # Examples
///
/// ```
/// use redos_check::is_safe;
///
/// assert!(is_safe("(beep|boop)*"));
/// assert!(!is_safe("(x+x+)+y"));
/// ```
pub fn is_safe(re: &str) -> bool {
    safe_regex(re, Options::default())
}

/// Accumulator for the single depth-first walk over the AST.
#[derive(Default)]
struct Measure {
    /// Deepest observed nesting of repetition nodes (the star height).
    max_depth: usize,
    /// Total count of repetition nodes anywhere in the tree.
    total: usize,
}

impl Measure {
    /// Visit `node` carrying the current repetition nesting `depth`.
    ///
    /// Entering a repetition raises depth and the total. The body of the
    /// repetition is then visited at the higher depth, so a repetition inside
    /// another repetition lifts the observed star height.
    fn walk(&mut self, node: &Node, depth: usize) {
        match node {
            Node::Empty => {}
            Node::Char => {}
            Node::Concat(parts) | Node::Alt(parts) => {
                for part in parts {
                    self.walk(part, depth);
                }
            }
            Node::Group(inner) => {
                self.walk(inner, depth);
            }
            Node::Repetition(inner) => {
                self.total += 1;
                let next = depth + 1;
                if next > self.max_depth {
                    self.max_depth = next;
                }
                self.walk(inner, next);
            }
        }
    }
}
