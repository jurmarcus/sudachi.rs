/*
 * Copyright (c) 2025 Works Applications Co., Ltd.
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

use crate::dic::lexicon::strings::StringPointer;
use crate::dic::subset::InfoSubset;
use crate::dic::word_id::{DictId,WordId, WordRef};

/// Raw word info contains word references
#[derive(Copy, Clone, Debug)]
pub enum WordIdOrRef {
    Id(WordId),
    Ref(WordRef),
}

impl Default for WordIdOrRef {
    fn default() -> Self {
        WordIdOrRef::Id(WordId::default())
    }
}

impl WordIdOrRef {
    pub fn resolve(&self, dict_id: DictId) -> Self {
        match self {
            WordIdOrRef::Id(_) => {
                // Already resolved
                *self
            }
            WordIdOrRef::Ref(word_ref) => {
                WordIdOrRef::Id(word_ref.resolve(dict_id))
            }
        }
    }
}

/// Parsed raw binary representation of a word info entry.
#[derive(Clone, Debug, Default)]
pub struct WordInfoData {
    pub pos_id: i16,

    pub headword_strptr: StringPointer,
    pub reading_form_strptr: StringPointer,
    pub normalized_form: WordIdOrRef,
    pub dictionary_form: WordIdOrRef,

    pub index_form_length: i16,
    pub c_unit_split_length: i8,
    pub b_unit_split_length: i8,
    pub a_unit_split_length: i8,
    pub word_structure_length: i8,
    pub synonym_group_ids_length: i8,
    pub user_data_flag: i8,

    pub c_unit_split: Vec<WordIdOrRef>,
    pub b_unit_split: Vec<WordIdOrRef>,
    pub a_unit_split: Vec<WordIdOrRef>,
    pub word_structure: Vec<WordIdOrRef>,
    pub synonym_group_ids: Vec<i32>,
    pub user_data: String,
}

impl WordInfoData {
    /// resolve word references in the WordInfoData
    pub fn resolve_word_ref(&mut self, dict_id: DictId, subset: InfoSubset) {
        if subset.contains(InfoSubset::NORMALIZED_FORM) {
            self.normalized_form = self.normalized_form.resolve(dict_id);
        }
        if subset.contains(InfoSubset::DICTIONARY_FORM) {
            self.dictionary_form = self.dictionary_form.resolve(dict_id);
        }

        if subset.contains(InfoSubset::SPLIT_C) {
            Self::resolve_word_refs(&mut self.c_unit_split, dict_id);
        }
        if subset.contains(InfoSubset::SPLIT_B) {
            Self::resolve_word_refs(&mut self.b_unit_split, dict_id);
        }
        if subset.contains(InfoSubset::SPLIT_A) {
            Self::resolve_word_refs(&mut self.a_unit_split, dict_id);
        }
        if subset.contains(InfoSubset::WORD_STRUCTURE) {
            Self::resolve_word_refs(&mut self.word_structure, dict_id);
        }
    }

    fn resolve_word_refs(refs: &mut Vec<WordIdOrRef>, dict_id: DictId) {
        for id_or_ref in refs.iter_mut() {
            *id_or_ref = id_or_ref.resolve(dict_id);
        }
    }
}

/// WordInfo API.
///
/// Internal data is not accessible by default, but can be extracted as
/// `let data: WordInfoData = info.into()`.
/// Note: this will consume WordInfo.
#[derive(Clone, Default)]
#[repr(transparent)]
pub struct WordInfo {
    data: WordInfoData,
}

impl WordInfo {
    pub fn headword(&self) -> &str {
        &self.data.headword
    }

    pub fn index_form_length(&self) -> usize {
        self.data.index_form_length as usize
    }

    pub fn pos_id(&self) -> u16 {
        self.data.pos_id
    }

    pub fn normalized_form(&self) -> &str {
        if self.data.normalized_form.is_empty() {
            self.headword()
        } else {
            &self.data.normalized_form
        }
    }

    pub fn dictionary_form_word_id(&self) -> i32 {
        self.data.dictionary_form_word_id
    }

    pub fn dictionary_form(&self) -> &str {
        if self.data.dictionary_form.is_empty() {
            self.headword()
        } else {
            &self.data.dictionary_form
        }
    }

    pub fn reading_form(&self) -> &str {
        if self.data.reading_form.is_empty() {
            self.headword()
        } else {
            &self.data.reading_form
        }
    }

    pub fn a_unit_split(&self) -> &[WordId] {
        &self.data.a_unit_split
    }

    pub fn b_unit_split(&self) -> &[WordId] {
        &self.data.b_unit_split
    }

    pub fn word_structure(&self) -> &[WordId] {
        &self.data.word_structure
    }

    pub fn synonym_group_ids(&self) -> &[u32] {
        &self.data.synonym_group_ids
    }

    pub fn borrow_data(&self) -> &WordInfoData {
        &self.data
    }
}

impl From<WordInfoData> for WordInfo {
    fn from(data: WordInfoData) -> Self {
        WordInfo { data }
    }
}

impl From<WordInfo> for WordInfoData {
    fn from(info: WordInfo) -> Self {
        info.data
    }
}
