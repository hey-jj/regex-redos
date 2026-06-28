# safe-regex-rs

Static heuristic detector for catastrophic-backtracking (ReDoS) regular
expressions. Give it a pattern, get back a boolean. It never runs the regex.

The check parses a JavaScript-syntax pattern and applies two syntactic rules:

1. Star height greater than 1. A repetition nested inside another repetition,
   like `(a+)*`, is flagged.
2. Repetition count over a limit. More than `limit` repetition nodes (default
   25) is flagged.

If either rule fires, the pattern is unsafe. Unparseable input is unsafe.
Everything else is safe.

## Usage

```rust
use safe_regex_rs::{safe_regex, is_safe, Options};

// Default limit of 25.
assert!(is_safe("(beep|boop)*"));
assert!(!is_safe("(x+x+)+y"));

// Custom limit.
let pattern = "a?".repeat(26);
assert!(!safe_regex(&pattern, Options::default()));
assert!(safe_regex(&pattern, Options { limit: 30 }));
```

## What counts as a repetition

Any atom followed by `*`, `+`, `?`, `{n}`, `{n,}`, or `{n,m}`, greedy or lazy.
Bound size does not matter. `{0}` counts the same as `*`. Two repetitions
nested through any number of groups give star height 2.

## Accuracy

The check is fast and purely syntactic, so it has false positives and false
negatives by design.

- `(ab*)+` runs in linear time but is flagged unsafe, because it has star
  height 2.
- `(a|a)*` is exponential but reported safe, because its star height is 1.

For higher accuracy use a dynamic analysis tool. This crate trades precision
for speed and a zero-execution guarantee.

## Installation

```toml
[dependencies]
safe-regex-rs = "0.1"
```

## License

Licensed under the [MIT license](LICENSE).
