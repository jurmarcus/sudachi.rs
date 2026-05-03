/*
 *  Copyright (c) 2026 Works Applications Co., Ltd.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 */

//! Tests for `JapaneseDictionary::from_system_bytes` and
//! `from_system_static_bytes` — the byte-array constructors used in WASM
//! and embedded contexts.
//!
//! These constructors use `Config::new_embedded()` which configures the
//! standard MeCab OOV plugin and expects a real Sudachi dictionary
//! (system_full.dic / system_core.dic / system_small.dic) with the
//! standard POS list. The tiny synthetic test dict under `tests/resources/`
//! does not satisfy this — it omits POS entries the standard plugins
//! require.
//!
//! We therefore look for a real Sudachi dictionary at:
//!   1. `$SUDACHI_DICT_PATH` env var
//!   2. `~/.local/share/sudachi/sudachi-dictionary-latest/system_full.dic`
//!   3. Newest `~/.local/share/sudachi/sudachi-dictionary-*/system_full.dic`
//!
//! If none is found, the tests print a notice and skip — this lets CI
//! pass on machines without an installed Sudachi dictionary while still
//! providing meaningful coverage for developers who do have one.

extern crate sudachi;

use std::path::PathBuf;
use std::sync::Arc;

use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::{Mode, Tokenize};
use sudachi::dic::dictionary::JapaneseDictionary;

/// Try to locate a real installed Sudachi system dictionary. Returns None
/// if no dictionary is found, in which case tests should skip.
fn find_real_system_dict() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SUDACHI_DICT_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home).join(".local/share/sudachi");

    let latest = base.join("sudachi-dictionary-latest/system_full.dic");
    if latest.exists() {
        return Some(latest);
    }

    let entries = std::fs::read_dir(&base).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("sudachi-dictionary-"))
        })
        .map(|p| p.join("system_full.dic"))
        .filter(|p| p.exists())
        .collect();
    candidates.sort();
    candidates.pop()
}

/// Skip-if-no-dict marker: prints to stderr and returns.
macro_rules! require_dict {
    () => {
        match find_real_system_dict() {
            Some(p) => p,
            None => {
                eprintln!(
                    "SKIP: no installed Sudachi dictionary found \
                     (set SUDACHI_DICT_PATH or install one under ~/.local/share/sudachi/)"
                );
                return;
            }
        }
    };
}

#[test]
fn from_system_bytes_roundtrip() {
    let dict_path = require_dict!();
    let bytes = std::fs::read(&dict_path).expect("failed to read system dict");
    let dict =
        JapaneseDictionary::from_system_bytes(bytes).expect("from_system_bytes should succeed");
    let tokenizer = StatelessTokenizer::new(Arc::new(dict));
    let result = tokenizer
        .tokenize("東京都", Mode::C, false)
        .expect("tokenize should succeed");
    assert!(!result.is_empty(), "expected at least one morpheme");
}

#[test]
fn from_system_static_bytes_roundtrip() {
    let dict_path = require_dict!();
    let bytes = std::fs::read(&dict_path).expect("failed to read system dict");
    // Box::leak gives us a 'static slice for testing; in real WASM use,
    // include_bytes! provides a 'static slice naturally.
    let bytes_static: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let dict = JapaneseDictionary::from_system_static_bytes(bytes_static)
        .expect("from_system_static_bytes should succeed");
    let tokenizer = StatelessTokenizer::new(Arc::new(dict));
    let result = tokenizer
        .tokenize("東京都", Mode::C, false)
        .expect("tokenize should succeed");
    assert!(!result.is_empty(), "expected at least one morpheme");
}

#[test]
fn from_system_bytes_produces_consistent_results() {
    // Sanity check: two from_system_bytes loads of the same bytes produce
    // identical tokenization for the same input. Catches non-determinism.
    let dict_path = require_dict!();
    let bytes_1 = std::fs::read(&dict_path).expect("failed to read system dict");
    let bytes_2 = bytes_1.clone();

    let dict_1 = Arc::new(JapaneseDictionary::from_system_bytes(bytes_1).unwrap());
    let dict_2 = Arc::new(JapaneseDictionary::from_system_bytes(bytes_2).unwrap());

    let tok_1 = StatelessTokenizer::new(Arc::clone(&dict_1));
    let tok_2 = StatelessTokenizer::new(Arc::clone(&dict_2));

    for input in ["東京", "今日", "学生", "コーヒー"] {
        let r1 = tok_1.tokenize(input, Mode::C, false).unwrap();
        let r2 = tok_2.tokenize(input, Mode::C, false).unwrap();
        assert_eq!(r1.len(), r2.len(), "morpheme count differs for {input}");
        for i in 0..r1.len() {
            assert_eq!(
                r1.get(i).word_id(),
                r2.get(i).word_id(),
                "word_id differs at idx {i} for {input}"
            );
        }
    }
}
