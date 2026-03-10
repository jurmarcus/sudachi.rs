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

pub const ENTRY_INITIAL_OFFSET: usize = 32;
pub const PARAMS_SIZE: usize = 6;
pub const FIXED_PART_SIZE: usize = 32;
pub const WORD_INFO_FIXED_SIZE: usize = FIXED_PART_SIZE - PARAMS_SIZE;
pub const WORD_ID_ALIGNMENT_BITS: usize = 3;
pub const WORD_INFO_OFFSET_ALIGNMENT: usize = 1 << WORD_ID_ALIGNMENT_BITS;

pub const OFFSET_C_UNIT_SPLIT_LENGTH: usize = 26;
pub const OFFSET_B_UNIT_SPLIT_LENGTH: usize = 27;
pub const OFFSET_A_UNIT_SPLIT_LENGTH: usize = 28;
pub const OFFSET_WORD_STRUCTURE_LENGTH: usize = 29;
pub const OFFSET_SYNONYM_GROUP_IDS_LENGTH: usize = 30;
pub const OFFSET_USER_DATA_FLAG: usize = 31;

#[inline]
pub const fn aligned_size(size: usize) -> usize {
    (size + (WORD_INFO_OFFSET_ALIGNMENT - 1)) & !(WORD_INFO_OFFSET_ALIGNMENT - 1)
}

#[inline]
pub fn embedded_len(len: i8) -> usize {
    std::cmp::max(0, len) as usize
}

#[inline]
pub fn is_valid_user_data_flag(flag: i8) -> bool {
    matches!(flag, 0 | 1)
}

pub fn size_from_lengths(
    c_len: i8,
    b_len: i8,
    a_len: i8,
    ws_len: i8,
    syn_len: i8,
    user_data_units: Option<i16>,
) -> Option<usize> {
    let size = unaligned_size_from_lengths(c_len, b_len, a_len, ws_len, syn_len, user_data_units)?;
    Some(aligned_size(size))
}

pub fn unaligned_size_from_lengths(
    c_len: i8,
    b_len: i8,
    a_len: i8,
    ws_len: i8,
    syn_len: i8,
    user_data_units: Option<i16>,
) -> Option<usize> {
    if c_len < 0 || syn_len < 0 {
        return None;
    }

    let mut size = FIXED_PART_SIZE;
    size = size.checked_add(4 * c_len as usize)?;
    size = size.checked_add(4 * embedded_len(b_len))?;
    size = size.checked_add(4 * embedded_len(a_len))?;
    size = size.checked_add(4 * embedded_len(ws_len))?;
    size = size.checked_add(4 * syn_len as usize)?;

    if let Some(units) = user_data_units {
        if units < 0 {
            return None;
        }
        // put length as i16
        size = size.checked_add(2 + units as usize * 2)?;
    }

    Some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_size_without_user_data() {
        let size = size_from_lengths(2, -1, -1, 3, 1, None).unwrap();
        assert_eq!(size, 56);
    }

    #[test]
    fn computes_size_with_user_data() {
        let size = size_from_lengths(1, 2, 3, -1, 2, Some(4)).unwrap();
        assert_eq!(size, 80);
    }

    #[test]
    fn rejects_invalid_lengths() {
        assert!(size_from_lengths(-1, 0, 0, 0, 0, None).is_none());
        assert!(size_from_lengths(0, 0, 0, 0, -1, None).is_none());
        assert!(size_from_lengths(0, 0, 0, 0, 0, Some(-1)).is_none());
    }
}
