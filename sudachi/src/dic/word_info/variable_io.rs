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

use nom::number::complete::{le_i32, le_u32};

use crate::dic::read::utf16_string::{skip_utf16_string, utf16_string};
use crate::error::SudachiResult;

pub(crate) fn parse_u32_array(
    input: &[u8],
    length: usize,
    keep: bool,
) -> SudachiResult<(&[u8], Vec<u32>)> {
    if keep {
        let (rest, values) = nom::multi::count(le_u32, length)(input)?;
        Ok((rest, values))
    } else {
        let bytes = length * 4;
        let (rest, _) = nom::bytes::complete::take(bytes)(input)?;
        Ok((rest, Vec::new()))
    }
}

pub(crate) fn parse_i32_array(
    input: &[u8],
    length: usize,
    keep: bool,
) -> SudachiResult<(&[u8], Vec<i32>)> {
    if keep {
        let (rest, values) = nom::multi::count(le_i32, length)(input)?;
        Ok((rest, values))
    } else {
        let bytes = length * 4;
        let (rest, _) = nom::bytes::complete::take(bytes)(input)?;
        Ok((rest, Vec::new()))
    }
}

pub(crate) fn parse_user_data(input: &[u8], keep: bool) -> SudachiResult<(&[u8], String)> {
    if keep {
        utf16_string(input).map_err(Into::into)
    } else {
        skip_utf16_string(input).map_err(Into::into)
    }
}

pub(crate) fn write_u32_slice<W: Write>(w: &mut W, data: &[u32]) -> std::io::Result<usize> {
    let mut size = 0;
    for value in data {
        w.write_all(&value.to_le_bytes())?;
        size += 4;
    }
    Ok(size)
}

pub(crate) fn write_i32_slice<W: Write>(w: &mut W, data: &[i32]) -> std::io::Result<usize> {
    let mut size = 0;
    for value in data {
        w.write_all(&value.to_le_bytes())?;
        size += 4;
    }
    Ok(size)
}

pub(crate) fn write_utf16_string<W: Write>(w: &mut W, data: &str) -> std::io::Result<usize> {
    let utf16: Vec<u16> = data.encode_utf16().collect();
    let mut size = 0;
    w.write_all(&(utf16.len() as i16).to_le_bytes())?;
    size += 2;
    for unit in utf16 {
        w.write_all(&unit.to_le_bytes())?;
        size += 2;
    }
    Ok(size)
}
