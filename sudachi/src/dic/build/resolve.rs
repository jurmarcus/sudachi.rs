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
use crate::dic::word_id::WordRef;
use crate::dic::word_info::WordInfo;
use crate::dic::DictionaryAccess;
use crate::error::SudachiResult;
use crate::util::fxhash::FxBuildHasher;
use std::collections::HashMap;

// HashMap from headword to (pos_id, reading_form, word-ref)s
type ResolutionCandidateMap<T> = HashMap<T, Vec<(u16, Option<T>, WordRef)>, FxBuildHasher>;

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

/// Resolver based on a (system) binary dictionary.
///
/// We can't use trie to resolve splits because it is possible that refs are not in trie
/// This resolver has to be owning because the dictionary content is lazily loaded and transient
pub struct BinDictResolver {
    index: ResolutionCandidateMap<String>,
    headwords: HashMap<WordRef, String, FxBuildHasher>,
    line_to_wref: Vec<WordRef>,
}

impl BinDictResolver {
    pub fn new<D: DictionaryAccess>(dict: D) -> SudachiResult<Self> {
        let lex = dict.lexicon();
        let line_to_wid = lex.system_word_ids_in_order();
        let line_to_wref = line_to_wid
            .iter()
            .map(|wid| WordRef::new(true, wid.entry().as_raw()))
            .collect::<Vec<_>>();
        let mut index: ResolutionCandidateMap<String> = HashMap::default();
        let mut headwords: HashMap<WordRef, String, FxBuildHasher> = HashMap::default();
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

            let wref = WordRef::new(true, wid.entry().as_raw());
            index
                .entry(headword.clone())
                .or_default()
                .push((pos_id, rdfield, wref));
            headwords.insert(wref, headword);
        }

        Ok(Self {
            index,
            headwords,
            line_to_wref,
        })
    }
}

impl WordRefResolver for BinDictResolver {
    fn resolve_by_line_ref(&self, line_ref: WordRef) -> Option<WordRef> {
        if !line_ref.is_system() {
            return None;
        }
        self.line_to_wref
            .get(line_ref.entry().as_raw() as usize)
            .copied()
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordRef> {
        self.index
            .get(headword)
            .and_then(|v| v.first().map(|(_, _, wref)| *wref))
    }

    fn resolve_inline(&self, headword: &str, pos: u16, reading: Option<&str>) -> Option<WordRef> {
        self.index.get(headword).and_then(|v| {
            for (p, rd, wref) in v {
                if *p == pos && reading.eq(&rd.as_deref()) {
                    return Some(*wref);
                }
            }
            None
        })
    }

    fn resolve_headword(&self, wref: WordRef) -> Option<String> {
        self.headwords.get(&wref).cloned()
    }
}

/// Resolver based on a lexicon csv
pub struct RawDictResolver {
    data: ResolutionCandidateMap<String>,
    headwords: Vec<String>,
    line_to_wref: Vec<WordRef>,
    user: bool,
}

impl RawDictResolver {
    pub(crate) fn new<T: ResolverEntryView>(
        entries: &[T],
        line_to_wref: Vec<WordRef>,
        user: bool,
    ) -> Self {
        let mut data: ResolutionCandidateMap<String> = HashMap::default();
        let mut headwords = Vec::with_capacity(entries.len());

        for (i, e) in entries.iter().enumerate() {
            let headword = e.headword().to_owned();
            let reading = e.reading().to_owned();
            let wref = line_to_wref[i];
            headwords.push(headword.clone());

            let read_opt = if e.headword() == reading {
                None
            } else {
                Some(reading)
            };

            data.entry(headword)
                .or_default()
                .push((e.pos_id(), read_opt, wref));
        }

        Self {
            data,
            headwords,
            line_to_wref,
            user,
        }
    }
}

impl WordRefResolver for RawDictResolver {
    fn resolve_by_line_ref(&self, line_ref: WordRef) -> Option<WordRef> {
        if line_ref.is_system() == self.user {
            return None;
        }
        self.line_to_wref
            .get(line_ref.entry().as_raw() as usize)
            .copied()
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordRef> {
        self.data
            .get(headword)
            .and_then(|v| v.first().map(|(_, _, wref)| *wref))
    }

    fn resolve_inline(&self, headword: &str, pos: u16, reading: Option<&str>) -> Option<WordRef> {
        self.data.get(headword).and_then(|data| {
            for (p, rd, wref) in data {
                if *p == pos && rd.as_deref() == reading {
                    return Some(*wref);
                }
            }
            None
        })
    }

    fn resolve_headword(&self, wref: WordRef) -> Option<String> {
        if wref.is_system() == self.user {
            return None;
        }
        self.line_to_wref
            .iter()
            .position(|candidate| candidate == &wref)
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
    fn resolve_by_line_ref(&self, line_ref: WordRef) -> Option<WordRef> {
        self.a
            .resolve_by_line_ref(line_ref)
            .or_else(|| self.b.resolve_by_line_ref(line_ref))
    }

    fn resolve_by_headword(&self, headword: &str) -> Option<WordRef> {
        self.a
            .resolve_by_headword(headword)
            .or_else(|| self.b.resolve_by_headword(headword))
    }

    fn resolve_inline(&self, headword: &str, pos: u16, reading: Option<&str>) -> Option<WordRef> {
        self.a
            .resolve_inline(headword, pos, reading)
            .or_else(|| self.b.resolve_inline(headword, pos, reading))
    }

    fn resolve_headword(&self, wref: WordRef) -> Option<String> {
        self.a
            .resolve_headword(wref)
            .or_else(|| self.b.resolve_headword(wref))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dic::build::lexicon::WordRef;
    use crate::dic::word_id::WordRef as DicWordRef;

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
        by_line_ref: Option<DicWordRef>,
        by_headword: Option<DicWordRef>,
        by_inline: Option<DicWordRef>,
    }

    impl WordRefResolver for StubResolver {
        fn resolve_by_line_ref(&self, _line_ref: DicWordRef) -> Option<DicWordRef> {
            self.by_line_ref
        }

        fn resolve_by_headword(&self, _headword: &str) -> Option<DicWordRef> {
            self.by_headword
        }

        fn resolve_inline(
            &self,
            _headword: &str,
            _pos: u16,
            _reading: Option<&str>,
        ) -> Option<DicWordRef> {
            self.by_inline
        }
    }

    #[test]
    fn chained_resolver_prioritizes_first_resolver() {
        let a = StubResolver {
            by_line_ref: Some(DicWordRef::new(true, 3)),
            by_headword: Some(DicWordRef::new(true, 1)),
            by_inline: Some(DicWordRef::new(true, 2)),
        };
        let b = StubResolver {
            by_line_ref: Some(DicWordRef::new(false, 3)),
            by_headword: Some(DicWordRef::new(false, 1)),
            by_inline: Some(DicWordRef::new(false, 2)),
        };
        let chained = ChainedResolver::new(a, b);
        assert_eq!(
            chained.resolve(&crate::dic::build::lexicon::WordRef::LineRef(
                DicWordRef::new(true, 0,)
            )),
            Some(DicWordRef::new(true, 3))
        );
        assert_eq!(
            chained.resolve(&WordRef::Headword("京都".to_string())),
            Some(DicWordRef::new(true, 1))
        );
        assert_eq!(
            chained.resolve(&WordRef::Inline {
                headword: "京都".to_string(),
                pos: 0,
                reading: None,
            }),
            Some(DicWordRef::new(true, 2))
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
        let line_to_wref = vec![
            DicWordRef::new(true, 11),
            DicWordRef::new(true, 27),
            DicWordRef::new(true, 42),
        ];
        let resolver = RawDictResolver::new(&entries, line_to_wref.clone(), false);

        assert_eq!(
            resolver.resolve_inline("京都", 0, Some("キョウト")),
            Some(line_to_wref[0])
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
        let line_to_wref = vec![
            DicWordRef::new(true, 11),
            DicWordRef::new(true, 27),
            DicWordRef::new(true, 42),
        ];
        let resolver = RawDictResolver::new(&entries, line_to_wref.clone(), false);

        assert_eq!(resolver.resolve_by_headword("京都"), Some(line_to_wref[0]));
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
        let line_to_wref = vec![
            DicWordRef::new(true, 11),
            DicWordRef::new(true, 27),
            DicWordRef::new(true, 42),
        ];
        let resolver = RawDictResolver::new(&entries, line_to_wref.clone(), false);

        assert_eq!(
            resolver.resolve_by_line_ref(DicWordRef::new(true, 0)),
            Some(line_to_wref[0])
        );
        assert_eq!(
            resolver.resolve_by_line_ref(DicWordRef::new(true, 1)),
            Some(line_to_wref[1])
        );
    }
}
