/*
 * Copyright (c) 2026 Works Applications Co., Ltd.
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

use std::io::Write;

use nom::number::complete::{le_i16, le_i8, le_u32};

use crate::dic::lexicon::strings::StringPointer;
use crate::dic::read::error::SudachiNomResult;
use crate::dic::word_info::layout;
use crate::dic::word_info::{write_i32_slice, write_u32_slice, write_utf16_string};

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

pub(crate) struct WordInfoVariableData<'a> {
    pub c_unit_split: &'a [u32],
    pub b_unit_split: &'a [u32],
    pub a_unit_split: &'a [u32],
    pub word_structure: &'a [u32],
    pub synonym_group_ids: &'a [i32],
    pub user_data: &'a str,
}

impl<'a> WordInfoVariableData<'a> {
    pub fn write_to<W: Write>(
        &self,
        w: &mut W,
        fixed: &WordInfoFixedData,
    ) -> std::io::Result<usize> {
        let mut size = 0;
        size += write_u32_slice(w, self.c_unit_split)?;
        if fixed.b_unit_split_length > 0 {
            size += write_u32_slice(w, self.b_unit_split)?;
        }
        if fixed.a_unit_split_length > 0 {
            size += write_u32_slice(w, self.a_unit_split)?;
        }
        if fixed.word_structure_length > 0 {
            size += write_u32_slice(w, self.word_structure)?;
        }
        size += write_i32_slice(w, self.synonym_group_ids)?;
        if fixed.has_user_data() {
            size += write_utf16_string(w, self.user_data)?;
        }
        Ok(size)
    }
}

#[derive(Clone, Debug, Default)]
pub struct WordInfoFixedData {
    pub pos_id: i16,
    pub headword_strptr: StringPointer,
    pub reading_form_strptr: StringPointer,
    pub normalized_form: u32,
    pub dictionary_form: u32,
    pub index_form_length: i16,
    pub c_unit_split_length: i8,
    pub b_unit_split_length: i8,
    pub a_unit_split_length: i8,
    pub word_structure_length: i8,
    pub synonym_group_ids_length: i8,
    pub user_data_flag: i8,
}

impl WordInfoFixedData {
    pub fn parse(input: &[u8]) -> SudachiNomResult<&[u8], Self> {
        let (input, pos_id) = le_i16(input)?;
        let (input, headword_strptr) =
            le_u32(input).map(|(rest, pointer)| (rest, StringPointer::decode(pointer)))?;
        let (input, reading_form_strptr) =
            le_u32(input).map(|(rest, pointer)| (rest, StringPointer::decode(pointer)))?;
        let (input, normalized_form) = le_u32(input)?;
        let (input, dictionary_form) = le_u32(input)?;
        let (input, index_form_length) = le_i16(input)?;
        let (input, c_unit_split_length) = le_i8(input)?;
        let (input, b_unit_split_length) = le_i8(input)?;
        let (input, a_unit_split_length) = le_i8(input)?;
        let (input, word_structure_length) = le_i8(input)?;
        let (input, synonym_group_ids_length) = le_i8(input)?;
        let (input, user_data_flag) = le_i8(input)?;
        Ok((
            input,
            Self {
                pos_id,
                headword_strptr,
                reading_form_strptr,
                normalized_form,
                dictionary_form,
                index_form_length,
                c_unit_split_length,
                b_unit_split_length,
                a_unit_split_length,
                word_structure_length,
                synonym_group_ids_length,
                user_data_flag,
            },
        ))
    }

    pub fn from_entry_bytes(data: &[u8]) -> Option<Self> {
        let fixed = data.get(..layout::FIXED_PART_SIZE)?;
        Some(Self {
            pos_id: i16::from_le_bytes([
                fixed[layout::PARAMS_SIZE],
                fixed[layout::PARAMS_SIZE + 1],
            ]),
            headword_strptr: StringPointer::decode(u32::from_le_bytes([
                fixed[8], fixed[9], fixed[10], fixed[11],
            ])),
            reading_form_strptr: StringPointer::decode(u32::from_le_bytes([
                fixed[12], fixed[13], fixed[14], fixed[15],
            ])),
            normalized_form: u32::from_le_bytes([fixed[16], fixed[17], fixed[18], fixed[19]]),
            dictionary_form: u32::from_le_bytes([fixed[20], fixed[21], fixed[22], fixed[23]]),
            index_form_length: i16::from_le_bytes([fixed[24], fixed[25]]),
            c_unit_split_length: fixed[layout::OFFSET_C_UNIT_SPLIT_LENGTH] as i8,
            b_unit_split_length: fixed[layout::OFFSET_B_UNIT_SPLIT_LENGTH] as i8,
            a_unit_split_length: fixed[layout::OFFSET_A_UNIT_SPLIT_LENGTH] as i8,
            word_structure_length: fixed[layout::OFFSET_WORD_STRUCTURE_LENGTH] as i8,
            synonym_group_ids_length: fixed[layout::OFFSET_SYNONYM_GROUP_IDS_LENGTH] as i8,
            user_data_flag: fixed[layout::OFFSET_USER_DATA_FLAG] as i8,
        })
    }

    pub fn has_user_data(&self) -> bool {
        self.user_data_flag == 1
    }

    pub fn write_to<W: Write>(&self, w: &mut W) -> std::io::Result<usize> {
        w.write_all(&self.pos_id.to_le_bytes())?;
        w.write_all(&self.headword_strptr.encode().to_le_bytes())?;
        w.write_all(&self.reading_form_strptr.encode().to_le_bytes())?;
        w.write_all(&self.normalized_form.to_le_bytes())?;
        w.write_all(&self.dictionary_form.to_le_bytes())?;
        w.write_all(&self.index_form_length.to_le_bytes())?;
        w.write_all(&self.c_unit_split_length.to_le_bytes())?;
        w.write_all(&self.b_unit_split_length.to_le_bytes())?;
        w.write_all(&self.a_unit_split_length.to_le_bytes())?;
        w.write_all(&self.word_structure_length.to_le_bytes())?;
        w.write_all(&self.synonym_group_ids_length.to_le_bytes())?;
        w.write_all(&self.user_data_flag.to_le_bytes())?;
        Ok(layout::WORD_INFO_FIXED_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fixed() -> WordInfoFixedData {
        WordInfoFixedData {
            pos_id: 123,
            headword_strptr: StringPointer::unchecked(3, 8),
            reading_form_strptr: StringPointer::unchecked(5, 16),
            normalized_form: 42,
            dictionary_form: 84,
            index_form_length: 9,
            c_unit_split_length: 2,
            b_unit_split_length: -1,
            a_unit_split_length: 4,
            word_structure_length: -1,
            synonym_group_ids_length: 3,
            user_data_flag: 1,
        }
    }

    #[test]
    fn fixed_data_write_and_parse_round_trip() {
        let fixed = sample_fixed();
        let mut bytes = Vec::new();
        let written = fixed.write_to(&mut bytes).unwrap();
        assert_eq!(written, layout::WORD_INFO_FIXED_SIZE);

        let (rest, parsed) = WordInfoFixedData::parse(&bytes).unwrap();
        assert!(rest.is_empty());
        assert_eq!(parsed.pos_id, fixed.pos_id);
        assert_eq!(parsed.headword_strptr, fixed.headword_strptr);
        assert_eq!(parsed.reading_form_strptr, fixed.reading_form_strptr);
        assert_eq!(parsed.normalized_form, fixed.normalized_form);
        assert_eq!(parsed.dictionary_form, fixed.dictionary_form);
        assert_eq!(parsed.index_form_length, fixed.index_form_length);
        assert_eq!(parsed.c_unit_split_length, fixed.c_unit_split_length);
        assert_eq!(parsed.b_unit_split_length, fixed.b_unit_split_length);
        assert_eq!(parsed.a_unit_split_length, fixed.a_unit_split_length);
        assert_eq!(parsed.word_structure_length, fixed.word_structure_length);
        assert_eq!(
            parsed.synonym_group_ids_length,
            fixed.synonym_group_ids_length
        );
        assert_eq!(parsed.user_data_flag, fixed.user_data_flag);
    }

    #[test]
    fn fixed_data_reads_from_entry_bytes_after_params() {
        let fixed = sample_fixed();
        let mut entry = vec![0u8; layout::PARAMS_SIZE];
        fixed.write_to(&mut entry).unwrap();
        assert_eq!(entry.len(), layout::FIXED_PART_SIZE);

        let scanned = WordInfoFixedData::from_entry_bytes(&entry).unwrap();
        assert_eq!(scanned.pos_id, fixed.pos_id);
        assert_eq!(scanned.headword_strptr, fixed.headword_strptr);
        assert_eq!(scanned.reading_form_strptr, fixed.reading_form_strptr);
        assert_eq!(scanned.normalized_form, fixed.normalized_form);
        assert_eq!(scanned.dictionary_form, fixed.dictionary_form);
        assert_eq!(scanned.index_form_length, fixed.index_form_length);
        assert_eq!(scanned.c_unit_split_length, fixed.c_unit_split_length);
        assert_eq!(scanned.b_unit_split_length, fixed.b_unit_split_length);
        assert_eq!(scanned.a_unit_split_length, fixed.a_unit_split_length);
        assert_eq!(scanned.word_structure_length, fixed.word_structure_length);
        assert_eq!(
            scanned.synonym_group_ids_length,
            fixed.synonym_group_ids_length
        );
        assert_eq!(scanned.user_data_flag, fixed.user_data_flag);
    }
}
