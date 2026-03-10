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

use nom::number::complete::{le_i32, le_u32};

use crate::dic::read::utf16_string::utf16_string;
use crate::dic::subset::InfoSubset;
use crate::dic::word_info::layout;
use crate::dic::word_info::{WordInfoFixedData, WordInfoRawData};
use crate::error::SudachiResult;

pub struct WordInfoParser {
    info: WordInfoRawData,
}

impl Default for WordInfoParser {
    #[inline]
    fn default() -> Self {
        Self::subset(InfoSubset::all())
    }
}

impl WordInfoParser {
    #[inline]
    pub fn subset(_flds: InfoSubset) -> WordInfoParser {
        Self {
            info: Default::default(),
        }
    }

    #[inline]
    pub fn embedded_c_unit_split_length(&self) -> usize {
        self.info.c_unit_split_length as usize
    }

    #[inline]
    pub fn embedded_b_unit_split_length(&self) -> usize {
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
        self.info.synonym_group_ids_length as usize
    }

    #[inline]
    pub fn parse(mut self, data: &[u8]) -> SudachiResult<WordInfoRawData> {
        let (data, _) = nom::bytes::complete::take(layout::PARAMS_SIZE)(data)?;
        let (data, fixed) = WordInfoFixedData::parse(data)?;
        self.info.pos_id = fixed.pos_id;
        self.info.headword_strptr = fixed.headword_strptr;
        self.info.reading_form_strptr = fixed.reading_form_strptr;
        self.info.normalized_form = fixed.normalized_form;
        self.info.dictionary_form = fixed.dictionary_form;
        self.info.index_form_length = fixed.index_form_length;
        self.info.c_unit_split_length = fixed.c_unit_split_length;
        self.info.b_unit_split_length = fixed.b_unit_split_length;
        self.info.a_unit_split_length = fixed.a_unit_split_length;
        self.info.word_structure_length = fixed.word_structure_length;
        self.info.synonym_group_ids_length = fixed.synonym_group_ids_length;
        self.info.user_data_flag = fixed.user_data_flag;

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

        if fixed.has_user_data() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dic::lexicon::strings::StringPointer;

    fn push_u32s(buf: &mut Vec<u8>, data: &[u32]) {
        for value in data {
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn push_i32s(buf: &mut Vec<u8>, data: &[i32]) {
        for value in data {
            buf.extend_from_slice(&value.to_le_bytes());
        }
    }

    fn push_utf16(buf: &mut Vec<u8>, data: &str) {
        let utf16: Vec<u16> = data.encode_utf16().collect();
        buf.extend_from_slice(&(utf16.len() as i16).to_le_bytes());
        for unit in utf16 {
            buf.extend_from_slice(&unit.to_le_bytes());
        }
    }

    #[test]
    fn parses_embedded_variable_length_fields() {
        let fixed = WordInfoFixedData {
            pos_id: 5,
            headword_strptr: StringPointer::unchecked(2, 4),
            reading_form_strptr: StringPointer::unchecked(3, 8),
            normalized_form: 11,
            dictionary_form: 12,
            index_form_length: 6,
            c_unit_split_length: 2,
            b_unit_split_length: 1,
            a_unit_split_length: 3,
            word_structure_length: 2,
            synonym_group_ids_length: 2,
            user_data_flag: 1,
        };

        let mut bytes = vec![0u8; layout::PARAMS_SIZE];
        fixed.write_to(&mut bytes).unwrap();
        push_u32s(&mut bytes, &[100, 101]);
        push_u32s(&mut bytes, &[200]);
        push_u32s(&mut bytes, &[300, 301, 302]);
        push_u32s(&mut bytes, &[400, 401]);
        push_i32s(&mut bytes, &[7, 8]);
        push_utf16(&mut bytes, "meta");

        let parsed = WordInfoParser::default().parse(&bytes).unwrap();
        assert_eq!(parsed.pos_id, fixed.pos_id);
        assert_eq!(parsed.c_unit_split, vec![100, 101]);
        assert_eq!(parsed.b_unit_split, vec![200]);
        assert_eq!(parsed.a_unit_split, vec![300, 301, 302]);
        assert_eq!(parsed.word_structure, vec![400, 401]);
        assert_eq!(parsed.synonym_group_ids, vec![7, 8]);
        assert_eq!(parsed.user_data, "meta");
    }

    #[test]
    fn expands_shared_split_arrays() {
        let fixed = WordInfoFixedData {
            pos_id: 9,
            headword_strptr: StringPointer::unchecked(1, 2),
            reading_form_strptr: StringPointer::unchecked(1, 4),
            normalized_form: 21,
            dictionary_form: 22,
            index_form_length: 3,
            c_unit_split_length: 2,
            b_unit_split_length: -1,
            a_unit_split_length: -1,
            word_structure_length: -1,
            synonym_group_ids_length: 1,
            user_data_flag: 0,
        };

        let mut bytes = vec![0u8; layout::PARAMS_SIZE];
        fixed.write_to(&mut bytes).unwrap();
        push_u32s(&mut bytes, &[10, 11]);
        push_i32s(&mut bytes, &[99]);

        let parsed = WordInfoParser::default().parse(&bytes).unwrap();
        assert_eq!(parsed.c_unit_split, vec![10, 11]);
        assert_eq!(parsed.b_unit_split, vec![10, 11]);
        assert_eq!(parsed.a_unit_split, vec![10, 11]);
        assert_eq!(parsed.word_structure, vec![10, 11]);
        assert_eq!(parsed.synonym_group_ids, vec![99]);
        assert!(parsed.user_data.is_empty());
    }
}
