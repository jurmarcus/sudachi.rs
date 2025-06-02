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

use crate::util::cow_array::CowArray;

use super::word_infos::{word_id_to_offset};


/// A word paratemter, used in the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct  WordParameter(u64);

impl WordParameter {
    /// The left connection cost id.
    #[inline]
    pub fn left_id(self) -> i16 {
        (self.0 & 0xffff) as i16
    }

    /// The right connection cost id.
    #[inline]
    pub fn right_id(self) -> i16 {
        ((self.0 >> 16) & 0xffff) as i16
    }

    /// The cost of the word.
    #[inline]
    pub fn cost(self) -> i16 {
        ((self.0 >> 32) & 0xffff) as i16
    }

    /// Return a new parameters with the given cost.
    #[inline]
    pub fn with_cost(self, cost:i16) -> WordParameter {
        WordParameter((cost as u64) << 32 | (self.0 & 0xffffffff))
    }
}

pub struct WordParams<'a> {
    // word infos are aligned per 8 bytes (see WordInfos::WORD_INFO_OFFSET_ALIGNMENT).
    data: CowArray<'a, u64>,
}

impl<'a> WordParams<'a> {
    const PARAM_SIZE: usize = 3;
    const ELEMENT_SIZE: usize = 2 * Self::PARAM_SIZE;

    pub fn from_bytes(bytes: &'a [u8]) -> WordParams<'a> {
        Self {
            data: CowArray::from_bytes(bytes, 0, bytes.len()),
        }
    }
    
    #[inline]
    pub fn get_params(&self, word_id: u32) -> WordParameter {
        let offset = word_id_to_offset(word_id);
        // the first 8 bytes of the word info in the parameters
        WordParameter(self.data[offset])
    }

    #[inline]
    pub fn get_cost(&self, word_id: u32) -> i16 {
        let params = self.get_params(word_id);
        params.cost()
    }

    pub fn set_cost(&mut self, word_id: u32, cost: i16) {
        let offset = word_id_to_offset(word_id);
        let params = self.get_params(word_id);
        self.data.set(offset, params.with_cost(cost).0);
    }
}
