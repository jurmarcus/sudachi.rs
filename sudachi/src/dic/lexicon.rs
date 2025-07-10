/*
 * Copyright (c) 2021-2025 Works Applications Co., Ltd.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::cmp;

use self::trie::Trie;
use self::word_id_table::WordIdTable;
use self::word_infos::WordInfos;
use self::word_params::WordParams;
use crate::analysis::stateful_tokenizer::StatefulTokenizer;
use crate::analysis::stateless_tokenizer::DictionaryAccess;
use crate::dic::binary_loader::BinaryLexicon;
use crate::dic::lexicon::strings::CompactedStrings;
use crate::dic::subset::InfoSubset;
use crate::dic::word_id::{EntryId, WordId};
use crate::dic::word_info::WordInfoData;
use crate::prelude::*;

pub mod strings;
pub mod trie;
pub mod word_id_table;
pub mod word_infos;
pub mod word_params;

/// The first 4 bits of word_id are used to indicate that from which lexicon
/// the word comes, thus we can only hold 15 lexicons in the same time.
/// 16th is reserved for marking OOVs.
pub const MAX_DICTIONARIES: usize = 15;

/// Dictionary lexicon
///
/// Contains trie, word_id, word_param, word_info
pub struct Lexicon<'a> {
    lex_id: u8,

    trie: Trie<'a>,
    word_id_table: WordIdTable<'a>,
    word_params: WordParams<'a>,
    word_infos: WordInfos<'a>,
    strings: CompactedStrings<'a>,
}

/// Result of the Lexicon lookup
#[derive(Eq, PartialEq, Debug)]
pub struct LexiconEntry {
    /// Id of the returned word
    pub word_id: WordId,
    /// Byte index of the word end
    pub end: usize,
}

impl LexiconEntry {
    pub fn new(word_id: WordId, end: usize) -> LexiconEntry {
        LexiconEntry { word_id, end }
    }
}

impl<'a> Lexicon<'a> {
    const USER_DICT_COST_PER_MORPH: i32 = -20;

    pub fn from_binary(binary_lexicon: BinaryLexicon<'a>) -> SudachiResult<Self> {
        Ok(Self {
            trie: binary_lexicon.trie,
            word_id_table: binary_lexicon.word_id_table,
            word_params: binary_lexicon.word_params,
            word_infos: binary_lexicon.word_infos,
            strings: binary_lexicon.strings,
            lex_id: u8::MAX,
        })
    }

    /// Assign lexicon id to the current Lexicon
    pub fn set_dic_id(&mut self, id: u8) {
        assert!(id < MAX_DICTIONARIES as u8);
        self.lex_id = id
    }

    #[inline]
    fn word_id(&self, raw_id: u32) -> WordId {
        WordId::new(self.lex_id, raw_id)
    }

    /// Returns an iterator of word_id and end of words that matches given input
    #[inline]
    pub fn lookup(
        &'a self,
        input: &'a [u8],
        offset: usize,
    ) -> impl Iterator<Item = LexiconEntry> + 'a {
        debug_assert!(self.lex_id < MAX_DICTIONARIES as u8);
        self.trie
            .common_prefix_iterator(input, offset)
            .flat_map(move |e| {
                self.word_id_table
                    .entries(e.value as usize)
                    .map(move |eid| LexiconEntry::new(self.word_id(eid.as_raw()), e.end))
            })
    }

    /// Returns WordInfo for given word_id
    ///
    /// WordInfo will contain only fields included in InfoSubset
    pub fn get_word_info(
        &self,
        entry_id: EntryId,
        subset: InfoSubset,
    ) -> SudachiResult<WordInfoData> {
        self.word_infos.get_word_info(entry_id, subset)
    }

    /// Returns word_param for given word_id.
    /// Params are (left_id, right_id, cost).
    #[inline]
    pub fn get_word_param(&self, entry_id: EntryId) -> (i16, i16, i16) {
        let params = self.word_params.get_params(entry_id);
        (params.left_id(), params.right_id(), params.cost())
    }

    /// update word_param cost based on current tokenizer
    pub fn update_cost<D: DictionaryAccess>(&mut self, dict: &D) -> SudachiResult<()> {
        let mut tok = StatefulTokenizer::create(dict, false, Mode::C);
        let mut ms = MorphemeList::empty(dict);

        for entry_id in self.word_id_table.all_entries() {
            if self.word_params.get_cost(entry_id) != i16::MIN {
                continue;
            }
            let wi = self.get_word_info(entry_id, InfoSubset::HEADWORD)?;
            tok.reset().push_str(wi.surface());
            tok.do_tokenize()?;
            ms.collect_results(&mut tok)?;
            let internal_cost = ms.get_internal_cost();
            let cost = internal_cost + Lexicon::USER_DICT_COST_PER_MORPH * ms.len() as i32;
            let cost = cmp::min(cost, i16::MAX as i32);
            let cost = cmp::max(cost, i16::MIN as i32);
            self.word_params.set_cost(entry_id, cost as i16);
        }

        Ok(())
    }
}
