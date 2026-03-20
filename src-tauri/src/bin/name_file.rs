/// name_file — thin CLI wrapper around grayslate_lib naming pipeline.
///
/// Usage:
///   echo "file content" | name_file [language_hint] [filename]
///
/// Arguments:
///   language_hint  Language to use for naming (e.g. "rust", "python", "json").
///                  Pass "auto" or omit to let Rust detect from content + filename.
///   filename       Original filename (e.g. "main.rs"). Used as a detection hint
///                  when language_hint is "auto". Optional.
///
/// Reads content from stdin, writes the suggested filename stem to stdout.
/// Prints nothing if the naming system cannot derive a useful name (fallback).
///
/// Language hints match the values used in the Grayslate naming pipeline, e.g.:
///   rust  typescript  javascript  python  svelte  json  toml  yaml
///   markdown  sql  html  css  go  java  cpp  c  bash  csv
///
/// This binary is intended for use by audit_repos.py to scan external repos.
use std::io::{self, Read};

use grayslate_lib::naming;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // language_hint defaults to "auto" when not provided.
    let lang_hint = args.get(1).map(|s| s.as_str()).unwrap_or("auto");

    // Optional filename — used as an extension hint when lang_hint is "auto".
    let filename = args.get(2).map(|s| s.as_str());

    let mut content = String::new();
    io::stdin().read_to_string(&mut content).ok();

    let (name, _lang) = naming::suggest_stem_auto(&content, lang_hint, filename);
    if let Some(name) = name {
        print!("{}", name);
    }
    // Nothing printed → caller treats it as a fallback.
}
