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

use crate::dic::read::word_info::WordInfoParser;
use crate::dic::subset::InfoSubset;
use crate::dic::word_id::EntryId;
use crate::dic::word_info::WordInfoRefData;
use crate::prelude::*;

pub struct WordInfos<'a> {
    bytes: &'a [u8],
}

impl<'a> WordInfos<'a> {
    /// The byte size of Word entries in the Entries block are aligned to 8 bytes.
    /// WordId is a offset of the entry in the Entries block, w/o last 3 bits.
    pub const WORD_ID_ALIGNMENT_BITS: usize = 3;
    pub const WORD_INFO_OFFSET_ALIGNMENT: usize = 1 << Self::WORD_ID_ALIGNMENT_BITS;

    pub fn from_bytes(bytes: &'a [u8]) -> WordInfos<'a> {
        WordInfos { bytes }
    }

    pub fn entry_id_to_offset(entry_id: EntryId) -> usize {
        (entry_id.as_raw() as usize) << Self::WORD_ID_ALIGNMENT_BITS
    }

    /// Scans the Entries block and returns word entry ids in the original CSV row order.
    ///
    /// This is needed for line-number based split resolution. We cannot rely on the
    /// trie/word-pointer tables because non-indexed entries do not appear there.
    pub fn entry_ids_in_order(&self, num_total_entries: u32) -> Option<Vec<EntryId>> {
        let mut result = Vec::with_capacity(num_total_entries as usize);
        let mut offset = 0usize;

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
        let fixed = self.bytes.get(offset..offset + 32)?;
        let c_len = fixed[26] as i8;
        let b_len = fixed[27] as i8;
        let a_len = fixed[28] as i8;
        let ws_len = fixed[29] as i8;
        let syn_len = fixed[30] as i8;
        let user_data_flag = fixed[31] as i8;

        if c_len < 0 || syn_len < 0 || !(user_data_flag == 0 || user_data_flag == 1) {
            return None;
        }

        let mut size = 32usize;
        size += 4 * c_len as usize;
        size += 4 * std::cmp::max(0, b_len) as usize;
        size += 4 * std::cmp::max(0, a_len) as usize;
        size += 4 * std::cmp::max(0, ws_len) as usize;
        size += 4 * syn_len as usize;

        if user_data_flag == 1 {
            let user_len_bytes = self.bytes.get(offset + size..offset + size + 2)?;
            let user_len = i16::from_le_bytes([user_len_bytes[0], user_len_bytes[1]]);
            if user_len < 0 {
                return None;
            }
            size += 2 + user_len as usize * 2;
        }

        let aligned = (size + (Self::WORD_INFO_OFFSET_ALIGNMENT - 1))
            & !(Self::WORD_INFO_OFFSET_ALIGNMENT - 1);
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
