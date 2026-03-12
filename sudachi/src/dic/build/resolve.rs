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

use crate::dic::build::lexicon::{ParsedLexiconEntry, ResolvedLexiconEntry, WordRefResolver};
use crate::dic::subset::InfoSubset;
use crate::dic::word_id::WordId;
use crate::dic::word_info::WordInfo;
use crate::dic::DictionaryAccess;
use crate::error::SudachiResult;
use crate::util::fxhash::FxBuildHasher;
use std::collections::HashMap;

// HashMap from surface to (pos_id, reading_form, word-id)s
type ResolutionCandidateMap<T> = HashMap<T, Vec<(u16, Option<T>, WordId)>, FxBuildHasher>;

pub(crate) trait ResolverEntryView {
    fn headword(&self) -> &str;
    fn reading(&self) -> &str;
    fn pos_id(&self) -> u16;
}

impl ResolverEntryView for ParsedLexiconEntry {
    fn headword(&self) -> &str {
        self.headword()
    }

    fn reading(&self) -> &str {
        self.reading()
    }

    fn pos_id(&self) -> u16 {
        self.pos
    }
}

impl ResolverEntryView for ResolvedLexiconEntry {
    fn headword(&self) -> &str {
        self.headword()
    }

    fn reading(&self) -> &str {
        self.reading()
    }

    fn pos_id(&self) -> u16 {
        self.pos
    }
}

/// We can't use trie to resolve splits because it is possible that refs are not in trie
/// This resolver has to be owning because the dictionary content is lazily loaded and transient
pub struct BinDictResolver {
    index: ResolutionCandidateMap<String>,
    headwords: HashMap<WordId, String, FxBuildHasher>,
    line_to_wid: Vec<WordId>,
}

impl BinDictResolver {
    pub fn new<D: DictionaryAccess>(dict: D) -> SudachiResult<Self> {
        let lex = dict.lexicon();
        let line_to_wid = lex.system_word_ids_in_order();
        let mut index: ResolutionCandidateMap<String> = HashMap::default();
        let mut headwords: HashMap<WordId, String, FxBuildHasher> = HashMap::default();
        for wid in line_to_wid.iter().copied() {
            let winfo: WordInfo = lex.get_word_info_subset(
                wid,
                InfoSubset::HEADWORD | InfoSubset::READING_FORM | InfoSubset::POS_ID,
            )?;
            let headword = winfo.headword(&dict).to_string();
            let reading = winfo.reading_form(&dict).to_string();
            let pos_id = winfo.pos_id();

            let rdfield = if reading.is_empty() || headword == reading {
                None
            } else {
                Some(reading)
            };

            index
                .entry(headword.clone())
                .or_default()
                .push((pos_id, rdfield, wid));
            headwords.insert(wid, headword);
        }

        Ok(Self {
            index,
            headwords,
            line_to_wid,
        })
    }
}

impl WordRefResolver for BinDictResolver {
    fn resolve_by_line_ref(&self, line_ref: WordId) -> Option<WordId> {
        if line_ref.dict().as_raw() != 0 {
            return None;
        }
        self.line_to_wid
            .get(line_ref.entry().as_raw() as usize)
            .copied()
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordId> {
        self.index
            .get(headword)
            .and_then(|v| v.first().map(|(_, _, wid)| *wid))
    }

    fn resolve_inline(&self, surface: &str, pos: u16, reading: Option<&str>) -> Option<WordId> {
        self.index.get(surface).and_then(|v| {
            for (p, rd, wid) in v {
                if *p == pos && reading.eq(&rd.as_deref()) {
                    return Some(*wid);
                }
            }
            None
        })
    }

    fn resolve_headword(&self, wid: WordId) -> Option<String> {
        self.headwords.get(&wid).cloned()
    }
}

pub struct RawDictResolver {
    data: ResolutionCandidateMap<String>,
    headwords: Vec<String>,
    line_to_wid: Vec<WordId>,
    dic_id: u8,
}

impl RawDictResolver {
    pub(crate) fn new<T: ResolverEntryView>(
        entries: &[T],
        line_to_wid: Vec<WordId>,
        user: bool,
    ) -> Self {
        let mut data: ResolutionCandidateMap<String> = HashMap::default();
        let mut headwords = Vec::with_capacity(entries.len());

        let dic_id = if user { 1 } else { 0 };

        for (i, e) in entries.iter().enumerate() {
            let surface = e.headword().to_owned();
            let reading = e.reading().to_owned();
            let wid = line_to_wid[i];
            headwords.push(surface.clone());

            let read_opt = if e.headword() == reading {
                None
            } else {
                Some(reading)
            };

            data.entry(surface)
                .or_default()
                .push((e.pos_id(), read_opt, wid));
        }

        Self {
            data,
            headwords,
            line_to_wid,
            dic_id,
        }
    }
}

impl WordRefResolver for RawDictResolver {
    fn resolve_by_line_ref(&self, line_ref: WordId) -> Option<WordId> {
        if line_ref.dict().as_raw() != self.dic_id {
            return None;
        }
        self.line_to_wid
            .get(line_ref.entry().as_raw() as usize)
            .copied()
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordId> {
        self.data
            .get(headword)
            .and_then(|v| v.first().map(|(_, _, wid)| *wid))
    }

    fn resolve_inline(&self, surface: &str, pos: u16, reading: Option<&str>) -> Option<WordId> {
        self.data.get(surface).and_then(|data| {
            for (p, rd, wid) in data {
                if *p == pos && rd.as_deref() == reading {
                    return Some(*wid);
                }
            }
            None
        })
    }

    fn resolve_headword(&self, wid: WordId) -> Option<String> {
        if wid.dict().as_raw() != self.dic_id {
            return None;
        }
        self.line_to_wid
            .iter()
            .position(|candidate| candidate == &wid)
            .and_then(|idx| self.headwords.get(idx))
            .cloned()
    }
}

pub(crate) struct ChainedResolver<A, B> {
    a: A,
    b: B,
}

impl<A: WordRefResolver, B: WordRefResolver> ChainedResolver<A, B> {
    pub(crate) fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<A: WordRefResolver, B: WordRefResolver> WordRefResolver for ChainedResolver<A, B> {
    fn resolve_by_line_ref(&self, line_ref: WordId) -> Option<WordId> {
        self.a
            .resolve_by_line_ref(line_ref)
            .or_else(|| self.b.resolve_by_line_ref(line_ref))
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordId> {
        self.a
            .resolve_by_headword(headword)
            .or_else(|| self.b.resolve_by_headword(headword))
    }

    fn resolve_inline(&self, surface: &str, pos: u16, reading: Option<&str>) -> Option<WordId> {
        self.a
            .resolve_inline(surface, pos, reading)
            .or_else(|| self.b.resolve_inline(surface, pos, reading))
    }

    fn resolve_headword(&self, wid: WordId) -> Option<String> {
        self.a
            .resolve_headword(wid)
            .or_else(|| self.b.resolve_headword(wid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dic::build::lexicon::WordRef;

    struct TestEntry {
        headword: &'static str,
        reading: &'static str,
        pos_id: u16,
    }

    impl ResolverEntryView for TestEntry {
        fn headword(&self) -> &str {
            self.headword
        }

        fn reading(&self) -> &str {
            self.reading
        }

        fn pos_id(&self) -> u16 {
            self.pos_id
        }
    }

    struct StubResolver {
        by_line_ref: Option<WordId>,
        by_headword: Option<WordId>,
        by_inline: Option<WordId>,
    }

    impl WordRefResolver for StubResolver {
        fn resolve_by_line_ref(&self, _line_ref: WordId) -> Option<WordId> {
            self.by_line_ref
        }

        fn resolve_by_headword(&self, _headword: &str) -> Option<WordId> {
            self.by_headword
        }

        fn resolve_inline(
            &self,
            _surface: &str,
            _pos: u16,
            _reading: Option<&str>,
        ) -> Option<WordId> {
            self.by_inline
        }
    }

    #[test]
    fn chained_resolver_prioritizes_first_resolver() {
        let a = StubResolver {
            by_line_ref: Some(WordId::new(0, 3)),
            by_headword: Some(WordId::new(0, 1)),
            by_inline: Some(WordId::new(0, 2)),
        };
        let b = StubResolver {
            by_line_ref: Some(WordId::new(1, 3)),
            by_headword: Some(WordId::new(1, 1)),
            by_inline: Some(WordId::new(1, 2)),
        };
        let chained = ChainedResolver::new(a, b);
        assert_eq!(
            chained.resolve(&WordRef::LineRef(WordId::new(0, 0))),
            Some(WordId::new(0, 3))
        );
        assert_eq!(
            chained.resolve(&WordRef::Headword("京都".to_string())),
            Some(WordId::new(0, 1))
        );
        assert_eq!(
            chained.resolve(&WordRef::Inline {
                surface: "京都".to_string(),
                pos: 0,
                reading: None,
            }),
            Some(WordId::new(0, 2))
        );
    }

    #[test]
    fn raw_resolver_resolves_inline_to_first_duplicate_in_csv_order() {
        let entries = vec![
            TestEntry {
                headword: "京都",
                reading: "キョウト",
                pos_id: 0,
            },
            TestEntry {
                headword: "京都",
                reading: "キョウト",
                pos_id: 0,
            },
            TestEntry {
                headword: "東京",
                reading: "トウキョウ",
                pos_id: 0,
            },
        ];
        let line_to_wid = vec![WordId::new(0, 11), WordId::new(0, 27), WordId::new(0, 42)];
        let resolver = RawDictResolver::new(&entries, line_to_wid.clone(), false);

        assert_eq!(
            resolver.resolve_inline("京都", 0, Some("キョウト")),
            Some(line_to_wid[0])
        );
    }

    #[test]
    fn raw_resolver_resolves_headword_to_first_duplicate_in_csv_order() {
        let entries = vec![
            TestEntry {
                headword: "京都",
                reading: "キョウト",
                pos_id: 0,
            },
            TestEntry {
                headword: "京都",
                reading: "キョート",
                pos_id: 1,
            },
            TestEntry {
                headword: "東京",
                reading: "トウキョウ",
                pos_id: 0,
            },
        ];
        let line_to_wid = vec![WordId::new(0, 11), WordId::new(0, 27), WordId::new(0, 42)];
        let resolver = RawDictResolver::new(&entries, line_to_wid.clone(), false);

        assert_eq!(resolver.resolve_by_headword("京都"), Some(line_to_wid[0]));
    }

    #[test]
    fn raw_resolver_resolves_line_refs_in_csv_order() {
        let entries = vec![
            TestEntry {
                headword: "京都",
                reading: "キョウト",
                pos_id: 0,
            },
            TestEntry {
                headword: "京都",
                reading: "キョウト",
                pos_id: 0,
            },
            TestEntry {
                headword: "東京",
                reading: "トウキョウ",
                pos_id: 0,
            },
        ];
        let line_to_wid = vec![WordId::new(0, 11), WordId::new(0, 27), WordId::new(0, 42)];
        let resolver = RawDictResolver::new(&entries, line_to_wid.clone(), false);

        assert_eq!(
            resolver.resolve_by_line_ref(WordId::new(0, 0)),
            Some(line_to_wid[0])
        );
        assert_eq!(
            resolver.resolve_by_line_ref(WordId::new(0, 1)),
            Some(line_to_wid[1])
        );
    }
}
