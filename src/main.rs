//! `desm` — check a `.desm` file, show the effective bonds, or
//! segment text with them.

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
    /// Print the effective bonds of a language and/or files.
    Show {
        #[arg(long)]
        lang: Option<String>,
        files: Vec<std::path::PathBuf>,
    },
    /// Segment standard input into sentences, one per line.
    Segment {
        #[arg(long)]
        lang: Option<String>,
        /// Print `offset\tsentence` instead of the sentence alone.
        #[arg(long)]
        offsets: bool,
        files: Vec<std::path::PathBuf>,
    },
}

fn gather(lang: Option<&str>, files: &[std::path::PathBuf]) -> Result<Syndesmos, String> {
    let mut s = match lang {
        Some(tag) => Syndesmos::language(tag).ok_or_else(|| {
            format!(
                "no built-in language {tag:?} (have: {})",
                Syndesmos::languages().join(", ")
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
        Cmd::Show { lang, files } => gather(lang.as_deref(), &files).map(|s| {
            for a in s.abbreviations() {
                println!("{a}");
            }
            for p in s.patterns() {
                println!("{p}");
            }
        }),
        Cmd::Segment {
            lang,
            offsets,
            files,
        } => gather(lang.as_deref(), &files).and_then(|s| {
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
    };
    if let Err(e) = result {
        eprintln!("desm: {e}");
        std::process::exit(1);
    }
}
