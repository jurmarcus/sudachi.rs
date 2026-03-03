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
    it_next, none_if_equal, parse_i16, parse_mode, parse_slash_list,
    parse_u32_list, parse_wordid, unescape, unescape_cow, WORD_ID_LITERAL,
};
use crate::dic::build::primitives::{write_u32_array, Utf16Writer};
use crate::dic::build::report::{ReportBuilder, Reporter};
use crate::dic::build::MAX_POS_IDS;
use crate::dic::grammar::Grammar;
use crate::dic::pos::POS_DEPTH;
use crate::dic::word_id::WordId;
use crate::error::SudachiResult;

#[cfg(test)]
mod test;

#[cfg(test)]
mod wordinfo_test;

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

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum SplitUnit {
    Ref(WordId),
    Inline {
        surface: String,
        pos: u16,
        reading: Option<String>,
    },
}

#[repr(usize)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Column {
    IndexForm = 0,
    LeftId = 1,
    RightId = 2,
    Cost = 3,
    Headword = 4,
    Pos1 = 5,
    Pos2 = 6,
    Pos3 = 7,
    Pos4 = 8,
    Pos5 = 9,
    Pos6 = 10,
    ReadingForm = 11,
    NormalizedForm = 12,
    DictionaryForm = 13,
    Mode = 14,
    SplitA = 15,
    SplitB = 16,
    WordStructure = 17,
    SynonymGroups = 18,
    SplitC = 19,
    UserData = 20,
    PosId = 21,
}

const NUM_COLUMNS: usize = 22;

impl Column {
    const fn as_usize(self) -> usize {
        self as usize
    }

    const fn legacy_index(self) -> usize {
        self as usize
    }

    const fn is_required(self) -> bool {
        matches!(
            self,
            Column::IndexForm
                | Column::LeftId
                | Column::RightId
                | Column::Cost
                | Column::ReadingForm
                | Column::NormalizedForm
                | Column::DictionaryForm
                | Column::SplitA
                | Column::SplitB
                | Column::WordStructure
        )
    }

    const fn label(self) -> &'static str {
        match self {
            Column::IndexForm => "INDEX_FORM",
            Column::LeftId => "LEFT_ID",
            Column::RightId => "RIGHT_ID",
            Column::Cost => "COST",
            Column::Headword => "HEADWORD",
            Column::Pos1 => "POS1",
            Column::Pos2 => "POS2",
            Column::Pos3 => "POS3",
            Column::Pos4 => "POS4",
            Column::Pos5 => "POS5",
            Column::Pos6 => "POS6",
            Column::ReadingForm => "READING_FORM",
            Column::NormalizedForm => "NORMALIZED_FORM",
            Column::DictionaryForm => "DICTIONARY_FORM",
            Column::Mode => "MODE",
            Column::SplitA => "SPLIT_A",
            Column::SplitB => "SPLIT_B",
            Column::WordStructure => "WORD_STRUCTURE",
            Column::SynonymGroups => "SYNONYM_GROUPS",
            Column::SplitC => "SPLIT_C",
            Column::UserData => "USER_DATA",
            Column::PosId => "POS_ID",
        }
    }

    fn from_str(data: &str) -> Option<Self> {
        let mut normalized = String::with_capacity(data.len());
        for c in data.chars() {
            if c != '_' {
                normalized.push(c.to_ascii_lowercase());
            }
        }

        match normalized.as_str() {
            "indexform" => Some(Column::IndexForm),
            "leftid" => Some(Column::LeftId),
            "rightid" => Some(Column::RightId),
            "cost" => Some(Column::Cost),
            "headword" => Some(Column::Headword),
            "pos1" => Some(Column::Pos1),
            "pos2" => Some(Column::Pos2),
            "pos3" => Some(Column::Pos3),
            "pos4" => Some(Column::Pos4),
            "pos5" => Some(Column::Pos5),
            "pos6" => Some(Column::Pos6),
            "readingform" => Some(Column::ReadingForm),
            "normalizedform" => Some(Column::NormalizedForm),
            "dictionaryform" => Some(Column::DictionaryForm),
            "mode" => Some(Column::Mode),
            "splita" => Some(Column::SplitA),
            "splitb" => Some(Column::SplitB),
            "wordstructure" => Some(Column::WordStructure),
            "synonymgroups" => Some(Column::SynonymGroups),
            "splitc" => Some(Column::SplitC),
            "userdata" => Some(Column::UserData),
            "posid" => Some(Column::PosId),
            _ => None,
        }
    }
}

const POS_PARTS: [Column; POS_DEPTH] = [
    Column::Pos1,
    Column::Pos2,
    Column::Pos3,
    Column::Pos4,
    Column::Pos5,
    Column::Pos6,
];

const ALL_COLUMNS: [Column; NUM_COLUMNS] = [
    Column::IndexForm,
    Column::LeftId,
    Column::RightId,
    Column::Cost,
    Column::Headword,
    Column::Pos1,
    Column::Pos2,
    Column::Pos3,
    Column::Pos4,
    Column::Pos5,
    Column::Pos6,
    Column::ReadingForm,
    Column::NormalizedForm,
    Column::DictionaryForm,
    Column::Mode,
    Column::SplitA,
    Column::SplitB,
    Column::WordStructure,
    Column::SynonymGroups,
    Column::SplitC,
    Column::UserData,
    Column::PosId,
];

#[derive(Copy, Clone)]
enum ColumnLayout {
    Legacy,
    Header([i16; NUM_COLUMNS]),
}

impl ColumnLayout {
    fn from_record(record: &StringRecord, ctx: &DicCompilationCtx) -> SudachiResult<(Self, bool)> {
        if record.len() > 1 {
            if let Some(left_id) = record.get(Column::LeftId.legacy_index()) {
                if parse_i16(left_id).is_ok() {
                    return Ok((ColumnLayout::Legacy, false));
                }
            }
        }

        let mut mapping = [-1_i16; NUM_COLUMNS];
        for (idx, field) in record.iter().enumerate() {
            let col = match Column::from_str(field) {
                Some(c) => c,
                None => return ctx.err(BuildFailure::NoRawField("INVALID_COLUMN_NAME")),
            };
            let prev = &mut mapping[col.as_usize()];
            if *prev >= 0 {
                return ctx.err(BuildFailure::NoRawField("DUPLICATED_COLUMN_NAME"));
            }
            *prev = idx as i16;
        }

        for col in ALL_COLUMNS {
            if col.is_required() && mapping[col.as_usize()] < 0 {
                return ctx.err(BuildFailure::NoRawField(col.label()));
            }
        }

        let pos_parts_found = POS_PARTS
            .iter()
            .filter(|col| mapping[col.as_usize()] >= 0)
            .count();
        if pos_parts_found != 0 && pos_parts_found != POS_DEPTH {
            return ctx.err(BuildFailure::NoRawField("POS1_6_SET"));
        }

        let has_pos_id = mapping[Column::PosId.as_usize()] >= 0;
        if !has_pos_id && pos_parts_found == 0 {
            return ctx.err(BuildFailure::NoRawField("POS_OR_POS_ID"));
        }

        Ok((ColumnLayout::Header(mapping), true))
    }

    fn index(self, col: Column) -> Option<usize> {
        match self {
            ColumnLayout::Legacy => Some(col.legacy_index()),
            ColumnLayout::Header(mapping) => {
                let idx = mapping[col.as_usize()];
                if idx < 0 {
                    None
                } else {
                    Some(idx as usize)
                }
            }
        }
    }

    const fn is_legacy(self) -> bool {
        matches!(self, ColumnLayout::Legacy)
    }
}

impl SplitUnit {
    pub fn format(&self, lexicon: &LexiconReader) -> String {
        match self {
            SplitUnit::Ref(id) => id.as_raw().to_string(),
            SplitUnit::Inline {
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

pub(crate) trait SplitUnitResolver {
    fn resolve(&self, unit: &SplitUnit) -> Option<WordId> {
        match unit {
            SplitUnit::Ref(wid) => Some(*wid),
            SplitUnit::Inline {
                surface,
                pos,
                reading,
            } => self.resolve_inline(surface, *pos, reading.as_deref()),
        }
    }

    fn resolve_inline(&self, surface: &str, pos: u16, reading: Option<&str>) -> Option<WordId>;
}

pub(crate) struct RawLexiconEntry {
    pub left_id: i16,
    pub right_id: i16,
    pub cost: i16,
    pub surface: String,
    pub headword: Option<String>,
    pub dic_form: SplitUnit,
    pub norm_form: Option<String>,
    pub pos: u16,
    pub splits_a: Vec<SplitUnit>,
    pub splits_b: Vec<SplitUnit>,
    pub reading: Option<String>,
    #[allow(unused)]
    pub splitting: Mode,
    pub word_structure: Vec<SplitUnit>,
    pub synonym_groups: Vec<u32>,
}

impl RawLexiconEntry {
    pub fn surface(&self) -> &str {
        &self.surface
    }

    pub fn headword(&self) -> &str {
        self.headword.as_deref().unwrap_or_else(|| self.surface())
    }

    pub fn norm_form(&self) -> &str {
        self.norm_form.as_deref().unwrap_or_else(|| self.headword())
    }

    pub fn reading(&self) -> &str {
        self.reading.as_deref().unwrap_or_else(|| self.headword())
    }

    pub fn should_index(&self) -> bool {
        self.left_id >= 0
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
            SplitUnit::Ref(wid) => wid,
            SplitUnit::Inline { .. } => panic!("dictionary_form must be resolved before writing"),
        };
        w.write_all(&dic_form.as_raw().to_le_bytes())?;
        size += 4;
        size += u16w.write_empty_if_equal(w, self.reading(), self.headword())?;
        size += write_u32_array(w, &self.splits_a)?;
        size += write_u32_array(w, &self.splits_b)?;
        let mut ws = Vec::with_capacity(self.word_structure.len());
        for s in self.word_structure.iter() {
            match s {
                SplitUnit::Ref(wid) => ws.push(*wid),
                _ => panic!("word_structure refs must be resolved before writing"),
            }
        }
        size += write_u32_array(w, &ws)?;
        size += write_u32_array(w, &self.synonym_groups)?;

        Ok(size)
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
        let normalized = rec.get_col(layout, Column::NormalizedForm, unescape_cow)?;
        let dic_form_ref =
            rec.get_col_or(layout, Column::DictionaryForm, "".to_owned(), |s| Ok(s.to_owned()))?;
        let splitting = rec.get_col(layout, Column::Mode, parse_mode)?;
        let allow_word_id_ref = layout.is_legacy();
        let (split_a, resolve_a) = rec.get_col(layout, Column::SplitA, |s| {
            self.parse_splits(s, allow_word_id_ref)
        })?;
        let (split_b, resolve_b) = rec.get_col(layout, Column::SplitB, |s| {
            self.parse_splits(s, allow_word_id_ref)
        })?;
        let (parts, resolve_parts) =
            rec.get_col(layout, Column::WordStructure, |s| self.parse_splits(s, allow_word_id_ref))?;
        let synonyms = rec.get_col_or_default(layout, Column::SynonymGroups, parse_u32_list)?;
        let pos_id = rec.get_col_or(layout, Column::PosId, -1_i16, |s| {
            if s.is_empty() {
                Ok(-1)
            } else {
                parse_i16(s)
            }
        })?;

        let pos = if !p1.is_empty() {
            let pos = rec.ctx.transform(self.pos_of([p1, p2, p3, p4, p5, p6]))?;
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

        let (dic_form, resolve_dic_form) = rec.ctx.transform(self.parse_dic_form(
            &dic_form_ref,
            allow_word_id_ref,
            &headword,
            pos,
            reading.as_ref(),
        ))?;
        self.unresolved += resolve_a + resolve_b + resolve_parts + resolve_dic_form;

        if index_form.is_empty() {
            return rec.ctx.err(BuildFailure::EmptySurface);
        }

        self.ctx = rec.ctx;

        let entry = RawLexiconEntry {
            left_id,
            right_id,
            cost,
            dic_form,
            norm_form: none_if_equal(&headword, normalized),
            reading: none_if_equal(&headword, reading),
            headword: none_if_equal(&index_form, headword),
            surface: index_form,
            pos,
            splitting,
            splits_a: split_a,
            splits_b: split_b,
            word_structure: parts,
            synonym_groups: synonyms,
        };

        Ok(entry)
    }

    fn pos_of(&mut self, data: [Cow<str>; POS_DEPTH]) -> DicWriteResult<u16> {
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
        let (max_0, max_1) = match self.num_system {
            // means that we compile system dictionary, there must not be user words
            usize::MAX => (self.entries.len(), 0),
            // compiling user dictionary
            x => (x, self.entries.len()),
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
                SplitUnit::Ref(wid) => {
                    if wid != WordId::INVALID {
                        ctx.transform(Self::validate_wid(wid, max_0, max_1, "dic_form"))?;
                    }
                }
                _ => panic!("at this point dictionary_form must be resolved"),
            }

            for s in e.splits_a.iter() {
                match s {
                    SplitUnit::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "splits_a"))?;
                    }
                    _ => panic!("at this point there must not be unresolved splits"),
                }
            }

            for s in e.splits_b.iter() {
                match s {
                    SplitUnit::Ref(wid) => {
                        ctx.transform(Self::validate_wid(*wid, max_0, max_1, "splits_b"))?;
                    }
                    _ => panic!("at this point there must not be unresolved splits"),
                }
            }

            for wid in e.word_structure.iter() {
                match wid {
                    SplitUnit::Ref(wid) => {
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

    fn parse_splits(
        &mut self,
        data: &str,
        allow_word_id_ref: bool,
    ) -> DicWriteResult<(Vec<SplitUnit>, usize)> {
        if data.is_empty() || data == "*" {
            return Ok((Vec::new(), 0));
        }

        parse_slash_list(data, |s| self.parse_split(s, allow_word_id_ref)).map(|splits| {
            let unresolved = splits
                .iter()
                .map(|s| match s {
                    SplitUnit::Inline { .. } => 1,
                    _ => 0,
                })
                .sum();
            (splits, unresolved)
        })
    }

    fn parse_split(&mut self, data: &str, allow_word_id_ref: bool) -> DicWriteResult<SplitUnit> {
        if WORD_ID_LITERAL.is_match(data) {
            if !allow_word_id_ref {
                return Err(BuildFailure::InvalidSplit(data.to_owned()));
            }
            Ok(SplitUnit::Ref(parse_wordid(data)?))
        } else if data.matches(',').count() == 2 {
            let mut iter = data.splitn(3, ',');
            let surface = it_next(data, &mut iter, "(1) surface", unescape)?;
            let pos = it_next(data, &mut iter, "(2) pos-id", parse_i16)?;
            let reading = it_next(data, &mut iter, "(3) reading", unescape_cow)?;
            Ok(SplitUnit::Inline {
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

            let pos = self.pos_of([p1, p2, p3, p4, p5, p6])?;
            Ok(SplitUnit::Inline {
                pos,
                reading: none_if_equal(&surface, reading),
                surface,
            })
        }
    }

    pub fn write_pos_table<W: Write>(&self, w: &mut W) -> SudachiResult<usize> {
        let mut u16w = Utf16Writer::new();
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
                ctx.apply(|| u16w.write(w, field).map(|written| written_bytes += written))?;
            }
            ctx.add_line(1);
        }
        Ok(written_bytes)
    }

    //noinspection DuplicatedCode
    pub(crate) fn resolve_splits<R: SplitUnitResolver>(
        &mut self,
        resolver: &R,
    ) -> Result<usize, (String, usize)> {
        let mut total = 0;
        for (line, e) in self.entries.iter_mut().enumerate() {
            match Self::resolve_split(&mut e.dic_form, resolver) {
                Some(val) => total += val,
                None => {
                    let s: &SplitUnit = unsafe { std::mem::transmute(&e.dic_form) };
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
                        let s: &SplitUnit = unsafe { std::mem::transmute(&*s) };
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
                        let s: &SplitUnit = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
            for s in e.word_structure.iter_mut() {
                match Self::resolve_split(s, resolver) {
                    Some(val) => total += val,
                    None => {
                        let s: &SplitUnit = unsafe { std::mem::transmute(&*s) };
                        let split_info = s.format(self);
                        return Err((split_info, line));
                    }
                }
            }
        }
        Ok(total)
    }

    fn resolve_split<R: SplitUnitResolver>(unit: &mut SplitUnit, resolver: &R) -> Option<usize> {
        match unit {
            SplitUnit::Ref(_) => Some(0),
            _ => {
                let wid = resolver.resolve(&*unit)?;
                *unit = SplitUnit::Ref(wid);
                Some(1)
            }
        }
    }

    fn parse_dic_form(
        &mut self,
        data: &str,
        allow_word_id_ref: bool,
        headword: &str,
        pos: u16,
        reading: &str,
    ) -> DicWriteResult<(SplitUnit, usize)> {
        if data.is_empty() || data == "*" {
            return Ok((SplitUnit::Ref(WordId::INVALID), 0));
        }

        let parsed = self.parse_split(data, allow_word_id_ref)?;
        if let SplitUnit::Inline {
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
                return Ok((SplitUnit::Ref(WordId::INVALID), 0));
            }
        }

        let unresolved = match parsed {
            SplitUnit::Ref(_) => 0,
            SplitUnit::Inline { .. } => 1,
        };
        Ok((parsed, unresolved))
    }
}

struct RecordWrapper<'a> {
    pub record: &'a StringRecord,
    pub ctx: DicCompilationCtx,
}

impl<'a> RecordWrapper<'a> {
    #[inline(always)]
    fn get_col<T, F>(&self, layout: ColumnLayout, col: Column, f: F) -> SudachiResult<T>
    where
        F: FnOnce(&'a str) -> DicWriteResult<T>,
    {
        match layout.index(col).and_then(|idx| self.record.get(idx)) {
            Some(s) => self.ctx.transform(f(s)),
            None => self.ctx.err(BuildFailure::NoRawField(col.label())),
        }
    }

    #[inline(always)]
    fn get_col_or_empty<T, F>(&self, layout: ColumnLayout, col: Column, f: F) -> SudachiResult<T>
    where
        F: FnOnce(&'a str) -> DicWriteResult<T>,
    {
        match layout.index(col).and_then(|idx| self.record.get(idx)) {
            Some(s) => self.ctx.transform(f(s)),
            None => self.ctx.transform(f("")),
        }
    }

    #[inline(always)]
    fn get_col_or_default<T, F>(&self, layout: ColumnLayout, col: Column, f: F) -> SudachiResult<T>
    where
        F: FnOnce(&'a str) -> DicWriteResult<T>,
        T: Default,
    {
        self.get_col_or(layout, col, T::default(), f)
    }

    #[inline(always)]
    fn get_col_or<T, F>(
        &self,
        layout: ColumnLayout,
        col: Column,
        default: T,
        f: F,
    ) -> SudachiResult<T>
    where
        F: FnOnce(&'a str) -> DicWriteResult<T>,
    {
        match layout.index(col).and_then(|idx| self.record.get(idx)) {
            Some(s) => self.ctx.transform(f(s)),
            None => Ok(default),
        }
    }
}

pub struct LexiconWriter<'a> {
    entries: &'a [RawLexiconEntry],
    u16: Utf16Writer,
    buffer: Vec<u8>,
    offset: usize,
    reporter: &'a mut Reporter,
}

impl<'a> LexiconWriter<'a> {
    pub(crate) fn new(
        entries: &'a [RawLexiconEntry],
        offset: usize,
        reporter: &'a mut Reporter,
    ) -> Self {
        Self {
            buffer: Vec::with_capacity(entries.len() * 32),
            entries,
            u16: Utf16Writer::new(),
            offset,
            reporter,
        }
    }

    pub fn write<W: Write>(&mut self, w: &mut W) -> SudachiResult<usize> {
        let mut ctx = DicCompilationCtx::memory();
        ctx.set_filename("<write entries>".to_owned());
        let mut total = 4;

        let num_entries = self.entries.len() as u32;
        w.write_all(&num_entries.to_le_bytes())?;

        let rep = ReportBuilder::new("word_params");
        ctx.set_line(0);
        for e in self.entries {
            total += ctx.transform(e.write_params(w))?;
            ctx.add_line(1);
        }
        self.reporter.collect(total, rep);
        let start = total;

        let rep = ReportBuilder::new("wordinfo_offsets");
        ctx.set_line(0);
        let offset_base = self.offset + (6 + 4) * self.entries.len() + 4;
        let mut word_offset = 0;
        for e in self.entries {
            let u32_offset = (offset_base + word_offset) as u32;
            w.write_all(&u32_offset.to_le_bytes())?;
            let size = ctx.transform(e.write_word_info(&mut self.u16, &mut self.buffer))?;
            word_offset += size;
            total += 4;
            ctx.add_line(1);
        }
        self.reporter.collect(total - start, rep);

        let rep = ReportBuilder::new("wordinfos (copy only)");
        let info_size = self.buffer.len();
        w.write_all(&self.buffer)?;
        self.reporter.collect(info_size, rep);

        Ok(total + info_size)
    }
}
