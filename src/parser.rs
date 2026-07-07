//! A small parser for JavaScript-syntax regular expressions.
//!
//! The parser builds only the structure the safety heuristics need. It tracks
//! repetition nesting and validates the pattern. It does not capture char class
//! contents, escape values, or group names beyond what validation requires.
//!
//! The grammar follows ECMAScript regular expressions. Backreferences, named
//! groups, named backreferences, lookahead, lookbehind, and control escapes all
//! parse. Constructs that ECMAScript rejects, such as an unbalanced paren or an
//! atomic group `(?>...)`, produce an [`Error`] so the caller can report the
//! pattern as unsafe.

use std::collections::HashSet;

/// A parse failure. The pattern is not a valid ECMAScript regular expression.
///
/// The type is internal. The public API maps a parse failure to a `false`
/// safety verdict, so the cause never reaches a caller and carries no payload.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Error;

/// Maximum group nesting accepted by the recursive parser.
///
/// Patterns above this depth return [`Error`] before another recursive descent.
/// Every accepted group tree stays within this limit, so later tree walks see
/// bounded depth.
const MAX_GROUP_NESTING: usize = 256;

/// The reduced syntax tree.
///
/// Only repetitions and grouping carry meaning for the heuristics. Everything
/// else collapses into [`Node::Char`] or a list.
#[derive(Debug)]
pub(crate) enum Node {
    /// An empty branch, for example the right side of `a|`.
    Empty,
    /// Any single consuming or zero-width atom that is not a group or
    /// repetition. Literals, escapes, char classes, anchors, and dots.
    Char,
    /// A sequence of nodes, left to right.
    Concat(Vec<Node>),
    /// A set of alternatives separated by `|`.
    Alt(Vec<Node>),
    /// A grouping construct. Capturing, non-capturing, named, and lookaround all
    /// reduce to this. The walk descends into the body without adding height.
    Group(Box<Node>),
    /// A quantified atom. Carries the atom it repeats.
    Repetition(Box<Node>),
}

/// Parse `pattern` into a [`Node`] tree.
///
/// Returns [`Error`] when the pattern is not valid ECMAScript regex syntax.
pub(crate) fn parse(pattern: &str) -> Result<Node, Error> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut p = Parser {
        chars,
        pos: 0,
        group_depth: 0,
        group_names: HashSet::new(),
    };
    let node = p.parse_disjunction()?;
    if p.pos != p.chars.len() {
        // A leftover character means an unbalanced close paren or similar.
        return Err(Error);
    }
    Ok(node)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    group_depth: usize,
    group_names: HashSet<String>,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// `Disjunction :: Alternative ( '|' Alternative )*`
    fn parse_disjunction(&mut self) -> Result<Node, Error> {
        let mut branches = vec![self.parse_alternative()?];
        while self.peek() == Some('|') {
            self.bump();
            branches.push(self.parse_alternative()?);
        }
        if branches.len() == 1 {
            Ok(branches.pop().expect("one branch"))
        } else {
            Ok(Node::Alt(branches))
        }
    }

    /// `Alternative :: Term*`
    ///
    /// Stops at `|` or `)` or end of input.
    fn parse_alternative(&mut self) -> Result<Node, Error> {
        let mut terms = Vec::new();
        loop {
            match self.peek() {
                None | Some('|') | Some(')') => break,
                _ => terms.push(self.parse_term()?),
            }
        }
        match terms.len() {
            0 => Ok(Node::Empty),
            1 => Ok(terms.pop().expect("one term")),
            _ => Ok(Node::Concat(terms)),
        }
    }

    /// `Term :: Atom Quantifier?`
    ///
    /// A quantifier turns the atom into a [`Node::Repetition`]. ECMAScript
    /// forbids three things this guards against. A quantifier cannot apply to a
    /// zero-width assertion such as `^`, `$`, `\b`, `\B`, or a lookbehind. A
    /// brace range `{n,m}` with `n > m` is a syntax error. A second quantifier
    /// cannot stack on a quantified atom, so `a*{2}` and `x{5}{2}` are errors.
    /// Each case produces an [`Error`].
    fn parse_term(&mut self) -> Result<Node, Error> {
        let (atom, quant) = self.parse_atom()?;
        let q = self.peek_quantifier();
        if q == Quant::None {
            return Ok(atom);
        }
        if quant == Quantifiable::No || q == Quant::InvalidRange {
            return Err(Error);
        }
        self.consume_quantifier()?;
        // A second quantifier on the same atom is a syntax error. An invalid
        // stacked range still occupies quantifier position, so it counts.
        if self.peek_quantifier() != Quant::None {
            return Err(Error);
        }
        Ok(Node::Repetition(Box::new(atom)))
    }

    /// Classify what sits at the cursor as a quantifier.
    ///
    /// `*`, `+`, `?` are always quantifiers. A brace is a quantifier only when
    /// it has the shape `{n}`, `{n,}`, or `{n,m}` with digit-run bounds. Such a
    /// brace is [`Quant::InvalidRange`] when it is `{n,m}` with `n > m`, which
    /// is a syntax error rather than a literal. A brace that is not a quantifier
    /// shape, such as `{` or `{a}`, is a literal and reports [`Quant::None`].
    fn peek_quantifier(&self) -> Quant {
        match self.peek() {
            Some('*') | Some('+') | Some('?') => Quant::Symbol,
            Some('{') => self.brace_quantifier(),
            _ => Quant::None,
        }
    }

    /// Consume the quantifier the cursor sits on, including an optional lazy `?`.
    fn consume_quantifier(&mut self) -> Result<(), Error> {
        match self.peek() {
            Some('*') | Some('+') | Some('?') => {
                self.bump();
            }
            Some('{') => {
                let len = match self.brace_quantifier() {
                    Quant::Range(len) => len,
                    _ => return Err(Error),
                };
                for _ in 0..len {
                    self.bump();
                }
            }
            _ => return Err(Error),
        }
        // Lazy marker.
        if self.peek() == Some('?') {
            self.bump();
        }
        Ok(())
    }

    /// Classify a brace starting at the cursor.
    ///
    /// Valid quantifier forms: `{n}`, `{n,}`, `{n,m}` with `n` and `m` being
    /// digit runs. `{n,m}` with `n > m` is [`Quant::InvalidRange`], a syntax
    /// error. Anything else is [`Quant::None`], a literal brace. Bounds parse
    /// with saturating arithmetic, so a very long digit run never overflows.
    fn brace_quantifier(&self) -> Quant {
        if self.peek() != Some('{') {
            return Quant::None;
        }
        let mut i = 1;
        let first_len = self.digit_run(i);
        if first_len == 0 {
            return Quant::None;
        }
        let lower = self.parse_bound(i, first_len);
        i += first_len;
        match self.peek_at(i) {
            Some('}') => Quant::Range(i + 1),
            Some(',') => {
                i += 1;
                let second_len = self.digit_run(i);
                let upper = self.parse_bound(i, second_len);
                i += second_len;
                if self.peek_at(i) != Some('}') {
                    return Quant::None;
                }
                // An explicit upper bound below the lower bound is a syntax
                // error. An omitted upper bound (`{n,}`) has no ceiling.
                if second_len > 0 && upper < lower {
                    return Quant::InvalidRange;
                }
                Quant::Range(i + 1)
            }
            _ => Quant::None,
        }
    }

    /// Read a digit run of `len` characters at `offset` from the cursor as a
    /// number. Saturates at `u64::MAX` so an enormous bound never overflows.
    /// The exact value past saturation does not change quantifier validity,
    /// since both bounds saturate to the same ceiling.
    fn parse_bound(&self, offset: usize, len: usize) -> u64 {
        let mut value: u64 = 0;
        for k in 0..len {
            let digit = self
                .peek_at(offset + k)
                .and_then(|c| c.to_digit(10))
                .unwrap_or(0);
            value = value.saturating_mul(10).saturating_add(u64::from(digit));
        }
        value
    }

    /// Count consecutive ASCII digits starting at `offset` from the cursor.
    fn digit_run(&self, offset: usize) -> usize {
        let mut n = 0;
        while matches!(self.peek_at(offset + n), Some(c) if c.is_ascii_digit()) {
            n += 1;
        }
        n
    }

    /// Parse one atom and report whether a quantifier may follow it.
    ///
    /// `Atom :: '.' | '\' escape | '[' class ']' | '(' group ')' | anchor | char`
    ///
    /// Most atoms accept a quantifier. The exceptions are zero-width assertions:
    /// the anchors `^` and `$`, the word boundaries `\b` and `\B`, and a
    /// lookbehind. A lookahead does accept a quantifier without the `u` flag, so
    /// it stays [`Quantifiable::Yes`].
    fn parse_atom(&mut self) -> Result<(Node, Quantifiable), Error> {
        match self.peek() {
            Some('(') => self.parse_group(),
            Some('[') => Ok((self.parse_class()?, Quantifiable::Yes)),
            Some('\\') => self.parse_escape(),
            // A quantifier with nothing to bind to is invalid in ECMAScript.
            // The brace forms `{n}`, `{n,}`, `{n,m}` are quantifiers too when
            // well formed, so a leading one is an error. A malformed brace such
            // as `{` or `a{` stays a literal and is handled below.
            Some('*') | Some('+') | Some('?') => Err(Error),
            Some('{') if self.brace_quantifier() != Quant::None => Err(Error),
            // A stray close paren is handled by the caller. Reaching it here is
            // a bug, but treat it as the end of an atom run defensively.
            Some(')') => Err(Error),
            // Anchors are zero-width and cannot take a quantifier.
            Some('^') | Some('$') => {
                self.bump();
                Ok((Node::Char, Quantifiable::No))
            }
            Some(_) => {
                // `.` and any literal char are a single quantifiable atom.
                self.bump();
                Ok((Node::Char, Quantifiable::Yes))
            }
            None => Err(Error),
        }
    }

    /// Parse a group starting at `(`.
    ///
    /// Handles capturing `(...)`, non-capturing `(?:...)`, named `(?<name>...)`,
    /// lookahead `(?=...)` `(?!...)`, and lookbehind `(?<=...)` `(?<!...)`.
    /// Rejects the atomic group `(?>...)` and any other unknown `(?` prefix.
    ///
    /// A lookbehind is zero-width and cannot take a quantifier, so it returns
    /// [`Quantifiable::No`]. Every other group, including a lookahead, may be
    /// quantified.
    fn parse_group(&mut self) -> Result<(Node, Quantifiable), Error> {
        // Consume '('.
        self.bump();

        if self.group_depth >= MAX_GROUP_NESTING {
            return Err(Error);
        }
        self.group_depth += 1;
        let group = self.parse_group_body();
        self.group_depth -= 1;
        group
    }

    fn parse_group_body(&mut self) -> Result<(Node, Quantifiable), Error> {
        let mut quant = Quantifiable::Yes;
        if self.peek() == Some('?') {
            self.bump();
            match self.peek() {
                Some(':') | Some('=') | Some('!') => {
                    self.bump();
                }
                Some('<') => {
                    // Lookbehind `(?<=` / `(?<!` or a named group `(?<name>`.
                    self.bump();
                    match self.peek() {
                        Some('=') | Some('!') => {
                            self.bump();
                            quant = Quantifiable::No;
                        }
                        _ => self.consume_group_name()?,
                    }
                }
                // `(?>` atomic groups and anything else are not ECMAScript.
                _ => return Err(Error),
            }
        }

        let body = self.parse_disjunction()?;

        if self.bump() != Some(')') {
            return Err(Error);
        }
        Ok((Node::Group(Box::new(body)), quant))
    }

    /// Consume a group name up to and including the closing `>`.
    ///
    /// The name must be a unique ECMAScript identifier name.
    fn consume_group_name(&mut self) -> Result<(), Error> {
        let mut name = String::new();
        loop {
            match self.peek() {
                Some('>') => break,
                Some('\\') => name.push(self.consume_group_name_escape()?),
                Some(c) => {
                    self.bump();
                    name.push(c);
                }
                None => return Err(Error),
            }
        }
        if !is_group_name(&name) || !self.group_names.insert(name) {
            return Err(Error);
        }
        self.bump();
        Ok(())
    }

    fn consume_group_name_escape(&mut self) -> Result<char, Error> {
        if self.bump() != Some('\\') || self.bump() != Some('u') {
            return Err(Error);
        }
        if self.peek() == Some('{') {
            self.bump();
            char::from_u32(self.consume_code_point_escape()?).ok_or(Error)
        } else {
            let value = self.consume_fixed_hex_value(4).ok_or(Error)?;
            match value {
                0xd800..=0xdbff => self.consume_trailing_surrogate_escape(value),
                0xdc00..=0xdfff => Err(Error),
                _ => char::from_u32(value).ok_or(Error),
            }
        }
    }

    fn consume_trailing_surrogate_escape(&mut self, lead: u32) -> Result<char, Error> {
        if self.bump() != Some('\\') || self.bump() != Some('u') {
            return Err(Error);
        }
        let trail = self.consume_fixed_hex_value(4).ok_or(Error)?;
        if !(0xdc00..=0xdfff).contains(&trail) {
            return Err(Error);
        }
        let code_point = 0x10000 + ((lead - 0xd800) << 10) + (trail - 0xdc00);
        char::from_u32(code_point).ok_or(Error)
    }

    fn consume_code_point_escape(&mut self) -> Result<u32, Error> {
        let mut value = 0u32;
        let mut digits = 0usize;
        loop {
            match self.peek() {
                Some('}') if digits > 0 => {
                    self.bump();
                    return Ok(value);
                }
                Some(c) if c.is_ascii_hexdigit() => {
                    value = value
                        .checked_mul(16)
                        .and_then(|v| v.checked_add(c.to_digit(16).expect("hex digit")))
                        .filter(|v| *v <= 0x10ffff)
                        .ok_or(Error)?;
                    digits += 1;
                    self.bump();
                }
                _ => return Err(Error),
            }
        }
    }

    fn consume_fixed_hex_value(&mut self, len: usize) -> Option<u32> {
        let mut value = 0u32;
        for i in 0..len {
            let c = self.peek_at(i)?;
            if !c.is_ascii_hexdigit() {
                return None;
            }
            value = value * 16 + c.to_digit(16).expect("hex digit");
        }
        for _ in 0..len {
            self.bump();
        }
        Some(value)
    }

    /// Parse a character class `[...]`, including a leading `^` and ranges.
    ///
    /// In ECMAScript a `]` right after `[` or `[^` closes the class, so `[]`
    /// matches nothing and `[^]` matches anything. Both are valid. Escapes
    /// inside the class are consumed as a unit so `\]` does not close it. An
    /// unterminated class is an error.
    fn parse_class(&mut self) -> Result<Node, Error> {
        // Consume '['.
        self.bump();
        if self.peek() == Some('^') {
            self.bump();
        }
        loop {
            match self.peek() {
                None => return Err(Error),
                Some(']') => {
                    self.bump();
                    return Ok(Node::Char);
                }
                Some('\\') => {
                    // Consume the backslash and the escaped character.
                    self.bump();
                    if self.bump().is_none() {
                        return Err(Error);
                    }
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    /// Parse an escape sequence starting at `\` and report quantifiability.
    ///
    /// The cursor sits on `\`. ECMAScript requires a character after it. The
    /// value of the escape does not matter to the heuristics, so a backslash
    /// plus one following character forms one atom. The word boundaries `\b`
    /// and `\B` are zero-width assertions and cannot take a quantifier, so they
    /// return [`Quantifiable::No`]. Every other escape may be quantified. A
    /// trailing backslash with nothing after it is an error.
    fn parse_escape(&mut self) -> Result<(Node, Quantifiable), Error> {
        // Consume '\'.
        self.bump();
        match self.bump() {
            None => Err(Error),
            Some('b') | Some('B') => Ok((Node::Char, Quantifiable::No)),
            Some(_) => Ok((Node::Char, Quantifiable::Yes)),
        }
    }
}

/// Whether a quantifier may attach to an atom.
///
/// ECMAScript rejects a quantifier on a zero-width assertion: the anchors `^`
/// and `$`, the word boundaries `\b` and `\B`, and a lookbehind. Those atoms
/// are [`Quantifiable::No`]. A lookahead is quantifiable without the `u` flag,
/// so it stays [`Quantifiable::Yes`] like ordinary atoms.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Quantifiable {
    Yes,
    No,
}

/// What sits at the cursor in quantifier position.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Quant {
    /// No quantifier. A brace here is a literal.
    None,
    /// A `*`, `+`, or `?` quantifier.
    Symbol,
    /// A well-formed brace quantifier. Carries its length in characters.
    Range(usize),
    /// A brace range `{n,m}` with `n > m`. A syntax error, not a literal.
    InvalidRange,
}

fn is_group_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_group_name_start(first) && chars.all(is_group_name_continue)
}

fn is_group_name_start(c: char) -> bool {
    c == '$' || c == '_' || unicode_id_start::is_id_start(c)
}

fn is_group_name_continue(c: char) -> bool {
    is_group_name_start(c)
        || unicode_id_start::is_id_continue(c)
        || c == '\u{200c}'
        || c == '\u{200d}'
}
