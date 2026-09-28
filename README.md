# syndesmos

Sentence bonds over UAX #29.

Unicode's sentence rules (UAX #29) know nothing of a language's
abbreviations, so they break after *Dr.* and before *Smith*. A
`.desm` file names the *bonds*: the places where a sentence runs
on across a break the rules made. Atoms stay what the rules cut;
the bonds say which of them read as one sentence — molecules from
atoms. The name is Greek σύνδεσμος, "a binding together": the
chemical bond, the grammarians' word for what joins clauses, and
the joint that holds two bones together without fusing them.

Shared by [quarb](https://quarb.org) (the corpus reading and the
`sentences` function), literium (a display layer over its
sentence atoms) and atrep.

## The `.desm` file

One rule per line; `#` comments and blank lines are ignored.

```
# twain.desm — bonds for a Twain corpus
use en                 # the built-in English list (CLDR)
Injun                  # no: a bare line is an abbreviation…
Mr.                    # …a whole word a sentence may end in
/[A-Z]\. [A-Z]/        # initials: J. R. R. Tolkien
/No\. \d/              # No. 7
/[?!]” [a-z]/          # “Why?” he asked
/\.{3} [A-Z]/          # an ellipsis the sentence runs across
/etc\. [a-z]/i         # trailing i: case-insensitive
```

- A **bare line** is an abbreviation, a whole word a sentence may
  end in without ending. The break after it is undone. What
  precedes the word must be the start, a space or another
  non-letter, so `Undr.` is not `Dr.`. A comment may follow after
  two spaces. This is CLDR's sentence-break suppression, one to
  one, so the language seeds convert without interpretation.
- A **`/regex/` line** undoes a break that a match *straddles*:
  the match begins before the boundary and reaches it. UAX #29
  places the boundary at the start of the next segment, after the
  whitespace, so a pattern that spans the space spans the break. A
  left-only condition ends in the space (`/etc\. /`); a right-only
  one starts with it (`/ [a-z]/`). `\/` escapes a slash; a trailing
  `i` folds case; nothing else may follow but a comment.
- **`use LANG`** pulls in the crate's built-in file for a language,
  so a project file is its additions only.

Rules only ever *undo* breaks; nothing in a `.desm` adds one. A
text whose sentences lack terminators needs a segmenter decision
UAX #29 did not make, which is cleanup, not a bond. The one case
no rule file resolves is an abbreviation that really ends a
sentence (*in the U.S. Then…*); CLDR accepts the same limit, and
an external annotation (CoNLL-U) is the answer when it matters.

### The regex flavor

The [`regex`](https://docs.rs/regex) crate's dialect: Unicode
classes and properties (`\p{Lu}`), alternation, bounded
repetition; no lookaround, no backreferences; linear-time
matching. The straddle rule removes the main reason to want
lookaround (a left-and-right condition is one pattern across the
boundary), every rule of a file compiles into one set, and each
boundary is tested in a single pass over a window of 96 bytes on
either side. A rule file is user-written input, and a
linear-time engine cannot be made to misbehave by one.

## The overlay, and the drop-in

The core is an overlay, and it is the standard: `apply(text,
boundaries)` takes a text and the boundary offsets some UAX #29
implementation produced (byte offsets where segments begin) and
returns the ones that survive the bonds. That contract does not
care who segmented (unicode-segmentation, ICU, Python's, Go's),
it is what CLDR's suppressions already mean, and any
implementation can follow it from this page.

```rust
use syndesmos::Syndesmos;

let mut bonds = Syndesmos::language("en").unwrap();
bonds.extend(Syndesmos::load("twain.desm")?);

// The overlay: boundaries in, boundaries out.
let kept = bonds.apply(text, &boundaries);

// The drop-in (feature `uax29`, default): unicode-segmentation's
// names, bonds applied.
for (at, sentence) in bonds.split_sentence_bound_indices(text) { … }
for sentence in bonds.unicode_sentences(text) { … }
```

The built-in language files (`desm/*.desm`) are seeded from CLDR's
sentence-break suppressions: `de`, `en`, `es`, `fr`, `it`, `pt`,
`ru`. `Syndesmos::languages()` lists them.

## The `desm` binary

```
desm check twain.desm            # parse, report the first error
desm show --lang en twain.desm   # the effective bonds
desm segment --lang en < book.txt          # one sentence per line
desm segment --lang en --offsets < book.txt
```

## Features

- `uax29` (default): the drop-in over unicode-segmentation.
- `cli` (default): the `desm` binary.

With both off the crate is the overlay alone, `regex` its only
dependency.

## License

MIT or Apache-2.0, at your option. The seed lists under `desm/`
derive from Unicode CLDR data, © Unicode, Inc., under the Unicode
License (`LICENSE-UNICODE`).
