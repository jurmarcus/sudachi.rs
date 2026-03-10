/*
 * Copyright (c) 2025-2026 Works Applications Co., Ltd.
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

#[path = "word_info/data.rs"]
mod data;
#[path = "word_info/layout.rs"]
pub mod layout;
#[path = "word_info/parse.rs"]
pub mod parse;
#[path = "word_info/raw.rs"]
mod raw;

pub use data::*;
pub use raw::WordInfoRawData;
