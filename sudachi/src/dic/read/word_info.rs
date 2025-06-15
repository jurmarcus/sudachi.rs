/*
 *  Copyright (c) 2021-2025 Works Applications Co., Ltd.
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

use crate::dic::read::utf16_string::{short_utf16_string, skip_short_utf16_string};
use crate::dic::read::word_id::le_u32_word_id;
use crate::dic::subset::InfoSubset;
use crate::dic::word_id::WordId;
use crate::error::SudachiResult;

/// Binary representation of a word info entry.
///
/// Vec and String can be empty.
#[derive(Clone, Debug, Default)]
pub struct RawWordInfoData {
    pub pos_id: i16,

    pub headword_strptr: u32,
    pub reading_form_strptr: u32,
    pub normalized_form_word_ref: WordId,
    pub dictionary_form_word_ref: WordId,

    pub index_form_length: i16,
    pub c_unit_split_length: i8,
    pub b_unit_split_length: i8,
    pub a_unit_split_length: i8,
    pub word_structure_length: i8,
    pub synonym_group_ids_length: i8,
    pub user_data_flag: i8,

    pub c_unit_split: Vec<WordId>,
    pub b_unit_split: Vec<WordId>,
    pub a_unit_split: Vec<WordId>,
    pub word_structure: Vec<WordId>,
    pub synonym_group_ids: Vec<i32>,
    pub user_data: String,
}

pub struct WordInfoParser {
    info: RawWordInfoData,
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
        // length of b/a unit split and word structure is -1 when it is same as the larger unit split
        std::cmp::max(0, self.info.b_unit_split_length) as usize
    }

    #[inline]
    pub fn embedded_a_unit_split_length(&self) -> usize {
        std::cmp::max(0, self.info.a_unit_split_length) as usize
    }

    #[inline]
    pub fn embedded_word_structure_length(&self) -> usize {
        std::cmp::max(0, self.info.word_structure_length) as usize
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
    pub fn parse(mut self, data: &[u8]) -> SudachiResult<RawWordInfoData> {
        // skip the parameters part (i16 * 3)
        let (data, _) = nom::bytes::complete::take(6usize)(data)?;
        parse_field!(self, data, pos_id, InfoSubset::POS_ID, le_i16);

        parse_field!(self, data, headword_strptr, InfoSubset::HEADWORD, le_u32);
        parse_field!(
            self,
            data,
            reading_form_strptr,
            InfoSubset::READING_FORM,
            le_u32
        );
        parse_field!(
            self,
            data,
            normalized_form_word_ref,
            InfoSubset::NORMALIZED_FORM,
            le_u32_word_id
        );
        parse_field!(
            self,
            data,
            dictionary_form_word_ref,
            InfoSubset::DICTIONARY_FORM,
            le_u32_word_id
        );

        parse_field!(
            self,
            data,
            index_form_length,
            InfoSubset::INDEX_FORM_LENGTH,
            le_i16
        );
        parse_field!(self, data, c_unit_split_length, InfoSubset::SPLIT_C, le_i8);
        parse_field!(self, data, b_unit_split_length, InfoSubset::SPLIT_B, le_i8);
        parse_field!(self, data, a_unit_split_length, InfoSubset::SPLIT_A, le_i8);
        parse_field!(
            self,
            data,
            word_structure_length,
            InfoSubset::WORD_STRUCTURE,
            le_i8
        );
        parse_field!(
            self,
            data,
            synonym_group_ids_length,
            InfoSubset::SYNONYM_GROUP_IDS,
            le_i8
        );
        parse_field!(self, data, user_data_flag, InfoSubset::USER_DATA, le_i8);

        parse_field!(
            self,
            data,
            c_unit_split,
            InfoSubset::SPLIT_C,
            nom::multi::count(le_u32_word_id, self.embedded_c_unit_split_length()),
            nom::bytes::complete::take(4 * self.embedded_c_unit_split_length())
        );
        parse_field!(
            self,
            data,
            b_unit_split,
            InfoSubset::SPLIT_B,
            nom::multi::count(le_u32_word_id, self.embedded_b_unit_split_length()),
            nom::bytes::complete::take(4 * self.embedded_b_unit_split_length())
        );
        parse_field!(
            self,
            data,
            a_unit_split,
            InfoSubset::SPLIT_A,
            nom::multi::count(le_u32_word_id, self.embedded_a_unit_split_length()),
            nom::bytes::complete::take(4 * self.embedded_a_unit_split_length())
        );
        parse_field!(
            self,
            data,
            word_structure,
            InfoSubset::WORD_STRUCTURE,
            nom::multi::count(le_u32_word_id, self.embedded_word_structure_length()),
            nom::bytes::complete::take(4 * self.embedded_word_structure_length())
        );

        parse_field!(
            self,
            data,
            synonym_group_ids,
            InfoSubset::SYNONYM_GROUP_IDS,
            nom::multi::count(le_i32, self.embedded_synonym_group_ids_length()),
            nom::bytes::complete::take(4 * self.embedded_synonym_group_ids_length())
        );

        // parse user data only if the flag is set
        if self.has_user_data() {
            parse_field!(
                self,
                data,
                user_data,
                InfoSubset::USER_DATA,
                short_utf16_string,
                skip_short_utf16_string
            );
        }

        Ok(self.info)
    }
}
