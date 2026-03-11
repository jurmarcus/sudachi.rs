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
use crate::dic::build::error::{BuildFailure, DicWriteResult};
use crate::dic::lexicon::strings::StringPointer;
use crate::dic::word_id::WordId;
use crate::dic::word_info::{
    layout, WordInfoFixedData, WordInfoVariableData, WordInfoVariableLayout,
};

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

    #[cfg_attr(not(test), allow(dead_code))]
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
        let variable = self
            .variable_layout()
            .expect("entry lengths are validated before size computation");

        layout::size_from_variable_layout(variable)
            .expect("entry lengths are validated before size computation")
    }

    pub fn write_params<W: Write>(&self, w: &mut W) -> DicWriteResult<usize> {
        w.write_all(&self.left_id.to_le_bytes())?;
        w.write_all(&self.right_id.to_le_bytes())?;
        w.write_all(&self.cost.to_le_bytes())?;
        Ok(6)
    }

    pub fn write_rest<W: Write>(
        &self,
        w: &mut W,
        self_word_id: WordId,
        norm_form_word_id: WordId,
        headword_strptr: StringPointer,
        reading_strptr: StringPointer,
    ) -> DicWriteResult<usize> {
        let dic_form_word_id = match self.dic_form {
            WordRef::Ref(wid) => wid,
            WordRef::SelfRef => self_word_id,
            WordRef::LineRef(_) => panic!("dictionary_form must be resolved before writing"),
            WordRef::Headword(_) => panic!("dictionary_form must be resolved before writing"),
            WordRef::Inline { .. } => panic!("dictionary_form must be resolved before writing"),
        };

        if self.surface.len() > i16::MAX as usize {
            return Err(BuildFailure::InvalidFieldSize {
                actual: self.surface.len(),
                expected: i16::MAX as usize,
                field: "index_form_length",
            });
        }

        let variable = self.variable_layout()?;

        let fixed = WordInfoFixedData {
            pos_id: self.pos as i16,
            headword_strptr,
            reading_form_strptr: reading_strptr,
            normalized_form: norm_form_word_id.as_raw(),
            dictionary_form: dic_form_word_id.as_raw(),
            index_form_length: self.surface.len() as i16,
            c_unit_split_length: variable.c_unit_split_length,
            b_unit_split_length: variable.b_unit_split_length,
            a_unit_split_length: variable.a_unit_split_length,
            word_structure_length: variable.word_structure_length,
            synonym_group_ids_length: variable.synonym_group_ids_length,
            user_data_flag: variable.user_data_flag,
        };
        let mut size = fixed.write_to(w)?;
        let c_refs = self.word_ref_ids(&self.splits_c);
        let b_refs = self.word_ref_ids(&self.splits_b);
        let a_refs = self.word_ref_ids(&self.splits_a);
        let ws_refs = self.word_ref_ids(&self.word_structure);
        let syns: Vec<i32> = self.synonym_groups.iter().map(|sg| *sg as i32).collect();
        let payload = WordInfoVariableData {
            c_unit_split: &c_refs,
            b_unit_split: &b_refs,
            a_unit_split: &a_refs,
            word_structure: &ws_refs,
            synonym_group_ids: &syns,
            user_data: &self.user_data,
        };
        size += payload.write_to(w, &fixed)?;

        Ok(size)
    }

    fn word_ref_ids(&self, refs: &[WordRef]) -> Vec<u32> {
        let mut result = Vec::with_capacity(refs.len());
        for s in refs {
            match s {
                WordRef::Ref(wid) => {
                    result.push(wid.as_raw());
                }
                _ => panic!("word refs must be resolved before writing"),
            }
        }
        result
    }

    fn variable_layout(&self) -> DicWriteResult<WordInfoVariableLayout> {
        WordInfoVariableLayout::new(
            self.splits_c.len(),
            self.splits_b == self.splits_c,
            self.splits_b.len(),
            self.splits_a == self.splits_b,
            self.splits_a.len(),
            self.word_structure == self.splits_a,
            self.word_structure.len(),
            self.synonym_groups.len(),
            self.user_data.encode_utf16().count(),
        )
        .ok_or_else(|| {
            let user_data_units = self.user_data.encode_utf16().count();
            if self.splits_c.len() > i8::MAX as usize {
                return BuildFailure::InvalidFieldSize {
                    actual: self.splits_c.len(),
                    expected: i8::MAX as usize,
                    field: "splits_c",
                };
            }
            if self.splits_b.len() > i8::MAX as usize {
                return BuildFailure::InvalidFieldSize {
                    actual: self.splits_b.len(),
                    expected: i8::MAX as usize,
                    field: "splits_b",
                };
            }
            if self.splits_a.len() > i8::MAX as usize {
                return BuildFailure::InvalidFieldSize {
                    actual: self.splits_a.len(),
                    expected: i8::MAX as usize,
                    field: "splits_a",
                };
            }
            if self.word_structure.len() > i8::MAX as usize {
                return BuildFailure::InvalidFieldSize {
                    actual: self.word_structure.len(),
                    expected: i8::MAX as usize,
                    field: "word_structure",
                };
            }
            if self.synonym_groups.len() > i8::MAX as usize {
                return BuildFailure::InvalidFieldSize {
                    actual: self.synonym_groups.len(),
                    expected: i8::MAX as usize,
                    field: "synonym_groups",
                };
            }
            BuildFailure::InvalidFieldSize {
                actual: user_data_units,
                expected: i16::MAX as usize,
                field: "user_data",
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Mode;
    use crate::dic::lexicon::strings::StringPointer;
    use crate::dic::word_info::WordInfoParser;

    #[test]
    fn writer_output_is_readable_by_parser() {
        let entry = RawLexiconEntry {
            left_id: 1,
            right_id: 2,
            cost: 3,
            surface: "東京".to_string(),
            headword: Some("京都".to_string()),
            dic_form: WordRef::SelfRef,
            norm_form: None,
            pos: 4,
            splits_a: vec![WordRef::Ref(WordId::new(0, 1)), WordRef::Ref(WordId::new(0, 2))],
            splits_b: vec![WordRef::Ref(WordId::new(0, 1)), WordRef::Ref(WordId::new(0, 2))],
            splits_c: vec![WordRef::Ref(WordId::new(0, 1)), WordRef::Ref(WordId::new(0, 2))],
            reading: Some("キョウト".to_string()),
            splitting: Mode::B,
            word_structure: vec![WordRef::Ref(WordId::new(0, 3))],
            synonym_groups: vec![7, 8],
            user_data: "meta".to_string(),
        };

        let mut bytes = vec![0u8; layout::PARAMS_SIZE];
        let expected_size = entry.expected_entry_size();
        let written = entry
            .write_rest(
                &mut bytes,
                WordId::new(0, 10),
                WordId::new(0, 11),
                StringPointer::unchecked(2, 4),
                StringPointer::unchecked(4, 8),
            )
            .unwrap();
        assert_eq!(layout::aligned_size(written + layout::PARAMS_SIZE), expected_size);

        let parsed = WordInfoParser::default().parse(&bytes).unwrap();
        assert_eq!(parsed.pos_id, 4);
        assert_eq!(parsed.index_form_length, "東京".len() as i16);
        assert_eq!(parsed.dictionary_form, WordId::new(0, 10).as_raw());
        assert_eq!(parsed.normalized_form, WordId::new(0, 11).as_raw());
        assert_eq!(parsed.c_unit_split, vec![WordId::new(0, 1).as_raw(), WordId::new(0, 2).as_raw()]);
        assert_eq!(parsed.b_unit_split, parsed.c_unit_split);
        assert_eq!(parsed.a_unit_split, vec![WordId::new(0, 1).as_raw(), WordId::new(0, 2).as_raw()]);
        assert_eq!(parsed.word_structure, vec![WordId::new(0, 3).as_raw()]);
        assert_eq!(parsed.synonym_group_ids, vec![7, 8]);
        assert_eq!(parsed.user_data, "meta");
    }
}
