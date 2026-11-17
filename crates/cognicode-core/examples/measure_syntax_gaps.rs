//! Measure which source files the bundled grammars cannot read cleanly.
//!
//! This example is an instrument, not a product route. It reads one file
//! path per line from stdin and, for every file whose extension a language
//! reader claims, prints one tab-separated record:
//!
//! `language <TAB> clean|fault|unreadable <TAB> path <TAB> line:column <TAB> detail`
//!
//! Deciding whether a fault is valid code the grammar lacks or broken code is
//! the caller's job; `scripts/measure/syntax_gaps.py` drives this binary over
//! a corpus and aggregates the records.

use std::io::BufRead;
use std::path::Path;

use cognicode_core::application::ingest::extractor::extract_file_checked;
use cognicode_core::infrastructure::parser::LanguageConfig;

fn main() {
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let path = Path::new(&line);
        let Some(config) = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(LanguageConfig::from_extension)
        else {
            continue;
        };
        let language = format!("{:?}", config.language);
        let Ok(source) = std::fs::read_to_string(path) else {
            println!("{language}\tunreadable\t{line}\t\t");
            continue;
        };
        let (_, _, fault) = extract_file_checked(config, path, &source, "measure");
        match fault {
            None => println!("{language}\tclean\t{line}\t\t"),
            Some(fault) => println!(
                "{language}\tfault\t{line}\t{}:{}\t{}",
                fault.line,
                fault.column,
                fault.missing.as_deref().unwrap_or("unexpected input")
            ),
        }
    }
}
