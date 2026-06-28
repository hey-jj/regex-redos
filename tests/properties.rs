//! Property tests built on deterministic generators.
//!
//! These check invariants over generated inputs without an external property
//! testing crate. A small linear congruential generator drives the random
//! cases so failures reproduce.

use safe_regex_rs::{safe_regex, Options};

/// A tiny deterministic pseudo-random source. Same seed, same stream.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        // Numerical Recipes constants.
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let i = (self.next() >> 33) as usize % items.len();
        &items[i]
    }
}

/// Build a random pattern-like string from regex metacharacters and letters.
fn random_pattern(rng: &mut Lcg, len: usize) -> String {
    let alphabet = [
        "a", "b", "(", ")", "[", "]", "*", "+", "?", "|", "{", "}", "2", ",", "\\", ".", "^", "$",
        "<", ">", "=", "!", ":",
    ];
    let mut s = String::new();
    for _ in 0..len {
        s.push_str(rng.pick(&alphabet));
    }
    s
}

#[test]
fn never_panics_on_arbitrary_input() {
    let mut rng = Lcg(0x1234_5678);
    for _ in 0..5000 {
        let len = (rng.next() >> 40) as usize % 24;
        let pat = random_pattern(&mut rng, len);
        // The only requirement is that the call returns without panicking.
        let _ = safe_regex(&pat, Options::default());
    }
}

#[test]
fn deterministic() {
    let mut rng = Lcg(0xfeed_face);
    for _ in 0..2000 {
        let len = (rng.next() >> 40) as usize % 20;
        let pat = random_pattern(&mut rng, len);
        let a = safe_regex(&pat, Options::default());
        let b = safe_regex(&pat, Options::default());
        assert_eq!(a, b, "non-deterministic for {pat:?}");
    }
}

#[test]
fn monotonic_in_limit() {
    // Raising the limit never turns a safe pattern unsafe. Heuristic #2 only
    // relaxes. Heuristic #1 does not depend on the limit.
    let patterns = [
        "a?".repeat(5),
        "a?".repeat(10),
        "a?".repeat(26),
        "(a+)*".to_string(),
        "a*b*c*".to_string(),
    ];
    for pat in &patterns {
        for limit in 0..40 {
            if safe_regex(pat, Options { limit }) {
                for higher in limit..40 {
                    assert!(
                        safe_regex(pat, Options { limit: higher }),
                        "pattern {pat:?} safe at {limit} but unsafe at {higher}"
                    );
                }
                break;
            }
        }
    }
}

#[test]
fn nested_star_depth_threshold() {
    // Build (((...a*...)*)*) to depth d. The result is unsafe exactly when d
    // reaches 2, since each wrap adds one to the star height.
    for d in 1..=6 {
        let mut pat = String::from("a");
        for _ in 0..d {
            pat = format!("({pat})*");
        }
        let expect_safe = d < 2;
        assert_eq!(
            safe_regex(&pat, Options::default()),
            expect_safe,
            "depth {d} pattern {pat:?}"
        );
    }
}
