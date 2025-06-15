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

use nom::number::complete::le_u32;

use crate::dic::read::error::SudachiNomResult;
use crate::dic::word_id::WordId;

pub fn le_u32_word_id(input: &[u8]) -> SudachiNomResult<&[u8], WordId> {
    le_u32(input).map(|(rest, id)| (rest, WordId::from_raw(id)))
}
