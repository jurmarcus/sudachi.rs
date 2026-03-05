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

use std::io::Write;

use crate::analysis::Mode;
use crate::dic::build::error::DicWriteResult;
use crate::dic::build::primitives::{write_u32_array, Utf16Writer};
use crate::dic::lexicon::word_infos::WordInfos;

use super::refs::{NormFormValue, WordRef};

pub(crate) struct RawLexiconEntry {
    pub left_id: i16,
    pub right_id: i16,
    pub cost: i16,
    pub surface: String,
    pub headword: Option<String>,
    pub dic_form: WordRef,
    pub norm_form: Option<NormFormValue>,
    pub pos: u16,
    pub splits_a: Vec<WordRef>,
    pub splits_b: Vec<WordRef>,
    #[allow(unused)]
    pub splits_c: Vec<WordRef>,
    pub reading: Option<String>,
    #[allow(unused)]
    pub splitting: Mode,
    pub word_structure: Vec<WordRef>,
    pub synonym_groups: Vec<u32>,
    #[allow(unused)]
    pub user_data: String,
}

impl RawLexiconEntry {
    const MIN_ENTRY_SIZE: usize = 32;

    pub(super) fn make_phantom(base: &RawLexiconEntry, headword: String) -> Self {
        Self {
            // keep surface empty so this entry is not indexable and only used for reference resolution.
            surface: String::new(),
            headword: Some(headword),
            left_id: -1,
            right_id: -1,
            cost: i16::MAX,
            pos: base.pos,
            reading: base.reading.clone(),
            dic_form: base.dic_form.clone(),
            norm_form: None,
            splitting: base.splitting,
            splits_a: base.splits_a.clone(),
            splits_b: base.splits_b.clone(),
            splits_c: base.splits_c.clone(),
            word_structure: base.word_structure.clone(),
            synonym_groups: base.synonym_groups.clone(),
            user_data: base.user_data.clone(),
        }
    }

    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub fn headword(&self) -> &str {
        self.headword.as_deref().unwrap_or_else(|| self.surface())
    }

    pub fn norm_form(&self) -> &str {
        match self.norm_form.as_ref() {
            None => self.headword(),
            Some(NormFormValue::Value(s)) => s,
            Some(NormFormValue::Ref(_)) => {
                panic!("normalized_form must be resolved before writing")
            }
        }
    }

    pub fn reading(&self) -> &str {
        self.reading.as_deref().unwrap_or_else(|| self.headword())
    }

    pub fn should_index(&self) -> bool {
        self.left_id >= 0
    }

    pub fn expected_entry_size(&self) -> usize {
        let mut size = Self::MIN_ENTRY_SIZE;
        size += self.splits_c.len() * 4;
        if self.splits_b != self.splits_c {
            size += self.splits_b.len() * 4;
        }
        if self.splits_a != self.splits_b {
            size += self.splits_a.len() * 4;
        }
        if self.word_structure != self.splits_a {
            size += self.word_structure.len() * 4;
        }
        size += self.synonym_groups.len() * 4;
        if !self.user_data.is_empty() {
            size += 2 + self.user_data.encode_utf16().count() * 2;
        }

        // ceiling based on WORD_INFO_OFFSET_ALIGNMENT
        (size + (WordInfos::WORD_INFO_OFFSET_ALIGNMENT - 1))
            & !(WordInfos::WORD_INFO_OFFSET_ALIGNMENT - 1)
    }

    pub fn write_params<W: Write>(&self, w: &mut W) -> DicWriteResult<usize> {
        w.write_all(&self.left_id.to_le_bytes())?;
        w.write_all(&self.right_id.to_le_bytes())?;
        w.write_all(&self.cost.to_le_bytes())?;
        Ok(6)
    }

    pub fn write_word_info<W: Write>(
        &self,
        u16w: &mut Utf16Writer,
        w: &mut W,
    ) -> DicWriteResult<usize> {
        let mut size = 0;

        size += u16w.write(w, self.headword())?; // surface of WordInfo
        size += u16w.write_len(w, self.surface.len())?; // surface for trie
        w.write_all(&self.pos.to_le_bytes())?;
        size += 2;
        size += u16w.write_empty_if_equal(w, self.norm_form(), self.headword())?;
        let dic_form = match self.dic_form {
            WordRef::Ref(wid) => wid,
            WordRef::LineRef(_) => panic!("dictionary_form must be resolved before writing"),
            WordRef::Headword(_) => panic!("dictionary_form must be resolved before writing"),
            WordRef::Inline { .. } => panic!("dictionary_form must be resolved before writing"),
        };
        w.write_all(&dic_form.as_raw().to_le_bytes())?;
        size += 4;
        size += u16w.write_empty_if_equal(w, self.reading(), self.headword())?;
        size += write_u32_array(w, &self.splits_a)?;
        size += write_u32_array(w, &self.splits_b)?;
        let mut ws = Vec::with_capacity(self.word_structure.len());
        for s in self.word_structure.iter() {
            match s {
                WordRef::Ref(wid) => ws.push(*wid),
                _ => panic!("word_structure refs must be resolved before writing"),
            }
        }
        size += write_u32_array(w, &ws)?;
        size += write_u32_array(w, &self.synonym_groups)?;

        Ok(size)
    }
}
