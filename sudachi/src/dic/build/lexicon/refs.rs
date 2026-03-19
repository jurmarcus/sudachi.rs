/*
 *  Copyright (c) 2021-2026 Works Applications Co., Ltd.
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

use crate::dic::word_id::WordId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum WordRef {
    Ref(WordId),
    // explicit self-reference used for dictionary_form omission.
    SelfRef,
    // we use WordId to store system/user flag with line number.
    LineRef(WordId),
    Headword(String),
    Inline {
        headword: String,
        pos: u16,
        reading: Option<String>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum NormFormValue {
    Value(String),
    Ref(WordRef),
}

pub(crate) trait WordRefResolver {
    fn resolve(&self, unit: &WordRef) -> Option<WordId> {
        match unit {
            WordRef::Ref(wid) => Some(*wid),
            WordRef::SelfRef => None,
            WordRef::LineRef(line_ref) => self.resolve_by_line_ref(*line_ref),
            WordRef::Headword(headword) => self.resolve_by_headword(headword),
            WordRef::Inline {
                headword,
                pos,
                reading,
            } => self.resolve_inline(headword, *pos, reading.as_deref()),
        }
    }

    fn resolve_by_line_ref(&self, line_ref: WordId) -> Option<WordId>;

    fn resolve_by_headword(&self, headword: &str) -> Option<WordId>;

    fn resolve_inline(&self, headword: &str, pos: u16, reading: Option<&str>) -> Option<WordId>;

    fn resolve_headword(&self, _wid: WordId) -> Option<String> {
        None
    }
}
