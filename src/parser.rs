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

/// A parse failure. The pattern is not a valid ECMAScript regular expression.
#[derive(Debug, PartialEq, Eq)]
pub struct Error;

/// The reduced syntax tree.
///
/// Only repetitions and grouping carry meaning for the heuristics. Everything
/// else collapses into [`Node::Char`] or a list.
#[derive(Debug)]
pub enum Node {
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
pub fn parse(pattern: &str) -> Result<Node, Error> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut p = Parser { chars, pos: 0 };
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
    /// A quantifier turns the atom into a [`Node::Repetition`].
    fn parse_term(&mut self) -> Result<Node, Error> {
        let atom = self.parse_atom()?;
        if self.at_quantifier() {
            self.consume_quantifier()?;
            Ok(Node::Repetition(Box::new(atom)))
        } else {
            Ok(atom)
        }
    }

    /// Does a quantifier start at the cursor?
    ///
    /// `*`, `+`, `?` always qualify. `{` qualifies only when it forms a valid
    /// `{n}`, `{n,}`, or `{n,m}`. A lone `{` is a literal brace in ECMAScript.
    fn at_quantifier(&self) -> bool {
        match self.peek() {
            Some('*') | Some('+') | Some('?') => true,
            Some('{') => self.brace_quantifier_len().is_some(),
            _ => false,
        }
    }

    /// Consume the quantifier the cursor sits on, including an optional lazy `?`.
    fn consume_quantifier(&mut self) -> Result<(), Error> {
        match self.peek() {
            Some('*') | Some('+') | Some('?') => {
                self.bump();
            }
            Some('{') => {
                let len = self.brace_quantifier_len().ok_or(Error)?;
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

    /// If a valid brace quantifier starts at the cursor, return its length in
    /// characters including the braces. Otherwise return `None`.
    ///
    /// Valid forms: `{n}`, `{n,}`, `{n,m}` with `n` and `m` being digit runs.
    /// For `{n,m}` the bounds may be any digits. ECMAScript does not require
    /// `n <= m` at parse time, so neither do we.
    fn brace_quantifier_len(&self) -> Option<usize> {
        if self.peek() != Some('{') {
            return None;
        }
        let mut i = 1;
        let first_len = self.digit_run(i);
        if first_len == 0 {
            return None;
        }
        i += first_len;
        match self.peek_at(i) {
            Some('}') => Some(i + 1),
            Some(',') => {
                i += 1;
                let second_len = self.digit_run(i);
                i += second_len;
                if self.peek_at(i) == Some('}') {
                    Some(i + 1)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Count consecutive ASCII digits starting at `offset` from the cursor.
    fn digit_run(&self, offset: usize) -> usize {
        let mut n = 0;
        while matches!(self.peek_at(offset + n), Some(c) if c.is_ascii_digit()) {
            n += 1;
        }
        n
    }

    /// `Atom :: '.' | '\' escape | '[' class ']' | '(' group ')' | anchor | char`
    fn parse_atom(&mut self) -> Result<Node, Error> {
        match self.peek() {
            Some('(') => self.parse_group(),
            Some('[') => self.parse_class(),
            Some('\\') => self.parse_escape(),
            // A quantifier with nothing to bind to is invalid in ECMAScript.
            Some('*') | Some('+') | Some('?') => Err(Error),
            // A stray close paren is handled by the caller. Reaching it here is
            // a bug, but treat it as the end of an atom run defensively.
            Some(')') => Err(Error),
            Some(_) => {
                // `^`, `$`, `.`, and any literal char are a single atom.
                self.bump();
                Ok(Node::Char)
            }
            None => Err(Error),
        }
    }

    /// Parse a group starting at `(`.
    ///
    /// Handles capturing `(...)`, non-capturing `(?:...)`, named `(?<name>...)`,
    /// lookahead `(?=...)` `(?!...)`, and lookbehind `(?<=...)` `(?<!...)`.
    /// Rejects the atomic group `(?>...)` and any other unknown `(?` prefix.
    fn parse_group(&mut self) -> Result<Node, Error> {
        // Consume '('.
        self.bump();

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
        Ok(Node::Group(Box::new(body)))
    }

    /// Consume a group name up to and including the closing `>`.
    ///
    /// The name must be non-empty. The parser does not validate the identifier
    /// character set beyond requiring at least one character before `>`.
    fn consume_group_name(&mut self) -> Result<(), Error> {
        let mut count = 0;
        loop {
            match self.bump() {
                Some('>') => break,
                Some(_) => count += 1,
                None => return Err(Error),
            }
        }
        if count == 0 {
            return Err(Error);
        }
        Ok(())
    }

    /// Parse a character class `[...]`, including a leading `^` and ranges.
    ///
    /// A `]` immediately after `[` or `[^` is a literal member, matching
    /// ECMAScript. Escapes inside the class are consumed as a unit so `\]`
    /// does not close it. An unterminated class is an error.
    fn parse_class(&mut self) -> Result<Node, Error> {
        // Consume '['.
        self.bump();
        if self.peek() == Some('^') {
            self.bump();
        }
        // A literal ']' may appear first.
        if self.peek() == Some(']') {
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

    /// Parse an escape sequence starting at `\`.
    ///
    /// The cursor sits on `\`. ECMAScript requires a character after it. The
    /// value of the escape does not matter to the heuristics, so a backslash
    /// plus one following character forms one atom. A trailing backslash with
    /// nothing after it is an error.
    fn parse_escape(&mut self) -> Result<Node, Error> {
        // Consume '\'.
        self.bump();
        if self.bump().is_none() {
            return Err(Error);
        }
        Ok(Node::Char)
    }
}
