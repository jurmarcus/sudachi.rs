/*
 * Copyright (c) 2021-2026 Works Applications Co., Ltd.
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

use crate::dic::subset::InfoSubset;
use crate::dic::word_id::EntryId;
use crate::dic::word_info::layout;
use crate::dic::word_info::{WordInfoParser, WordInfoRefData};
use crate::prelude::*;

pub struct WordInfos<'a> {
    bytes: &'a [u8],
}

impl<'a> WordInfos<'a> {
    pub const ENTRIES_INITIAL_OFFSET: usize = layout::ENTRY_INITIAL_OFFSET;
    pub const WORD_ID_ALIGNMENT_BITS: usize = layout::WORD_ID_ALIGNMENT_BITS;
    pub const WORD_INFO_OFFSET_ALIGNMENT: usize = layout::WORD_INFO_OFFSET_ALIGNMENT;

    pub fn from_bytes(bytes: &'a [u8]) -> WordInfos<'a> {
        WordInfos { bytes }
    }

    pub fn entry_id_to_offset(entry_id: EntryId) -> usize {
        (entry_id.as_raw() as usize) << Self::WORD_ID_ALIGNMENT_BITS
    }

    pub fn entry_ids_in_order(&self, num_total_entries: u32) -> Option<Vec<EntryId>> {
        let mut result = Vec::with_capacity(num_total_entries as usize);
        let mut offset = Self::ENTRIES_INITIAL_OFFSET;

        while result.len() < num_total_entries as usize {
            if offset % Self::WORD_INFO_OFFSET_ALIGNMENT != 0 {
                return None;
            }

            let entry_id = EntryId::new((offset >> Self::WORD_ID_ALIGNMENT_BITS) as u32);
            result.push(entry_id);

            let size = self.entry_size_at(offset)?;
            offset = offset.checked_add(size)?;
        }

        Some(result)
    }

    fn entry_size_at(&self, offset: usize) -> Option<usize> {
        let fixed = self.bytes.get(offset..offset + layout::FIXED_PART_SIZE)?;
        let c_len = fixed[layout::OFFSET_C_UNIT_SPLIT_LENGTH] as i8;
        let b_len = fixed[layout::OFFSET_B_UNIT_SPLIT_LENGTH] as i8;
        let a_len = fixed[layout::OFFSET_A_UNIT_SPLIT_LENGTH] as i8;
        let ws_len = fixed[layout::OFFSET_WORD_STRUCTURE_LENGTH] as i8;
        let syn_len = fixed[layout::OFFSET_SYNONYM_GROUP_IDS_LENGTH] as i8;
        let user_data_flag = fixed[layout::OFFSET_USER_DATA_FLAG] as i8;

        if !layout::is_valid_user_data_flag(user_data_flag) {
            return None;
        }

        let mut user_data_units = None;
        if user_data_flag == 1 {
            let user_data_offset = offset
                + layout::unaligned_size_from_lengths(c_len, b_len, a_len, ws_len, syn_len, None)?;
            let user_len_bytes = self.bytes.get(user_data_offset..user_data_offset + 2)?;
            let user_len = i16::from_le_bytes([user_len_bytes[0], user_len_bytes[1]]);
            user_data_units = Some(user_len);
        }

        let aligned =
            layout::size_from_lengths(c_len, b_len, a_len, ws_len, syn_len, user_data_units)?;
        self.bytes.get(offset..offset + aligned)?;
        Some(aligned)
    }

    pub fn get_word_info(
        &self,
        entry_id: EntryId,
        subset: InfoSubset,
    ) -> SudachiResult<WordInfoRefData> {
        let offset = Self::entry_id_to_offset(entry_id);
        let parser = WordInfoParser::subset(subset);
        let word_info = parser.parse(&self.bytes[offset..])?;
        Ok(WordInfoRefData::from_raw(word_info))
    }
}
