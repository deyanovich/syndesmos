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

## The `.desm` file

One rule per line; `#` comments and blank lines are ignored.

```
# twain.desm — bonds for a Twain corpus
#!cldr en 48.2         # Unicode CLDR's English suppressions, that release
#!include house.desm   # another .desm beside this one
Mr.                    # a bare line is an abbreviation…
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
  one, so CLDR's lists convert without interpretation.
- A **`/regex/` line** undoes a break that a match *straddles*:
  the match begins before the boundary and reaches it. UAX #29
  places the boundary at the start of the next segment, after the
  whitespace, so a pattern that spans the space spans the break. A
  left-only condition ends in the space (`/etc\. /`); a right-only
  one starts with it (`/ [a-z]/`). `\/` escapes a slash; a trailing
  `i` folds case; nothing else may follow but a comment.
- **`#!cldr TAG [VERSION]`** merges Unicode CLDR's sentence-break
  suppressions for a language. With no version, or the version this
  crate ships (`syndesmos::CLDR_VERSION`), the shipped list is used;
  another version is read from the cache that `desm cldr get
  VERSION` fills, and refused with that command named when absent —
  a written version is a pin, never a silent substitution.
- **`#!include PATH`** merges another `.desm`, resolved against the
  including file's directory. Includes nest; a cycle is an error.

Directives begin with `#!`, so they are comments to any reader that
knows none, and a bond line never begins with `#`: the two cannot
collide whatever punctuation a path carries.

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

let bonds = Syndesmos::load("twain.desm")?;   // #!cldr and #!include resolved

// The overlay: boundaries in, boundaries out.
let kept = bonds.apply(text, &boundaries);

// The drop-in (feature `uax29`, default): unicode-segmentation's
// names, bonds applied.
for (at, sentence) in bonds.split_sentence_bound_indices(text) { … }
for sentence in bonds.unicode_sentences(text) { … }
```

## The CLDR lists

`cldr/*.desm` are Unicode CLDR's sentence-break suppressions,
verbatim, one file per language that has them: `de`, `en`, `es`,
`fr`, `it`, `pt`, `ru`. They come from a named release — **48.2**
(`release-48-2`, `common/segments/<tag>.xml`,
`<suppressions type="standard">`) — which every file's header
states and `syndesmos::CLDR_VERSION` states in code. The crate
never edits them: they are written by its own converter, and a
new release is `desm cldr get <version> --into cldr/`.

`Syndesmos::cldr("en")` reads a shipped list in code (for a
consumer with no filesystem, such as a browser build);
`Syndesmos::cldr_languages()` lists the tags.

```
desm cldr get 48.2               # into the cache, for #!cldr en 48.2
desm cldr get 47 --into vendor/  # any release, any directory
desm cldr get latest --into cldr/
```

`get` reads the release archive Unicode publishes
(`unicode.org/Public/cldr/<version>/cldr-common-<version>.zip`,
a few tens of megabytes), which never changes once released, so a
fetch by version is reproducible; `latest` takes the highest
version directory there that holds an archive (a directory
prepared for a coming release is passed over). That is a
maintenance question, so a build step names a version.

## The `desm` binary

```
desm check twain.desm            # parse, report the first error
desm show twain.desm             # the effective bonds, directives resolved
desm segment twain.desm < book.txt           # one sentence per line
desm segment --cldr en --offsets < book.txt  # a shipped CLDR list alone
```

## Features

- `uax29` (default): the drop-in over unicode-segmentation.
- `fetch`: `cldr::fetch` and `cldr::latest` (an HTTP client).
- `cli` (default): the `desm` binary; implies both.

With all off the crate is the overlay and the converter alone,
`regex` its only dependency.

## License

MIT or Apache-2.0, at your option. The lists under `cldr/` are
Unicode CLDR data (release 48.2), © Unicode, Inc., under the
Unicode License (`LICENSE-UNICODE`).
