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

use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::Write;
use std::path::Path;

use csv::{StringRecord, Trim};
use indexmap::map::IndexMap;
use indexmap::Equivalent;
use memmap2::Mmap;

use crate::analysis::Mode;
use crate::dic::build::error::{BuildFailure, DicCompilationCtx, DicWriteResult};
use crate::dic::build::parse::{
    it_next, none_if_equal, parse_i16, parse_mode, parse_slash_list, parse_u32_list_with_asterisk,
    parse_wordid, unescape, unescape_cow, WORD_ID_LITERAL,
};
#[cfg(test)]
use crate::dic::build::primitives::Utf16Writer;
use crate::dic::build::pos::read_pos_bytes as read_pos_csv_bytes;
use crate::dic::build::report::{ReportBuilder, Reporter};
use crate::dic::build::MAX_POS_IDS;
use crate::dic::grammar::Grammar;
use crate::dic::lexicon::word_infos::WordInfos;
use crate::dic::pos::POS_DEPTH;
use crate::dic::word_id::WordId;
use crate::error::SudachiResult;

#[cfg(test)]
mod test;

#[cfg(test)]
mod wordinfo_test;

mod entry;
mod layout;
mod refs;
mod string_store;
pub(crate) use entry::RawLexiconEntry;
use layout::{Column, ColumnLayout, RecordWrapper};
pub(crate) use refs::{NormFormValue, WordRef, WordRefResolver};
pub use string_store::StringStore;

#[derive(Hash, Eq, PartialEq)]
pub struct StrPosEntry {
    data: [Cow<'static, str>; POS_DEPTH],
}

impl<'a> Borrow<[Cow<'a, str>; POS_DEPTH]> for StrPosEntry {
    fn borrow(&self) -> &[Cow<'a, str>; POS_DEPTH] {
        &self.data
    }
}

impl<'a> Equivalent<[Cow<'a, str>; POS_DEPTH]> for StrPosEntry {
    fn equivalent(&self, key: &[Cow<'_, str>; POS_DEPTH]) -> bool {
        self.data.eq(key)
    }
}

impl StrPosEntry {
    /// owning means 'static
    fn rewrap(data: Cow<str>) -> Cow<'static, str> {
        match data {
            Cow::Borrowed(b) => Cow::Owned(b.to_owned()),
            Cow::Owned(s) => Cow::Owned(s),
        }
    }

    pub fn new(data: [Cow<str>; POS_DEPTH]) -> Self {
        let [d1, d2, d3, d4, d5, d6] = data;
        let owned: [Cow<'static, str>; POS_DEPTH] = [
            Self::rewrap(d1),
            Self::rewrap(d2),
            Self::rewrap(d3),
            Self::rewrap(d4),
            Self::rewrap(d5),
            Self::rewrap(d6),
        ];
        Self { data: owned }
    }

    pub fn from_built_pos(data: &Vec<String>) -> Self {
        let mut iter = data.iter().map(|x| x.as_str());
        let p1 = Cow::Borrowed(iter.next().unwrap());
        let p2 = Cow::Borrowed(iter.next().unwrap());
        let p3 = Cow::Borrowed(iter.next().unwrap());
        let p4 = Cow::Borrowed(iter.next().unwrap());
        let p5 = Cow::Borrowed(iter.next().unwrap());
        let p6 = Cow::Borrowed(iter.next().unwrap());
        Self::new([p1, p2, p3, p4, p5, p6])
    }

    pub fn fields(&self) -> &[Cow<'static, str>; 6] {
        &self.data
    }
}

impl Debug for StrPosEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{},{},{},{},{},{}",
            self.data[0], self.data[1], self.data[2], self.data[3], self.data[4], self.data[5]
        )
    }
}

impl WordRef {
    pub fn format(&self, lexicon: &LexiconReader) -> String {
        match self {
            WordRef::Ref(id) => id.as_raw().to_string(),
            WordRef::SelfRef => "<self>".to_owned(),
            WordRef::LineRef(id) => id.as_raw().to_string(),
            WordRef::Headword(h) => h.clone(),
            WordRef::Inline {
                surface,
                pos,
                reading,
            } => format!(
                "{},{:?},{}",
                surface,
                lexicon.pos_obj(*pos).unwrap(),
                reading.as_ref().unwrap_or(surface)
            ),
        }
    }
}

pub struct LexiconReader {
    pos: IndexMap<StrPosEntry, u16>,
    ctx: DicCompilationCtx,
    entries: Vec<RawLexiconEntry>,
    unresolved: usize,
    start_pos: usize,
    max_left: i16,
    max_right: i16,
    num_system: usize,
}

impl LexiconReader {
    const ENTRY_INITIAL_OFFSET: usize = 32;

    pub fn new() -> Self {
        Self {
            pos: IndexMap::new(),
            ctx: DicCompilationCtx::default(),
            entries: Vec::new(),
            unresolved: 0,
            start_pos: 0,
            max_left: i16::MAX,
            max_right: i16::MAX,
            num_system: usize::MAX,
        }
    }

    pub(crate) fn entries(&self) -> &[RawLexiconEntry] {
        &self.entries
    }

    pub fn needs_split_resolution(&self) -> bool {
        self.unresolved > 0
    }

    pub fn set_max_conn_sizes(&mut self, left: i16, right: i16) {
        self.max_left = left;
        self.max_right = right;
    }

    pub fn set_num_system_words(&mut self, num: usize) {
        self.num_system = num;
    }

    pub fn preload_pos(&mut self, grammar: &Grammar) {
        assert_eq!(self.pos.len(), 0);
        for (i, pos) in grammar.pos_list.iter().enumerate() {
            let key = StrPosEntry::from_built_pos(pos);
            self.pos.insert(key, i as u16);
        }
        self.start_pos = self.pos.len();
    }

    pub(crate) fn pos_obj(&self, pos_id: u16) -> Option<&StrPosEntry> {
        self.pos.get_index(pos_id as usize).map(|(k, v)| {
            assert_eq!(v, &pos_id);
            k
        })
    }

    pub fn read_file(&mut self, path: &Path) -> SudachiResult<usize> {
        let file = File::open(path)?;
        let map = unsafe { Mmap::map(&file) }?;
        let filename = path.to_str().unwrap_or("<invalid-utf8>").to_owned();
        let old_name = self.ctx.set_filename(filename);
        let res = self.read_bytes(&map);
        self.ctx.set_filename(old_name);
        res
    }

    pub fn read_bytes(&mut self, data: &[u8]) -> SudachiResult<usize> {
        // check header later to parse both of v0/v1
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .trim(Trim::None)
            .flexible(true)
            .from_reader(data);
        let mut layout = ColumnLayout::Legacy;
        let mut first_row = true;
        let mut nread = 0;
        for record in reader.records() {
            match record {
                Ok(r) => {
                    let line = r.position().map_or(0, |p| p.line()) as usize;
                    self.ctx.set_line(line);
                    if first_row {
                        first_row = false;
                        let (resolved, skip_row) = ColumnLayout::from_record(&r, &self.ctx)?;
                        layout = resolved;
                        if skip_row {
                            continue;
                        }
                    }
                    self.read_record(&r, layout)?;
                    nread += 1;
                }
                Err(e) => {
                    let line = e.position().map_or(0, |p| p.line()) as usize;
                    self.ctx.set_line(line);
                    return Err(self.ctx.to_sudachi_err(BuildFailure::CsvError(e)));
                }
            }
        }
        Ok(nread)
    }

    pub fn read_pos_file(&mut self, path: &Path) -> SudachiResult<usize> {
        let file = File::open(path)?;
        let map = unsafe { Mmap::map(&file) }?;
        let filename = path.to_str().unwrap_or("<invalid-utf8>").to_owned();
        let old_name = self.ctx.set_filename(filename);
        let res = self.read_pos_bytes(&map);
        self.ctx.set_filename(old_name);
        res
    }

    pub fn read_pos_bytes(&mut self, data: &[u8]) -> SudachiResult<usize> {
        read_pos_csv_bytes(&mut self.pos, !self.entries.is_empty(), data, &mut self.ctx)
    }

    fn read_record(&mut self, data: &StringRecord, layout: ColumnLayout) -> SudachiResult<()> {
        self.parse_record(data, layout)
            .map(|r| self.entries.push(r))
    }

    fn parse_record(
        &mut self,
        data: &StringRecord,
        layout: ColumnLayout,
    ) -> SudachiResult<RawLexiconEntry> {
        let ctx = std::mem::take(&mut self.ctx);
        let rec = RecordWrapper { record: data, ctx };
        let index_form = rec.get_col(layout, Column::IndexForm, unescape)?;
        let left_id = rec.get_col(layout, Column::LeftId, parse_i16)?;
        let right_id = rec.get_col(layout, Column::RightId, parse_i16)?;
        let cost = rec.get_col(layout, Column::Cost, parse_i16)?;

        let headword = rec.get_col_or_empty(layout, Column::Headword, unescape_cow)?;

        let p1 = rec.get_col_or_empty(layout, Column::Pos1, unescape_cow)?;
        let p2 = rec.get_col_or_empty(layout, Column::Pos2, unescape_cow)?;
        let p3 = rec.get_col_or_empty(layout, Column::Pos3, unescape_cow)?;
        let p4 = rec.get_col_or_empty(layout, Column::Pos4, unescape_cow)?;
        let p5 = rec.get_col_or_empty(layout, Column::Pos5, unescape_cow)?;
        let p6 = rec.get_col_or_empty(layout, Column::Pos6, unescape_cow)?;

        let reading = rec.get_col(layout, Column::ReadingForm, unescape_cow)?;
        let normalized = rec.get_col(layout, Column::NormalizedForm, |s| Ok(s.to_owned()))?;
        let dic_form_ref = rec.get_col_or(layout, Column::DictionaryForm, "".to_owned(), |s| {
            Ok(s.to_owned())
        })?;
        let splitting = rec.get_col_or(layout, Column::Mode, Mode::C, parse_mode)?;
        let allow_word_id_ref = layout.is_legacy();
        let allow_asterisk = layout.is_legacy();
        let (split_a, resolve_a) = rec.get_col(layout, Column::SplitA, |s| {
            self.parse_splits_with_asterisk(s, allow_word_id_ref, allow_asterisk)
        })?;
        let (split_b, resolve_b) = rec.get_col(layout, Column::SplitB, |s| {
            self.parse_splits_with_asterisk(s, allow_word_id_ref, allow_asterisk)
        })?;
        let (split_c, resolve_c) = rec.get_col_or_default(layout, Column::SplitC, |s| {
            self.parse_splits_with_asterisk(s, allow_word_id_ref, allow_asterisk)
        })?;
        let (parts, resolve_parts) = rec.get_col(layout, Column::WordStructure, |s| {
            self.parse_splits_with_asterisk(s, allow_word_id_ref, allow_asterisk)
        })?;
        let synonyms = rec.get_col_or_default(layout, Column::SynonymGroups, |s| {
            parse_u32_list_with_asterisk(s, allow_asterisk)
        })?;
        let user_data = rec.get_col_or_default(layout, Column::UserData, unescape)?;
        let pos_id = rec.get_col_or(layout, Column::PosId, -1_i16, |s| {
            if s.is_empty() {
                Ok(-1)
            } else {
                parse_i16(s)
            }
        })?;

        let pos = if !p1.is_empty() {
            let pos = rec
                .ctx
                .transform(self.pos_id_of([p1, p2, p3, p4, p5, p6]))?;
            if pos_id >= 0 && pos_id as u16 != pos {
                return rec.ctx.err(BuildFailure::InvalidSplit(
                    "PosId and Pos1..Pos6 do not match".to_owned(),
                ));
            }
            pos
        } else if pos_id >= 0 {
            let pos = pos_id as u16;
            if pos as usize >= self.pos.len() {
                return rec.ctx.err(BuildFailure::InvalidSplit(
                    "POS for id was not present in the table".to_owned(),
                ));
            }
            pos
        } else {
            return rec.ctx.err(BuildFailure::InvalidSplit(
                "Both PosId and Pos1..Pos6 are missing".to_owned(),
            ));
        };

        if splitting == Mode::A && (!split_a.is_empty() || !split_b.is_empty()) {
            return rec.ctx.err(BuildFailure::InvalidSplit(
                "A-mode tokens can't have splits".to_owned(),
            ));
        }

        let effective_headword: Cow<str> = if headword.is_empty() {
            Cow::Borrowed(index_form.as_str())
        } else {
            headword
        };

        let (dic_form, resolve_dic_form) = rec.ctx.transform(self.parse_dic_form(
            &dic_form_ref,
            allow_word_id_ref,
            effective_headword.as_ref(),
            pos,
            reading.as_ref(),
        ))?;
        let (norm_form, resolve_norm_form) = rec
            .ctx
            .transform(self.parse_norm_form(&normalized, effective_headword.as_ref()))?;
        self.unresolved += resolve_a
            + resolve_b
            + resolve_c
            + resolve_parts
            + resolve_dic_form
            + resolve_norm_form;

        if index_form.is_empty() {
            return rec.ctx.err(BuildFailure::EmptySurface);
        }

        self.ctx = rec.ctx;

        let entry = RawLexiconEntry {
            left_id,
            right_id,
            cost,
            dic_form,
            norm_form,
            reading: none_if_equal(effective_headword.as_ref(), reading),
            headword: none_if_equal(&index_form, effective_headword),
            surface: index_form,
            pos,
            splitting,
            splits_a: split_a,
            splits_b: split_b,
            splits_c: split_c,
            word_structure: parts,
            synonym_groups: synonyms,
            user_data,
        };

        Ok(entry)
    }

    fn pos_id_of(&mut self, data: [Cow<str>; POS_DEPTH]) -> DicWriteResult<u16> {
        match self.pos.get(&data) {
            Some(pos) => Ok(*pos),
            None => {
                let key = StrPosEntry::new(data);
                let pos_id = self.pos.len();
                if pos_id > MAX_POS_IDS {
                    Err(BuildFailure::PosLimitExceeded(format!("{:?}", key)))
                } else {
                    let pos_id = pos_id as u16;
                    self.pos.insert(key, pos_id);
                    Ok(pos_id)
                }
            }
        }
    }

    pub fn validate_entries(&self) -> SudachiResult<()> {
        let mut ctx = DicCompilationCtx::default();
        ctx.set_filename("<entry id>".to_owned());
        ctx.set_line(0);
        let max_current = self.max_entry_id_plus_one() as usize;
        let (max_0, max_1) = match self.num_system {
            // means that we compile system dictionary, there must not be user words
            usize::MAX => (max_current, 0),
            // compiling user dictionary
            x => (x, max_current),
        };
        for e in self.entries.iter() {
            if e.left_id >= self.max_left {
                return ctx.err(BuildFailure::InvalidFieldSize {
                    actual: e.left_id as _,
                    expected: self.max_left as _,
                    field: "left_id",
                });
            }

            if e.right_id >= self.max_right {
                return ctx.err(BuildFailure::InvalidFieldSize {
                    actual: e.right_id as _,
                    expected: self.max_right as _,
                    field: "right_id",
                });
            }

            match e.dic_form {
                WordRef::Ref(wid) => {
                    ctx.transform(Self::validate_wid(wid, max_0, max_1, "dic_form"))?;
                }
                WordRef::SelfRef => {}
                _ => panic!("at this point dictionary_form must be resolved"),
            }
            if matches!(e.norm_form, Some(NormFormValue::Ref(_))) {
                panic!("at this point normalized_form must be resolved");
            }

            for s in e.splits_a.iter() {
                match s {
                    WordRef::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "splits_a"))?;
                    }
                    _ => panic!("at this point there must not be unresolved splits"),
                }
            }

            for s in e.splits_b.iter() {
                match s {
                    WordRef::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "splits_b"))?;
                    }
                    _ => panic!("at this point there must not be unresolved splits"),
                }
            }

            for s in e.splits_c.iter() {
                match s {
                    WordRef::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "splits_c"))?;
                    }
                    _ => panic!("at this point there must not be unresolved splits"),
                }
            }

            for wid in e.word_structure.iter() {
                match wid {
                    WordRef::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "word_structure"))?;
                    }
                    _ => panic!("at this point there must not be unresolved word_structure"),
                }
            }

            ctx.add_line(1);
        }
        Ok(())
    }

    fn validate_wid(
        wid: WordId,
        dic0_max: usize,
        dic1_max: usize,
        label: &'static str,
    ) -> DicWriteResult<()> {
        let max = match wid.dict().as_raw() {
            0 => dic0_max,
            1 => dic1_max,
            x => panic!("invalid dictionary ID={}, should not happen", x),
        };
        if wid.entry().as_raw() >= max as u32 {
            return Err(BuildFailure::InvalidFieldSize {
                actual: wid.entry().as_raw() as _,
                expected: max,
                field: label,
            });
        }
        Ok(())
    }

    #[cfg(test)]
    fn parse_splits(
        &mut self,
        data: &str,
        allow_word_id_ref: bool,
    ) -> DicWriteResult<(Vec<WordRef>, usize)> {
        self.parse_splits_with_asterisk(data, allow_word_id_ref, true)
    }

    fn parse_splits_with_asterisk(
        &mut self,
        data: &str,
        allow_word_id_ref: bool,
        allow_asterisk: bool,
    ) -> DicWriteResult<(Vec<WordRef>, usize)> {
        if data.is_empty() || data == "*" {
            if data == "*" && !allow_asterisk {
                return Err(BuildFailure::InvalidSplit(data.to_owned()));
            }
            return Ok((Vec::new(), 0));
        }

        parse_slash_list(data, |s| self.parse_split(s, allow_word_id_ref)).map(|splits| {
            let unresolved = splits
                .iter()
                .map(|s| match s {
                    WordRef::LineRef(_) => 1,
                    WordRef::Headword(_) => 1,
                    WordRef::Inline { .. } => 1,
                    _ => 0,
                })
                .sum();
            (splits, unresolved)
        })
    }

    fn parse_split(&mut self, data: &str, allow_word_id_ref: bool) -> DicWriteResult<WordRef> {
        if WORD_ID_LITERAL.is_match(data) {
            if !allow_word_id_ref {
                return Err(BuildFailure::InvalidSplit(data.to_owned()));
            }
            Ok(WordRef::LineRef(parse_wordid(data)?))
        } else if data.matches(',').count() == 2 {
            let mut iter = data.splitn(3, ',');
            let surface = it_next(data, &mut iter, "(1) surface", unescape)?;
            let pos = it_next(data, &mut iter, "(2) pos-id", parse_i16)?;
            let reading = it_next(data, &mut iter, "(3) reading", unescape_cow)?;
            Ok(WordRef::Inline {
                pos: pos as u16,
                reading: none_if_equal(&surface, reading),
                surface,
            })
        } else {
            let mut iter = data.splitn(8, ',');
            let surface = it_next(data, &mut iter, "(1) surface", unescape)?;
            let p1 = it_next(data, &mut iter, "(2) pos-1", unescape_cow)?;
            let p2 = it_next(data, &mut iter, "(3) pos-2", unescape_cow)?;
            let p3 = it_next(data, &mut iter, "(4) pos-3", unescape_cow)?;
            let p4 = it_next(data, &mut iter, "(5) pos-4", unescape_cow)?;
            let p5 = it_next(data, &mut iter, "(6) pos-conj-1", unescape_cow)?;
            let p6 = it_next(data, &mut iter, "(7) pos-conj-2", unescape_cow)?;
            let reading = it_next(data, &mut iter, "(8) surface", unescape_cow)?;

            let pos = self.pos_id_of([p1, p2, p3, p4, p5, p6])?;
            Ok(WordRef::Inline {
                pos,
                reading: none_if_equal(&surface, reading),
                surface,
            })
        }
    }

    pub fn write_pos_table<W: Write>(&self, w: &mut W) -> SudachiResult<usize> {
        let real_count = self.pos.len() - self.start_pos;
        w.write_all(&u16::to_le_bytes(real_count as u16))?;
        let mut written_bytes = 2;
        let mut ctx = DicCompilationCtx::default();
        ctx.set_filename("<pos-table>".to_owned());
        for (row, pos_id) in self.pos.iter() {
            if (*pos_id as usize) < self.start_pos {
                continue;
            }
            for field in row.fields() {
                ctx.apply(|| write_short_utf16(w, field).map(|written| written_bytes += written))?;
            }
            ctx.add_line(1);
        }
        Ok(written_bytes)
    }

    //noinspection DuplicatedCode
    pub(crate) fn resolve_splits<R: WordRefResolver>(
        &mut self,
        resolver: &R,
    ) -> Result<usize, (String, usize)> {
        let mut total = 0;
        let mut phantoms: Vec<RawLexiconEntry> = Vec::new();
        for line in 0..self.entries.len() {
            let current = {
                let e = &mut self.entries[line];
                std::mem::take(&mut e.norm_form)
            };
            if let Some(NormFormValue::Ref(mut norm_ref)) = current {
                match &norm_ref {
                    WordRef::Headword(headword) => {
                        if resolver.resolve(&norm_ref).is_none()
                            && !self.has_headword(headword)
                            && !phantoms.iter().any(|p| p.headword() == headword)
                        {
                            let phantom = RawLexiconEntry::make_phantom(
                                &self.entries[line],
                                headword.clone(),
                            );
                            phantoms.push(phantom);
                        }
                        self.entries[line].norm_form = Some(NormFormValue::Value(headword.clone()));
                        total += 1;
                    }
                    _ => match Self::resolve_split(&mut norm_ref, resolver) {
                        Some(val) => {
                            total += val;
                            let wid = match norm_ref {
                                WordRef::Ref(wid) => wid,
                                _ => panic!("normalized_form must be resolved to word id"),
                            };
                            let headword = match resolver.resolve_headword(wid) {
                                Some(s) => s,
                                None => {
                                    let split_info = norm_ref.format(self);
                                    return Err((split_info, line));
                                }
                            };
                            self.entries[line].norm_form =
                                if headword == self.entries[line].headword() {
                                    None
                                } else {
                                    Some(NormFormValue::Value(headword))
                                };
                        }
                        None => {
                            let split_info = norm_ref.format(self);
                            return Err((split_info, line));
                        }
                    },
                };
            } else {
                self.entries[line].norm_form = current;
            }
            let e = &mut self.entries[line];
            match Self::resolve_split(&mut e.dic_form, resolver) {
                Some(val) => total += val,
                None => {
                    let s: &WordRef = unsafe { std::mem::transmute(&e.dic_form) };
                    let split_info = s.format(self);
                    return Err((split_info, line));
                }
            }
            for s in e.splits_a.iter_mut() {
                match Self::resolve_split(s, resolver) {
                    Some(val) => total += val,
                    None => {
                        // at this point s is a read only borrow,
                        // but borrow checker does not allow to do this cleanly
                        // self conflicts with splits_a borrow
                        let s: &WordRef = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
            for s in e.splits_b.iter_mut() {
                match Self::resolve_split(s, resolver) {
                    Some(val) => total += val,
                    None => {
                        // at this point s is a read only borrow,
                        // but borrow checker does not allow to do this cleanly
                        // self conflicts with splits_b borrow
                        let s: &WordRef = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
            for s in e.splits_c.iter_mut() {
                match Self::resolve_split(s, resolver) {
                    Some(val) => total += val,
                    None => {
                        let s: &WordRef = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
            for s in e.word_structure.iter_mut() {
                match Self::resolve_split(s, resolver) {
                    Some(val) => total += val,
                    None => {
                        let s: &WordRef = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
        }
        self.entries.extend(phantoms);
        Ok(total)
    }

    fn has_headword(&self, headword: &str) -> bool {
        self.entries.iter().any(|e| e.headword() == headword)
    }

    fn resolve_split<R: WordRefResolver>(unit: &mut WordRef, resolver: &R) -> Option<usize> {
        match unit {
            WordRef::Ref(_) => Some(0),
            WordRef::SelfRef => Some(0),
            _ => {
                let wid = resolver.resolve(&*unit)?;
                *unit = WordRef::Ref(wid);
                Some(1)
            }
        }
    }

    pub(crate) fn row_word_ids(&self, dic_id: u8) -> Vec<WordId> {
        let mut result = Vec::with_capacity(self.entries.len());
        let mut offset = Self::ENTRY_INITIAL_OFFSET;
        for e in &self.entries {
            let entry_id = (offset >> WordInfos::WORD_ID_ALIGNMENT_BITS) as u32;
            result.push(WordId::new(dic_id, entry_id));
            offset += e.expected_entry_size();
        }
        result
    }

    pub(crate) fn max_entry_id_plus_one(&self) -> u32 {
        let mut offset = Self::ENTRY_INITIAL_OFFSET;
        for e in &self.entries {
            offset += e.expected_entry_size();
        }
        (offset >> WordInfos::WORD_ID_ALIGNMENT_BITS) as u32
    }

    fn parse_dic_form(
        &mut self,
        data: &str,
        allow_word_id_ref: bool,
        headword: &str,
        pos: u16,
        reading: &str,
    ) -> DicWriteResult<(WordRef, usize)> {
        if data.is_empty() || (allow_word_id_ref && data == "*") {
            return Ok((WordRef::SelfRef, 0));
        }
        if data == "*" {
            return Err(BuildFailure::InvalidSplit(data.to_owned()));
        }

        let parsed = self.parse_split(data, allow_word_id_ref)?;
        if let WordRef::Inline {
            surface,
            pos: p,
            reading: r,
        } = &parsed
        {
            let own_reading = if headword == reading {
                None
            } else {
                Some(reading)
            };
            if surface == headword && *p == pos && r.as_deref() == own_reading {
                return Ok((WordRef::SelfRef, 0));
            }
        }

        let unresolved = match parsed {
            WordRef::Ref(_) => 0,
            WordRef::SelfRef => 0,
            WordRef::LineRef(_) => 1,
            WordRef::Headword(_) => 1,
            WordRef::Inline { .. } => 1,
        };
        Ok((parsed, unresolved))
    }

    fn parse_norm_form(
        &mut self,
        data: &str,
        headword: &str,
    ) -> DicWriteResult<(Option<NormFormValue>, usize)> {
        if data.is_empty() || data == "*" {
            return Ok((None, 0));
        }

        if data.matches(',').count() == 2 || data.matches(',').count() == 7 {
            let parsed = self.parse_split(data, false)?;
            return Ok((Some(NormFormValue::Ref(parsed)), 1));
        }

        let normalized = unescape(data)?;
        if normalized == headword {
            Ok((None, 0))
        } else {
            Ok((Some(NormFormValue::Ref(WordRef::Headword(normalized))), 1))
        }
    }
}

fn write_short_utf16<W: Write>(w: &mut W, data: &str) -> DicWriteResult<usize> {
    let utf16: Vec<u16> = data.encode_utf16().collect();
    if utf16.len() > i16::MAX as usize {
        return Err(BuildFailure::InvalidSize {
            actual: utf16.len(),
            expected: i16::MAX as usize,
        });
    }
    let len = utf16.len() as i16;
    w.write_all(&len.to_le_bytes())?;
    let mut written = 2;
    for c in utf16 {
        w.write_all(&c.to_le_bytes())?;
        written += 2;
    }
    Ok(written)
}

pub struct LexiconWriter<'a> {
    entries: &'a [RawLexiconEntry],
    strings: &'a StringStore,
    user: bool,
    reporter: &'a mut Reporter,
}

impl<'a> LexiconWriter<'a> {
    pub(crate) fn new(
        entries: &'a [RawLexiconEntry],
        strings: &'a StringStore,
        user: bool,
        reporter: &'a mut Reporter,
    ) -> Self {
        Self {
            entries,
            strings,
            user,
            reporter,
        }
    }

    pub fn write<W: Write>(&mut self, w: &mut W) -> SudachiResult<usize> {
        let mut ctx = DicCompilationCtx::memory();
        ctx.set_filename("<write entries>".to_owned());
        let mut total = LexiconReader::ENTRY_INITIAL_OFFSET;
        w.write_all(&[0u8; LexiconReader::ENTRY_INITIAL_OFFSET])?;

        let rep = ReportBuilder::new("entries");
        let mut offset = LexiconReader::ENTRY_INITIAL_OFFSET;
        let self_dic_id = if self.user { 1 } else { 0 };
        let mut headword_to_id = HashMap::with_capacity(self.entries.len());
        for e in self.entries {
            let entry_id = (offset >> WordInfos::WORD_ID_ALIGNMENT_BITS) as u32;
            let wid = WordId::new(self_dic_id, entry_id);
            headword_to_id.insert(e.headword().to_owned(), wid);
            offset += e.expected_entry_size();
        }

        ctx.set_line(0);
        let mut offset = LexiconReader::ENTRY_INITIAL_OFFSET;
        for e in self.entries {
            let entry_id = (offset >> WordInfos::WORD_ID_ALIGNMENT_BITS) as u32;
            let self_word_id = WordId::new(self_dic_id, entry_id);
            let norm_form_word_id = match e.norm_form.as_ref() {
                None => self_word_id,
                Some(NormFormValue::Value(headword)) => {
                    if headword == e.headword() {
                        self_word_id
                    } else if let Some(wid) = headword_to_id.get(headword) {
                        *wid
                    } else {
                        return ctx.err(BuildFailure::InvalidSplitWordReference(format!(
                            "normalized_form headword not found: {}",
                            headword
                        )));
                    }
                }
                Some(NormFormValue::Ref(_)) => {
                    panic!("normalized_form must be resolved before writing");
                }
            };
            let headword_strptr = self.strings.resolve(e.headword());
            let reading_strptr = self.strings.resolve(e.reading());
            total += ctx.transform(e.write_params(w))?;
            total += ctx.transform(e.write_rest(
                w,
                self_word_id,
                norm_form_word_id,
                headword_strptr,
                reading_strptr,
            ))?;
            let expected_end = offset + e.expected_entry_size();
            if total > expected_end {
                return ctx.err(BuildFailure::InvalidSize {
                    actual: total,
                    expected: expected_end,
                });
            }
            while total < expected_end {
                w.write_all(&[0])?;
                total += 1;
            }
            offset = expected_end;
            ctx.add_line(1);
        }
        self.reporter.collect(total, rep);
        Ok(total)
    }
}
