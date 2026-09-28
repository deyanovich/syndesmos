//! # syndesmos — sentence bonds over UAX #29
//!
//! Unicode's sentence rules (UAX #29) know nothing of a language's
//! abbreviations, so they break after *Dr.* and before *Smith*. A
//! `.desm` file names the bonds: the places where a sentence runs on
//! across a break the rules made. Atoms stay what the rules cut;
//! the bonds say which of them read as one sentence — molecules
//! from atoms.
//!
//! The core is an overlay: [`Syndesmos::apply`] takes a text and the
//! boundaries some UAX #29 implementation produced and returns the
//! ones that survive the bonds. That contract is the standard, and
//! it is what CLDR's sentence-break suppressions already mean.
//! With the `uax29` feature (default) the crate also offers the
//! drop-in — [`Syndesmos::unicode_sentences`] and
//! [`Syndesmos::split_sentence_bound_indices`], the same names as
//! unicode-segmentation's, bonds applied.
//!
//! ## The `.desm` file
//!
//! One rule per line; `#` comments and blank lines are ignored.
//!
//! - A **bare line** is an abbreviation, a whole word a sentence may
//!   end in without ending: `Dr.`, `Mrs.`, `e.g.`. The break after
//!   it is undone. What precedes the word must be the start, a
//!   space, or another non-letter, so `Undr.` is not `Dr.`.
//! - A **`/regex/` line** undoes a break that a match *straddles*:
//!   the match begins before the boundary and reaches it. UAX #29
//!   places the boundary at the start of the next segment, after
//!   the whitespace, so a pattern that spans the space spans the
//!   break: `/[A-Z]\. [A-Z]/` keeps initials together,
//!   `/[?!]” [a-z]/` keeps *“Why?” he asked* one sentence. `\/`
//!   escapes a slash; a trailing `i` folds case.
//! - `use en` pulls in the crate's built-in file for a language
//!   (seeded from CLDR), so a project file is its additions only.
//!
//! The regex flavor is the `regex` crate's: Unicode classes and
//! properties, no lookaround, no backreferences, linear-time. Every
//! rule of a file compiles into one set, and each boundary is tested
//! in one pass over a bounded window around it.

use regex::{Regex, RegexBuilder, RegexSet, RegexSetBuilder};
use std::fmt;

/// How far, in bytes, a pattern may reach on either side of a
/// boundary. A rule longer than this is not a sentence bond.
const WINDOW: usize = 96;

/// A failure to read a `.desm` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line in the file.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// The built-in language files, seeded from CLDR's sentence-break
/// suppressions.
const LANGUAGES: &[(&str, &str)] = &[
    ("de", include_str!("../desm/de.desm")),
    ("en", include_str!("../desm/en.desm")),
    ("es", include_str!("../desm/es.desm")),
    ("fr", include_str!("../desm/fr.desm")),
    ("it", include_str!("../desm/it.desm")),
    ("pt", include_str!("../desm/pt.desm")),
    ("ru", include_str!("../desm/ru.desm")),
];

/// A set of sentence bonds: the rules of one or more `.desm` files.
#[derive(Debug, Clone, Default)]
pub struct Syndesmos {
    abbreviations: Vec<String>,
    /// Each pattern's source spelling (as written, with flags) and
    /// its compiled form.
    patterns: Vec<(String, Regex)>,
    /// The patterns as one set, rebuilt when they change.
    set: Option<RegexSet>,
}

impl Syndesmos {
    /// No bonds: every UAX #29 boundary stands.
    pub fn empty() -> Self {
        Self::default()
    }

    /// The tags of the built-in language files.
    pub fn languages() -> Vec<&'static str> {
        LANGUAGES.iter().map(|(t, _)| *t).collect()
    }

    /// The built-in bonds for a language tag (`"en"`), if the crate
    /// ships one.
    pub fn language(tag: &str) -> Option<Self> {
        let (_, text) = LANGUAGES.iter().find(|(t, _)| *t == tag)?;
        Some(Self::parse(text).expect("the built-in files parse"))
    }

    /// Read a `.desm` text.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut out = Self::empty();
        for (i, raw) in text.lines().enumerate() {
            let line = i + 1;
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if let Some(tag) = l.strip_prefix("use ") {
                let tag = tag.trim();
                let lang = Self::language(tag).ok_or_else(|| ParseError {
                    line,
                    message: format!(
                        "no built-in language {tag:?} (have: {})",
                        Self::languages().join(", ")
                    ),
                })?;
                out.extend(lang);
                continue;
            }
            if let Some(rest) = l.strip_prefix('/') {
                let (pattern, tail) = split_pattern(rest).ok_or_else(|| ParseError {
                    line,
                    message: "unclosed /regex/".into(),
                })?;
                let mut flags = String::new();
                let mut tail = tail;
                while let Some(c) = tail.chars().next() {
                    if c == 'i' {
                        flags.push(c);
                        tail = &tail[1..];
                    } else {
                        break;
                    }
                }
                let tail = tail.trim();
                if !tail.is_empty() && !tail.starts_with('#') {
                    return Err(ParseError {
                        line,
                        message: format!(
                            "after the /regex/ only the flag `i` or a # comment may follow, not {tail:?}"
                        ),
                    });
                }
                let re = RegexBuilder::new(&pattern)
                    .case_insensitive(flags.contains('i'))
                    .build()
                    .map_err(|e| ParseError {
                        line,
                        message: format!("bad regex: {e}"),
                    })?;
                let source = format!("/{pattern}/{flags}");
                out.patterns.push((source, re));
                continue;
            }
            // A bare abbreviation, comment allowed after two spaces.
            let word = match l.find("  #") {
                Some(at) => l[..at].trim_end(),
                None => l,
            };
            out.abbreviations.push(word.to_string());
        }
        out.rebuild();
        Ok(out)
    }

    /// Read a `.desm` file.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::parse(&text)?)
    }

    /// Add another set's bonds to this one.
    pub fn extend(&mut self, other: Syndesmos) {
        for a in other.abbreviations {
            if !self.abbreviations.contains(&a) {
                self.abbreviations.push(a);
            }
        }
        for (src, re) in other.patterns {
            if !self.patterns.iter().any(|(s, _)| *s == src) {
                self.patterns.push((src, re));
            }
        }
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.set = if self.patterns.is_empty() {
            None
        } else {
            Some(
                RegexSetBuilder::new(self.patterns.iter().map(|(s, _)| {
                    // The set needs the flag inside the pattern.
                    let (pat, flags) = s[1..].rsplit_once('/').expect("source has slashes");
                    if flags.contains('i') {
                        format!("(?i){pat}")
                    } else {
                        pat.to_string()
                    }
                }))
                .build()
                .expect("patterns compiled individually"),
            )
        };
    }

    pub fn is_empty(&self) -> bool {
        self.abbreviations.is_empty() && self.patterns.is_empty()
    }

    /// The abbreviations, as written.
    pub fn abbreviations(&self) -> &[String] {
        &self.abbreviations
    }

    /// The patterns, as written (`/…/` with flags).
    pub fn patterns(&self) -> impl Iterator<Item = &str> {
        self.patterns.iter().map(|(s, _)| s.as_str())
    }

    /// Whether a bond undoes the boundary at `at` — the byte offset
    /// where the next segment begins.
    pub fn bonds(&self, text: &str, at: usize) -> bool {
        if at == 0 || at >= text.len() || !text.is_char_boundary(at) {
            return false;
        }
        // The abbreviation rule: the segment before ends in one, as
        // a whole word.
        let before = text[..at].trim_end();
        for a in &self.abbreviations {
            if let Some(head) = before.strip_suffix(a.as_str())
                && head
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric())
            {
                return true;
            }
        }
        // The pattern rule: a match straddles the boundary.
        let Some(set) = &self.set else {
            return false;
        };
        let mut lo = at.saturating_sub(WINDOW);
        while !text.is_char_boundary(lo) {
            lo += 1;
        }
        let mut hi = (at + WINDOW).min(text.len());
        while !text.is_char_boundary(hi) {
            hi -= 1;
        }
        let window = &text[lo..hi];
        let rel = at - lo;
        for idx in set.matches(window) {
            let (_, re) = &self.patterns[idx];
            if re
                .find_iter(window)
                .any(|m| m.start() < rel && m.end() >= rel)
            {
                return true;
            }
        }
        false
    }

    /// The overlay: of the boundaries a UAX #29 implementation
    /// produced (byte offsets where segments begin, the first
    /// segment's 0 excluded), those the bonds leave standing.
    pub fn apply(&self, text: &str, boundaries: &[usize]) -> Vec<usize> {
        boundaries
            .iter()
            .copied()
            .filter(|&b| !self.bonds(text, b))
            .collect()
    }
}

/// Split `rest` (after the opening slash) at the closing unescaped
/// slash: the pattern with `\/` unescaped, and what follows.
fn split_pattern(rest: &str) -> Option<(String, &str)> {
    let mut pattern = String::new();
    let mut chars = rest.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, '/')) => pattern.push('/'),
                Some((_, d)) => {
                    pattern.push('\\');
                    pattern.push(d);
                }
                None => return None,
            },
            '/' => return Some((pattern, &rest[i + 1..])),
            _ => pattern.push(c),
        }
    }
    None
}

#[cfg(feature = "uax29")]
impl Syndesmos {
    /// unicode-segmentation's `split_sentence_bound_indices`, bonds
    /// applied: `(byte offset, segment)` per sentence.
    pub fn split_sentence_bound_indices<'a>(&self, text: &'a str) -> Vec<(usize, &'a str)> {
        use unicode_segmentation::UnicodeSegmentation;
        let raw: Vec<usize> = text
            .split_sentence_bound_indices()
            .map(|(at, _)| at)
            .filter(|&at| at > 0)
            .collect();
        let kept = self.apply(text, &raw);
        let mut out = Vec::with_capacity(kept.len() + 1);
        let mut start = 0;
        for b in kept {
            out.push((start, &text[start..b]));
            start = b;
        }
        if start < text.len() || out.is_empty() && !text.is_empty() {
            out.push((start, &text[start..]));
        }
        out
    }

    /// unicode-segmentation's `unicode_sentences`, bonds applied.
    pub fn unicode_sentences<'a>(&self, text: &'a str) -> Vec<&'a str> {
        self.split_sentence_bound_indices(text)
            .into_iter()
            .map(|(_, s)| s)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_kinds() {
        let s = Syndesmos::parse("# c\nDr.\n\nMrs.  # honorific\n/[A-Z]\\. [A-Z]/\n/etc\\. [a-z]/i  # comment\n/a\\/b/\n").unwrap();
        assert_eq!(s.abbreviations(), ["Dr.", "Mrs."]);
        let p: Vec<&str> = s.patterns().collect();
        assert_eq!(p, ["/[A-Z]\\. [A-Z]/", "/etc\\. [a-z]/i", "/a/b/"]);
        assert!(Syndesmos::parse("/unclosed").is_err());
        assert!(Syndesmos::parse("/x/ junk").is_err());
        assert!(Syndesmos::parse("/(/").is_err());
        assert!(Syndesmos::parse("use xx").is_err());
        assert!(!Syndesmos::parse("use en").unwrap().is_empty());
    }

    #[test]
    fn bonds_straddle_the_boundary() {
        let s = Syndesmos::parse("Dr.\n/[A-Z]\\. [A-Z]/\n/[?!]” [a-z]/\n").unwrap();
        let t = "I met Dr. Smith. J. R. R. wrote. “Why?” he asked. Undr. Then.";
        let at = |w: &str| t.find(w).unwrap();
        assert!(s.bonds(t, at("Smith")));
        assert!(s.bonds(t, at("R. R.")));
        assert!(s.bonds(t, at("he asked")));
        assert!(!s.bonds(t, at("J. R.")));
        assert!(!s.bonds(t, at("Then")), "Undr. is not Dr.");
        assert!(!s.bonds(t, 0));
    }

    #[cfg(feature = "uax29")]
    #[test]
    fn drop_in_joins_sentences() {
        let s = Syndesmos::parse("Dr.\n").unwrap();
        let t = "I met Dr. Smith. He left.";
        assert_eq!(s.unicode_sentences(t), ["I met Dr. Smith. ", "He left."]);
        assert_eq!(
            Syndesmos::empty().unicode_sentences(t),
            ["I met Dr. ", "Smith. ", "He left."]
        );
        assert_eq!(s.split_sentence_bound_indices(t)[1].0, 17);
        assert!(Syndesmos::empty().unicode_sentences("").is_empty());
    }

    #[test]
    fn the_built_in_languages_load_and_bond() {
        for tag in Syndesmos::languages() {
            assert!(!Syndesmos::language(tag).unwrap().is_empty(), "{tag}");
        }
        let en = Syndesmos::language("en").unwrap();
        assert!(en.bonds("See Mr. Twain.", 8));
        let de = Syndesmos::language("de").unwrap();
        assert!(de.bonds("Er kam z.B. heute.", 12));
    }
}
