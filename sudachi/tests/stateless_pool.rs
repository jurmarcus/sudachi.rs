/*
 *  Copyright (c) 2026 Works Applications Co., Ltd.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 */

//! Tests for the per-thread `StatefulTokenizer` pool inside
//! `StatelessTokenizer<Arc<JapaneseDictionary>>`.

extern crate lazy_static;
extern crate sudachi;

use std::sync::Arc;
use std::thread;

use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::{Mode, Tokenize};
use sudachi::dic::dictionary::JapaneseDictionary;

mod common;
use common::TEST_CONFIG;

fn make_tokenizer() -> StatelessTokenizer<Arc<JapaneseDictionary>> {
    let dict = JapaneseDictionary::from_cfg(&TEST_CONFIG).expect("failed to make dictionary");
    StatelessTokenizer::new(Arc::new(dict))
}

#[test]
fn pool_returns_consistent_results_across_calls() {
    let tok = make_tokenizer();
    let s = "東京都";
    let r1 = tok.tokenize(s, Mode::C, false).unwrap();
    let r2 = tok.tokenize(s, Mode::C, false).unwrap();
    let r3 = tok.tokenize(s, Mode::C, false).unwrap();

    assert_eq!(r1.len(), r2.len());
    assert_eq!(r1.len(), r3.len());
    for i in 0..r1.len() {
        assert_eq!(r1.get(i).word_id(), r2.get(i).word_id());
        assert_eq!(r1.get(i).word_id(), r3.get(i).word_id());
    }
}

#[test]
fn pool_handles_mode_switching_within_same_thread() {
    let tok = make_tokenizer();
    let s = "東京都";
    let _ = tok.tokenize(s, Mode::A, false).unwrap();
    let _ = tok.tokenize(s, Mode::B, false).unwrap();
    let _ = tok.tokenize(s, Mode::C, false).unwrap();
    // Asserts only that no panic occurs and the pool tolerates mode changes.
}

#[test]
fn pool_isolates_per_thread() {
    let tok = Arc::new(make_tokenizer());
    let inputs = ["東京都", "京都市", "新宿区"];

    let handles: Vec<_> = (0..8)
        .map(|tid| {
            let tok = Arc::clone(&tok);
            thread::spawn(move || {
                for _ in 0..50 {
                    for s in &inputs {
                        let result = tok.tokenize(s, Mode::C, false);
                        assert!(
                            result.is_ok(),
                            "thread {tid} tokenize({s}) failed: {:?}",
                            result.err()
                        );
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }
}

#[test]
fn pool_distinguishes_two_dictionaries_in_same_thread() {
    // Two separate JapaneseDictionary instances → two separate pool entries
    // (different lexicon pointers), both reachable from the same thread.
    let tok1 = make_tokenizer();
    let tok2 = make_tokenizer();
    let r1 = tok1.tokenize("東京", Mode::C, false).unwrap();
    let r2 = tok2.tokenize("東京", Mode::C, false).unwrap();
    assert_eq!(r1.len(), r2.len());
    // Re-using the first should still work after the second pulled in a new entry.
    let r1_again = tok1.tokenize("東京", Mode::C, false).unwrap();
    assert_eq!(r1.len(), r1_again.len());
}
