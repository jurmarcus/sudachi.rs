/*
 *  Copyright (c) 2021-2024 Works Applications Co., Ltd.
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 *   Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 */

use crate::analysis::node::{LatticeNode, PathCost, ResultNode};
use crate::analysis::owned_morpheme::OwnedMorpheme;
use crate::analysis::stateless_tokenizer::DictionaryAccess;
use crate::dic::lexicon::word_infos::WordInfo;
use crate::dic::word_id::WordId;
use crate::input_text::InputTextIndex;
use crate::prelude::*;
use std::cell::Ref;
use std::ops::Range;

/// A morpheme (basic semantic unit of language)
pub struct Morpheme<'a, T> {
    list: &'a MorphemeList<T>,
    index: usize,
}

impl<T: DictionaryAccess> Morpheme<'_, T> {
    /// Returns the part of speech
    #[inline]
    pub fn part_of_speech(&self) -> &[String] {
        self.list
            .dict()
            .grammar()
            .pos_components(self.part_of_speech_id())
    }
}

impl<T: DictionaryAccess + Clone> Morpheme<'_, T> {
    /// Returns new morpheme list splitting the morpheme with given mode.
    #[deprecated(note = "use split_into", since = "0.6.1")]
    pub fn split(&self, mode: Mode) -> SudachiResult<MorphemeList<T>> {
        #[allow(deprecated)]
        self.list.split(mode, self.index)
    }
}

impl<'a, T: DictionaryAccess> Morpheme<'a, T> {
    pub(crate) fn for_list(list: &'a MorphemeList<T>, index: usize) -> Self {
        Morpheme { list, index }
    }

    #[inline]
    pub(crate) fn node(&self) -> &ResultNode {
        self.list.node(self.index)
    }

    /// Returns the begin index in bytes of the morpheme in the original text
    #[inline]
    pub fn begin(&self) -> usize {
        self.list.input().to_orig_byte_idx(self.node().begin())
    }

    /// Returns the end index in bytes of the morpheme in the original text
    #[inline]
    pub fn end(&self) -> usize {
        self.list.input().to_orig_byte_idx(self.node().end())
    }

    /// Returns the codepoint offset of the morpheme begin in the original text
    #[inline]
    pub fn begin_c(&self) -> usize {
        self.list.input().to_orig_char_idx(self.node().begin())
    }

    /// Returns the codepoint offset of the morpheme begin in the original text
    #[inline]
    pub fn end_c(&self) -> usize {
        self.list.input().to_orig_char_idx(self.node().end())
    }

    /// Returns a substring of the original text which corresponds to the morpheme
    #[inline]
    pub fn surface(&self) -> Ref<str> {
        let inp = self.list.input();
        Ref::map(inp, |i| i.orig_slice(self.node().bytes_range()))
    }

    #[inline]
    pub fn part_of_speech_id(&self) -> u16 {
        self.node().word_info().pos_id()
    }

    /// Returns the dictionary form of morpheme
    ///
    /// "Dictionary form" means a word's lemma and "終止形" in Japanese.
    #[inline]
    pub fn dictionary_form(&self) -> &str {
        self.get_word_info().dictionary_form()
    }

    /// Returns the normalized form of morpheme
    ///
    /// This method returns the form normalizing inconsistent spellings and inflected forms
    #[inline]
    pub fn normalized_form(&self) -> &str {
        self.get_word_info().normalized_form()
    }

    /// Returns the reading form of morpheme.
    ///
    /// Returns Japanese syllabaries 'フリガナ' in katakana.
    #[inline]
    pub fn reading_form(&self) -> &str {
        self.get_word_info().reading_form()
    }

    /// Returns if this morpheme is out of vocabulary
    #[inline]
    pub fn is_oov(&self) -> bool {
        self.word_id().is_oov()
    }

    /// Returns the word id of morpheme
    #[inline]
    pub fn word_id(&self) -> WordId {
        self.node().word_id()
    }

    /// Returns the dictionary id where the morpheme belongs
    ///
    /// Returns -1 if the morpheme is oov
    #[inline]
    pub fn dictionary_id(&self) -> i32 {
        let wid = self.word_id();
        if wid.is_oov() {
            -1
        } else {
            wid.dic() as i32
        }
    }

    #[inline]
    pub fn synonym_group_ids(&self) -> &[u32] {
        self.get_word_info().synonym_group_ids()
    }

    #[inline]
    pub fn get_word_info(&self) -> &WordInfo {
        self.node().word_info()
    }

    /// Returns the index of this morpheme
    #[inline]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Splits morpheme and writes sub-morphemes into the provided list.
    /// The resulting list is _not_ cleared before that.
    /// Returns true if split has produced any elements.
    pub fn split_into(&self, mode: Mode, out: &mut MorphemeList<T>) -> SudachiResult<bool> {
        self.list.split_into(mode, self.index, out)
    }

    /// Returns total cost from the beginning of the path
    pub fn total_cost(&self) -> i32 {
        return self.node().total_cost();
    }

    /// Snapshot this morpheme into an [`OwnedMorpheme`] that escapes the
    /// borrow on the parent [`MorphemeList`] / dictionary in a single
    /// allocation.
    ///
    /// All string fields (surface, dictionary_form, normalized_form,
    /// reading_form, and each POS component) are packed into one
    /// `Box<str>` arena with byte-range indices. This collapses the naive
    /// 5+ per-field `to_owned()` pattern into one allocation per morpheme.
    ///
    /// Use when you need owned data — building search index tokens,
    /// caching morphemes, sending across threads, surviving a tokenizer
    /// `reset()`. Don't use when borrowed access via [`Morpheme`] suffices.
    pub fn into_owned(&self) -> OwnedMorpheme {
        let surface = self.surface();
        let dict_form = self.dictionary_form();
        let norm_form = self.normalized_form();
        let reading = self.reading_form();
        let pos = self.part_of_speech();

        let pos_total: usize = pos.iter().map(String::len).sum();
        let total =
            surface.len() + dict_form.len() + norm_form.len() + reading.len() + pos_total;

        let mut arena = String::with_capacity(total);

        let push = |arena: &mut String, s: &str| -> Range<u32> {
            let start = arena.len() as u32;
            arena.push_str(s);
            start..arena.len() as u32
        };

        let surface_r = push(&mut arena, &surface);
        let dict_r = push(&mut arena, dict_form);
        let norm_r = push(&mut arena, norm_form);
        let reading_r = push(&mut arena, reading);
        let pos_ranges: Vec<Range<u32>> = pos.iter().map(|p| push(&mut arena, p)).collect();

        let wid = self.word_id();
        OwnedMorpheme {
            arena: arena.into_boxed_str(),
            surface: surface_r,
            dictionary_form: dict_r,
            normalized_form: norm_r,
            reading_form: reading_r,
            pos: pos_ranges,
            pos_id: self.part_of_speech_id(),
            word_id: wid.as_raw(),
            is_oov: wid.is_oov(),
            begin_bytes: self.begin(),
            end_bytes: self.end(),
            begin_chars: self.begin_c(),
            end_chars: self.end_c(),
        }
    }
}

impl<T: DictionaryAccess> std::fmt::Debug for Morpheme<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Morpheme")
            .field("surface", &self.surface())
            .field("pos", &self.part_of_speech())
            .field("normalized_form", &self.normalized_form())
            .field("reading_form", &self.reading_form())
            .field("dictionary_form", &self.dictionary_form())
            .finish()
    }
}
