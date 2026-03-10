/*
 *  Copyright (c) 2021-2026 Works Applications Co., Ltd.
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

use nom::number::complete::{le_i16, le_i32, le_i8, le_u32};

use crate::dic::lexicon::strings::StringPointer;
use crate::dic::read::error::SudachiNomResult;
use crate::dic::read::utf16_string::utf16_string;
use crate::dic::subset::InfoSubset;
use crate::dic::word_info::layout;
use crate::error::SudachiResult;

pub fn le_u32_string_pointer(input: &[u8]) -> SudachiNomResult<&[u8], StringPointer> {
    le_u32(input).map(|(rest, pointer)| (rest, StringPointer::decode(pointer)))
}

/// Parsed raw binary representation of a word info entry.
///
/// word id/ref fields are typed as u32 to avoid type conversion.
/// crate::dic::word_info::{WordInfoData, WordInfoRefData} will handle those types.
#[derive(Clone, Debug, Default)]
pub struct WordInfoRawData {
    pub pos_id: i16,

    pub headword_strptr: StringPointer,
    pub reading_form_strptr: StringPointer,
    pub normalized_form: u32,
    pub dictionary_form: u32,

    /// bytes length of the index form in utf-8.
    pub index_form_length: i16,
    pub c_unit_split_length: i8,
    pub b_unit_split_length: i8,
    pub a_unit_split_length: i8,
    pub word_structure_length: i8,
    pub synonym_group_ids_length: i8,
    pub user_data_flag: i8,

    pub c_unit_split: Vec<u32>,
    pub b_unit_split: Vec<u32>,
    pub a_unit_split: Vec<u32>,
    pub word_structure: Vec<u32>,
    pub synonym_group_ids: Vec<i32>,
    pub user_data: String,
}

pub struct WordInfoParser {
    info: WordInfoRawData,
    flds: InfoSubset,
}

/// Parse a single field of the WordInfo binary representation.
/// Six-parameter version accepts two funcitons:
/// true function which will actually parse the data, and
/// false function which should skip reading the data and just advance the parser position
///
/// Five-parameter version accepts only a single function and will unconditionally write
/// value from the binary form into the structure.
///
/// Six-parameter version should be used for "heavy" fields which require memory allocation
/// and five-parameter version should be used for "light" fields.
macro_rules! parse_field {
    ($root: expr, $data: ident, $name:tt, $field:expr, $tfn:expr, $ffn:expr) => {
        if $root.flds.is_empty() {
            return Ok($root.info);
        }
        #[allow(unused)]
        let $data = if $root.flds.contains($field) {
            let (next, res) = $tfn($data)?;
            $root.info.$name = res;
            $root.flds -= $field;
            next
        } else {
            let (next, _) = $ffn($data)?;
            next
        };
    };
    ($root: expr, $data: ident, $name:tt, $field:expr, $tfn:tt) => {
        if $root.flds.is_empty() {
            return Ok($root.info);
        }
        $root.flds -= $field;
        #[allow(unused)]
        let $data = {
            let (next, res) = $tfn($data)?;
            $root.info.$name = res;
            next
        };
    };
}

impl Default for WordInfoParser {
    #[inline]
    fn default() -> Self {
        Self::subset(InfoSubset::all())
    }
}

impl WordInfoParser {
    #[inline]
    pub fn subset(flds: InfoSubset) -> WordInfoParser {
        Self {
            info: Default::default(),
            flds,
        }
    }

    #[inline]
    pub fn embedded_c_unit_split_length(&self) -> usize {
        // length of c unit split is non-negative
        self.info.c_unit_split_length as usize
    }

    #[inline]
    pub fn embedded_b_unit_split_length(&self) -> usize {
        // when b/a unit split or word structure shares same array to the upper split, it is skipped and unit_split_length is set to -1.
        layout::embedded_len(self.info.b_unit_split_length)
    }

    #[inline]
    pub fn embedded_a_unit_split_length(&self) -> usize {
        layout::embedded_len(self.info.a_unit_split_length)
    }

    #[inline]
    pub fn embedded_word_structure_length(&self) -> usize {
        layout::embedded_len(self.info.word_structure_length)
    }

    #[inline]
    pub fn embedded_synonym_group_ids_length(&self) -> usize {
        // length of synonym group ids is non-negative
        self.info.synonym_group_ids_length as usize
    }

    #[inline]
    pub fn has_user_data(&self) -> bool {
        self.info.user_data_flag == 1
    }

    #[inline]
    pub fn parse(mut self, data: &[u8]) -> SudachiResult<WordInfoRawData> {
        // skip the parameters part (i16 * 3)
        let (data, _) = nom::bytes::complete::take(6usize)(data)?;
        parse_field!(self, data, pos_id, InfoSubset::POS_ID, le_i16);

        parse_field!(
            self,
            data,
            headword_strptr,
            InfoSubset::HEADWORD,
            le_u32_string_pointer
        );
        parse_field!(
            self,
            data,
            reading_form_strptr,
            InfoSubset::READING_FORM,
            le_u32_string_pointer
        );
        parse_field!(
            self,
            data,
            normalized_form,
            InfoSubset::NORMALIZED_FORM,
            le_u32
        );
        parse_field!(
            self,
            data,
            dictionary_form,
            InfoSubset::DICTIONARY_FORM,
            le_u32
        );

        parse_field!(
            self,
            data,
            index_form_length,
            InfoSubset::INDEX_FORM_LENGTH,
            le_i16
        );
        // Split/synonym lengths and user-data presence are metadata needed to advance the
        // parser, so read them unconditionally and apply subset filtering later.
        let (data, c_unit_split_length) = le_i8(data)?;
        self.info.c_unit_split_length = c_unit_split_length;
        let (data, b_unit_split_length) = le_i8(data)?;
        self.info.b_unit_split_length = b_unit_split_length;
        let (data, a_unit_split_length) = le_i8(data)?;
        self.info.a_unit_split_length = a_unit_split_length;
        let (data, word_structure_length) = le_i8(data)?;
        self.info.word_structure_length = word_structure_length;
        let (data, synonym_group_ids_length) = le_i8(data)?;
        self.info.synonym_group_ids_length = synonym_group_ids_length;
        let (data, user_data_flag) = le_i8(data)?;
        self.info.user_data_flag = user_data_flag;

        let (data, c_unit_split) =
            nom::multi::count(le_u32, self.embedded_c_unit_split_length())(data)?;
        self.info.c_unit_split = c_unit_split;
        let (data, b_unit_split) =
            nom::multi::count(le_u32, self.embedded_b_unit_split_length())(data)?;
        self.info.b_unit_split = b_unit_split;
        let (data, a_unit_split) =
            nom::multi::count(le_u32, self.embedded_a_unit_split_length())(data)?;
        self.info.a_unit_split = a_unit_split;
        let (data, word_structure) =
            nom::multi::count(le_u32, self.embedded_word_structure_length())(data)?;
        self.info.word_structure = word_structure;
        let (data, synonym_group_ids) =
            nom::multi::count(le_i32, self.embedded_synonym_group_ids_length())(data)?;
        self.info.synonym_group_ids = synonym_group_ids;

        if self.has_user_data() {
            let (data, user_data) = utf16_string(data)?;
            self.info.user_data = user_data;
            let _ = data;
        }

        if self.info.b_unit_split_length < 0 {
            self.info.b_unit_split = self.info.c_unit_split.clone();
        }
        if self.info.a_unit_split_length < 0 {
            self.info.a_unit_split = self.info.b_unit_split.clone();
        }
        if self.info.word_structure_length < 0 {
            self.info.word_structure = self.info.a_unit_split.clone();
        }

        Ok(self.info)
    }
}
