/*
 *  Copyright (c) 2026 Works Applications Co., Ltd.
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

use crate::dic::build::error::BuildFailure;
use crate::dic::word_id::WordId;
use crate::dic::word_info::WordInfos;
use crate::error::SudachiResult;

use super::{
    LexiconReader, NormFormValue, ParsedLexiconEntry, ResolvedDicForm, ResolvedLexiconEntry,
    WordRef, WordRefResolver,
};

impl LexiconReader {
    pub(crate) fn invalidate_resolved_entries(&mut self) {
        self.resolved_entries.clear();
    }

    pub(crate) fn ensure_resolved_entries(&mut self) -> SudachiResult<()> {
        if !self.resolved_entries.is_empty() || self.parsed_entries.is_empty() {
            return Ok(());
        }
        if self.unresolved > 0 {
            return self.ctx.err(BuildFailure::UnresolvedSplits);
        }
        self.rebuild_resolved_entries()
    }

    pub(super) fn rebuild_resolved_entries(&mut self) -> SudachiResult<()> {
        let mut resolved = Vec::with_capacity(self.parsed_entries.len());
        for entry in self.parsed_entries.iter().cloned() {
            resolved.push(
                Self::parsed_entry_to_resolved(entry)
                    .map_err(|e| self.ctx.to_sudachi_err(BuildFailure::InvalidSplit(e)))?,
            );
        }
        self.resolved_entries = resolved;
        Ok(())
    }

    fn parsed_entry_to_resolved(entry: ParsedLexiconEntry) -> Result<ResolvedLexiconEntry, String> {
        let dic_form = match entry.dic_form {
            WordRef::Ref(wid) => ResolvedDicForm::Ref(wid),
            WordRef::SelfRef => ResolvedDicForm::SelfRef,
            other => return Err(format!("unresolved dictionary_form: {:?}", other)),
        };
        let norm_form = match entry.norm_form {
            None => None,
            Some(NormFormValue::Value(v)) => Some(v),
            Some(NormFormValue::Ref(other)) => {
                return Err(format!("unresolved normalized_form: {:?}", other))
            }
        };
        Ok(ResolvedLexiconEntry {
            left_id: entry.left_id,
            right_id: entry.right_id,
            cost: entry.cost,
            index_form: entry.index_form,
            headword: entry.headword,
            dic_form,
            norm_form,
            pos: entry.pos,
            splits_a: Self::resolved_word_ids(entry.splits_a)?,
            splits_b: Self::resolved_word_ids(entry.splits_b)?,
            splits_c: Self::resolved_word_ids(entry.splits_c)?,
            reading: entry.reading,
            splitting: entry.splitting,
            word_structure: Self::resolved_word_ids(entry.word_structure)?,
            synonym_groups: entry.synonym_groups,
            user_data: entry.user_data,
        })
    }

    fn resolved_word_ids(values: Vec<WordRef>) -> Result<Vec<WordId>, String> {
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            match value {
                WordRef::Ref(wid) => out.push(wid),
                other => return Err(format!("unresolved word reference: {:?}", other)),
            }
        }
        Ok(out)
    }

    pub(crate) fn resolve_splits<R: WordRefResolver>(
        &mut self,
        resolver: &R,
    ) -> Result<usize, (String, usize)> {
        let mut total = 0;
        let mut resolved_parsed = Vec::with_capacity(self.parsed_entries.len());
        let mut resolved_entries = Vec::with_capacity(self.parsed_entries.len());
        let mut phantom_parsed: Vec<ParsedLexiconEntry> = Vec::new();
        let mut phantom_resolved: Vec<ResolvedLexiconEntry> = Vec::new();
        for (line, entry) in self.parsed_entries.iter().cloned().enumerate() {
            let (parsed, resolved, resolved_count, phantom_headword) = self
                .resolve_entry(entry, resolver, &phantom_parsed)
                .map_err(|split_info| (split_info, line))?;
            total += resolved_count;
            if let Some(headword) = phantom_headword {
                phantom_parsed.push(ParsedLexiconEntry::make_phantom(&parsed, headword.clone()));
                phantom_resolved.push(ResolvedLexiconEntry::make_phantom(&resolved, headword));
            }
            resolved_parsed.push(parsed);
            resolved_entries.push(resolved);
        }
        resolved_parsed.extend(phantom_parsed);
        resolved_entries.extend(phantom_resolved);
        self.unresolved = 0;
        self.parsed_entries = resolved_parsed;
        self.resolved_entries = resolved_entries;
        Ok(total)
    }

    fn has_headword(&self, headword: &str) -> bool {
        self.parsed_entries.iter().any(|e| e.headword() == headword)
    }

    pub(crate) fn row_word_ids(&self, dic_id: u8) -> Vec<WordId> {
        let mut result = Vec::with_capacity(self.parsed_entries.len());
        let mut offset = Self::ENTRY_INITIAL_OFFSET;
        for e in &self.parsed_entries {
            let entry_id = (offset >> WordInfos::WORD_ID_ALIGNMENT_BITS) as u32;
            result.push(WordId::new(dic_id, entry_id));
            offset += e.expected_entry_size();
        }
        result
    }

    fn resolve_entry<R: WordRefResolver>(
        &self,
        entry: ParsedLexiconEntry,
        resolver: &R,
        phantoms: &[ParsedLexiconEntry],
    ) -> Result<
        (
            ParsedLexiconEntry,
            ResolvedLexiconEntry,
            usize,
            Option<String>,
        ),
        String,
    > {
        let mut total = 0;
        let (norm_form, phantom_headword) =
            self.resolve_norm_form(&entry, resolver, phantoms, &mut total)?;
        let dic_form = self
            .resolve_dic_form_ref(&entry.dic_form, resolver, &mut total)
            .map_err(|r| self.format_word_ref(&r))?;
        let splits_a = self
            .resolve_word_refs(&entry.splits_a, resolver, &mut total)
            .map_err(|r| self.format_word_ref(&r))?;
        let splits_b = self
            .resolve_word_refs(&entry.splits_b, resolver, &mut total)
            .map_err(|r| self.format_word_ref(&r))?;
        let splits_c = self
            .resolve_word_refs(&entry.splits_c, resolver, &mut total)
            .map_err(|r| self.format_word_ref(&r))?;
        let word_structure = self
            .resolve_word_refs(&entry.word_structure, resolver, &mut total)
            .map_err(|r| self.format_word_ref(&r))?;

        let parsed = ParsedLexiconEntry {
            left_id: entry.left_id,
            right_id: entry.right_id,
            cost: entry.cost,
            index_form: entry.index_form.clone(),
            headword: entry.headword.clone(),
            dic_form: match dic_form {
                ResolvedDicForm::Ref(wid) => WordRef::Ref(wid),
                ResolvedDicForm::SelfRef => WordRef::SelfRef,
            },
            norm_form: norm_form.clone().map(NormFormValue::Value),
            pos: entry.pos,
            splits_a: splits_a.iter().copied().map(WordRef::Ref).collect(),
            splits_b: splits_b.iter().copied().map(WordRef::Ref).collect(),
            splits_c: splits_c.iter().copied().map(WordRef::Ref).collect(),
            reading: entry.reading.clone(),
            splitting: entry.splitting,
            word_structure: word_structure.iter().copied().map(WordRef::Ref).collect(),
            synonym_groups: entry.synonym_groups.clone(),
            user_data: entry.user_data.clone(),
        };

        let resolved = ResolvedLexiconEntry {
            left_id: entry.left_id,
            right_id: entry.right_id,
            cost: entry.cost,
            index_form: entry.index_form,
            headword: entry.headword,
            dic_form,
            norm_form,
            pos: entry.pos,
            splits_a,
            splits_b,
            splits_c,
            reading: entry.reading,
            splitting: entry.splitting,
            word_structure,
            synonym_groups: entry.synonym_groups,
            user_data: entry.user_data,
        };

        Ok((parsed, resolved, total, phantom_headword))
    }

    fn resolve_norm_form<R: WordRefResolver>(
        &self,
        entry: &ParsedLexiconEntry,
        resolver: &R,
        phantoms: &[ParsedLexiconEntry],
        total: &mut usize,
    ) -> Result<(Option<String>, Option<String>), String> {
        match entry.norm_form.as_ref() {
            None => Ok((None, None)),
            Some(NormFormValue::Value(v)) => Ok((Some(v.clone()), None)),
            Some(NormFormValue::Ref(WordRef::Headword(headword))) => {
                if resolver.resolve_by_headword(headword).is_none()
                    && !self.has_headword(headword)
                    && !phantoms.iter().any(|p| p.headword() == headword)
                {
                    *total += 1;
                    if headword == entry.headword() {
                        Ok((None, Some(headword.clone())))
                    } else {
                        Ok((Some(headword.clone()), Some(headword.clone())))
                    }
                } else {
                    *total += 1;
                    if headword == entry.headword() {
                        Ok((None, None))
                    } else {
                        Ok((Some(headword.clone()), None))
                    }
                }
            }
            Some(NormFormValue::Ref(word_ref)) => {
                let wid = resolver
                    .resolve(word_ref)
                    .ok_or_else(|| self.format_word_ref(word_ref))?;
                *total += 1;
                let headword = resolver
                    .resolve_headword(wid)
                    .ok_or_else(|| self.format_word_ref(&WordRef::Ref(wid)))?;
                if headword == entry.headword() {
                    Ok((None, None))
                } else {
                    Ok((Some(headword), None))
                }
            }
        }
    }

    fn resolve_dic_form_ref<R: WordRefResolver>(
        &self,
        word_ref: &WordRef,
        resolver: &R,
        total: &mut usize,
    ) -> Result<ResolvedDicForm, WordRef> {
        match word_ref {
            WordRef::SelfRef => Ok(ResolvedDicForm::SelfRef),
            WordRef::Ref(wid) => Ok(ResolvedDicForm::Ref(*wid)),
            other => {
                let wid = resolver.resolve(other).ok_or_else(|| other.clone())?;
                *total += 1;
                Ok(ResolvedDicForm::Ref(wid))
            }
        }
    }

    fn resolve_word_refs<R: WordRefResolver>(
        &self,
        word_refs: &[WordRef],
        resolver: &R,
        total: &mut usize,
    ) -> Result<Vec<WordId>, WordRef> {
        let mut out = Vec::with_capacity(word_refs.len());
        for word_ref in word_refs {
            match word_ref {
                WordRef::Ref(wid) => out.push(*wid),
                other => {
                    let wid = resolver.resolve(other).ok_or_else(|| other.clone())?;
                    *total += 1;
                    out.push(wid);
                }
            }
        }
        Ok(out)
    }

    fn format_word_ref(&self, word_ref: &WordRef) -> String {
        match word_ref {
            WordRef::Ref(id) => id.as_raw().to_string(),
            WordRef::SelfRef => "<self>".to_owned(),
            WordRef::LineRef(id) => id.as_raw().to_string(),
            WordRef::Headword(h) => h.clone(),
            WordRef::Inline {
                headword,
                pos,
                reading,
            } => format!(
                "{},{:?},{}",
                headword,
                self.pos_obj(*pos).unwrap(),
                reading.as_ref().unwrap_or(headword)
            ),
        }
    }
}
