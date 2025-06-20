/*
 *  Copyright (c) 2025 Works Applications Co., Ltd.
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

use nom::number::complete::le_i16;

use super::error::{SudachiNomError, SudachiNomResult};
use super::u16str::U16CodeUnits;

pub fn utf16_string_of_length(input: &[u8], char_length: usize) -> SudachiNomResult<&[u8], String> {
    let num_bytes = char_length * 2;
    input
        .split_at_checked(num_bytes)
        .ok_or(nom::Err::Failure(SudachiNomError::Utf16String))
        .and_then(|(data, rest)| {
            if data.is_empty() {
                Ok((rest, String::new()))
            } else {
                // most Japanese chars are 3-bytes in utf-8 and 2 in utf-16
                let estimated_capacity = (data.len() + 1) * 3 / 2;
                let mut result = String::with_capacity(estimated_capacity);
                let iter = U16CodeUnits::new(data);
                for c in char::decode_utf16(iter) {
                    match c {
                        Err(_) => return Err(nom::Err::Failure(SudachiNomError::Utf16String)),
                        Ok(c) => result.push(c),
                    }
                }
                Ok((rest, result))
            }
        })
}

pub fn utf16_string(input: &[u8]) -> SudachiNomResult<&[u8], String> {
    let (rest, char_length) = le_i16(input)?;
    utf16_string_of_length(rest, char_length as usize)
}

pub fn skip_utf16_string(input: &[u8]) -> SudachiNomResult<&[u8], String> {
    let (rest, length) = le_i16(input)?;
    let num_bytes = (length * 2) as usize;
    let (rest, _) = nom::bytes::complete::take(num_bytes)(rest)?;
    Ok((rest, String::new()))
}
