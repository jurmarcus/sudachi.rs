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
