/*
 *  Copyright (c) 2026 Works Applications Co., Ltd.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 */

//! Owned snapshot of a [`Morpheme`].
//!
//! [`Morpheme`] borrows from its parent [`MorphemeList`] (and through it from
//! the dictionary mmap and input buffer). For consumers that want to hold
//! morpheme data past the list's lifetime — building owned tokens for
//! search indexing, sending across threads, persisting in caches — the
//! naive escape pattern is to clone every string field per morpheme:
//!
//! ```text
//! Token {
//!     surface:        m.surface().to_owned(),     // 1 alloc
//!     dictionary_form: m.dictionary_form().to_owned(),     // 1 alloc
//!     normalized_form: m.normalized_form().to_owned(),     // 1 alloc
//!     reading_form:    m.reading_form().to_owned(),        // 1 alloc
//!     part_of_speech:  m.part_of_speech().to_vec(),        // 1+N allocs
//! }
//! ```
//!
//! That is 5+ allocations per morpheme. For a 30-token sentence: 180+
//! small allocations just to escape the borrow.
//!
//! [`OwnedMorpheme`] collapses this to one allocation per morpheme by
//! packing all string fields into a single arena ([`Box<str>`]) with
//! per-field byte ranges. Construct via [`crate::analysis::morpheme::Morpheme::into_owned`].
//!
//! [`Morpheme`]: crate::analysis::morpheme::Morpheme
//! [`MorphemeList`]: crate::analysis::mlist::MorphemeList

use std::ops::Range;

/// An owned, lifetime-free snapshot of a morpheme.
///
/// All string fields share a single backing [`Box<str>`] arena; accessors
/// return borrowed `&str` slices into it. POS components are addressed by
/// a small `Vec<Range<u32>>` (one range per component, not one allocation
/// per component).
#[derive(Debug, Clone)]
pub struct OwnedMorpheme {
    pub(crate) arena: Box<str>,
    pub(crate) surface: Range<u32>,
    pub(crate) dictionary_form: Range<u32>,
    pub(crate) normalized_form: Range<u32>,
    pub(crate) reading_form: Range<u32>,
    pub(crate) pos: Vec<Range<u32>>,
    pub(crate) pos_id: u16,
    pub(crate) word_id: u32,
    pub(crate) is_oov: bool,
    pub(crate) begin_bytes: usize,
    pub(crate) end_bytes: usize,
    pub(crate) begin_chars: usize,
    pub(crate) end_chars: usize,
}

impl OwnedMorpheme {
    #[inline]
    fn slice(&self, r: &Range<u32>) -> &str {
        &self.arena[r.start as usize..r.end as usize]
    }

    /// Surface form (the substring of the original input).
    #[inline]
    pub fn surface(&self) -> &str {
        self.slice(&self.surface)
    }

    /// Dictionary form (lemma).
    #[inline]
    pub fn dictionary_form(&self) -> &str {
        self.slice(&self.dictionary_form)
    }

    /// Normalized form.
    #[inline]
    pub fn normalized_form(&self) -> &str {
        self.slice(&self.normalized_form)
    }

    /// Reading form (typically katakana furigana).
    #[inline]
    pub fn reading_form(&self) -> &str {
        self.slice(&self.reading_form)
    }

    /// Part-of-speech components, as borrowed slices into the arena.
    #[inline]
    pub fn part_of_speech(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.pos.iter().map(move |r| self.slice(r))
    }

    /// Numeric POS handle (matches [`crate::analysis::morpheme::Morpheme::part_of_speech_id`]).
    #[inline]
    pub fn part_of_speech_id(&self) -> u16 {
        self.pos_id
    }

    /// True if this morpheme is OOV (out-of-vocabulary).
    #[inline]
    pub fn is_oov(&self) -> bool {
        self.is_oov
    }

    /// Raw word ID.
    #[inline]
    pub fn word_id(&self) -> u32 {
        self.word_id
    }

    /// Begin byte offset in the original text.
    #[inline]
    pub fn begin(&self) -> usize {
        self.begin_bytes
    }

    /// End byte offset in the original text.
    #[inline]
    pub fn end(&self) -> usize {
        self.end_bytes
    }

    /// Begin codepoint offset in the original text.
    #[inline]
    pub fn begin_c(&self) -> usize {
        self.begin_chars
    }

    /// End codepoint offset in the original text.
    #[inline]
    pub fn end_c(&self) -> usize {
        self.end_chars
    }
}
