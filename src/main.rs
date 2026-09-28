//! `desm` — check a `.desm` file, show the effective bonds, segment
//! text with them, or fetch Unicode CLDR's lists as `.desm` files.

use clap::{Parser, Subcommand};
use std::io::Read;
use syndesmos::Syndesmos;

#[derive(Parser)]
#[command(name = "desm", version, about = "Sentence bonds over UAX #29")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Parse the files and report the first error, if any.
    Check { files: Vec<std::path::PathBuf> },
    /// Print the effective bonds of files and/or a shipped CLDR list.
    Show {
        /// A shipped CLDR suppression list by language tag.
        #[arg(long, value_name = "TAG")]
        cldr: Option<String>,
        files: Vec<std::path::PathBuf>,
    },
    /// Segment standard input into sentences, one per line.
    Segment {
        /// A shipped CLDR suppression list by language tag.
        #[arg(long, value_name = "TAG")]
        cldr: Option<String>,
        /// Print `offset\tsentence` instead of the sentence alone.
        #[arg(long)]
        offsets: bool,
        files: Vec<std::path::PathBuf>,
    },
    /// Unicode CLDR's sentence-break suppressions as .desm files.
    #[command(subcommand)]
    Cldr(CldrCmd),
}

#[derive(Subcommand)]
enum CldrCmd {
    /// Fetch a CLDR release (`48.2`, or `latest`) and write one
    /// .desm per language that carries sentence-break suppressions.
    Get {
        /// The release, `48.2`-shaped, or `latest`.
        version: String,
        /// Where to write; default: the cache `#!cldr TAG VERSION`
        /// reads ($XDG_CACHE_HOME/syndesmos/cldr/<version>/).
        #[arg(long, value_name = "DIR")]
        into: Option<std::path::PathBuf>,
    },
}

fn gather(cldr: Option<&str>, files: &[std::path::PathBuf]) -> Result<Syndesmos, String> {
    let mut s = match cldr {
        Some(tag) => Syndesmos::cldr(tag).ok_or_else(|| {
            format!(
                "no CLDR list for {tag:?} (CLDR {}: {})",
                syndesmos::CLDR_VERSION,
                Syndesmos::cldr_languages().join(", ")
            )
        })?,
        None => Syndesmos::empty(),
    };
    for f in files {
        s.extend(Syndesmos::load(f).map_err(|e| format!("{}: {e}", f.display()))?);
    }
    Ok(s)
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Check { files } => files.iter().try_for_each(|f| {
            Syndesmos::load(f)
                .map(|s| {
                    println!(
                        "{}: {} abbreviations, {} patterns",
                        f.display(),
                        s.abbreviations().len(),
                        s.patterns().count()
                    )
                })
                .map_err(|e| format!("{}: {e}", f.display()))
        }),
        Cmd::Show { cldr, files } => gather(cldr.as_deref(), &files).map(|s| {
            for a in s.abbreviations() {
                println!("{a}");
            }
            for p in s.patterns() {
                println!("{p}");
            }
        }),
        Cmd::Segment {
            cldr,
            offsets,
            files,
        } => gather(cldr.as_deref(), &files).and_then(|s| {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            for (at, sentence) in s.split_sentence_bound_indices(&text) {
                let sentence = sentence.trim();
                if sentence.is_empty() {
                    continue;
                }
                if offsets {
                    println!("{at}\t{sentence}");
                } else {
                    println!("{sentence}");
                }
            }
            Ok(())
        }),
        Cmd::Cldr(CldrCmd::Get { version, into }) => {
            syndesmos::cldr::fetch(&version).and_then(|(version, files)| {
                let dir = match into {
                    Some(d) => d,
                    None => syndesmos::cldr::cache_dir()
                        .ok_or_else(|| {
                            "no cache directory (set HOME or XDG_CACHE_HOME)".to_string()
                        })?
                        .join(&version),
                };
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                for (tag, text) in &files {
                    let path = dir.join(format!("{tag}.desm"));
                    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
                    println!("{}", path.display());
                }
                eprintln!("CLDR {version}: {} languages", files.len());
                Ok(())
            })
        }
    };
    if let Err(e) = result {
        eprintln!("desm: {e}");
        std::process::exit(1);
    }
}
