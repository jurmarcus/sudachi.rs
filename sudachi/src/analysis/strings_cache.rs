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


use crate::dic::lexicon_set::LexiconSet;
use crate::dic::read::word_info;
use crate::dic::word_info::WordInfo;
use crate::dic::word_id::{WordId, DictId};
use crate::dic::subset::InfoSubset;


#[derive(Debug)]
pub(super) struct StringsCache<'a> {
    pub lexicon_set: &'a LexiconSet<'a>,
    pub word_id: WordId,
    dict_id: DictId,

    word_info: Option<WordInfo>,

    surface: Option<String>,
    reading: Option<String>,
    normalized_form: Option<String>,
    dictionary_form: Option<String>,
}

impl<'a> StringsCache<'a> {
    pub fn new(lexicon_set: &'a LexiconSet<'a>, word_id: WordId) -> Self {
        StringsCache {
            lexicon_set,
            word_id,
            dict_id: word_id.dict_id(),
            word_info: None,
            surface: None,
            reading: None,
            normalized_form: None,
            dictionary_form: None,
        }
    }

    pub fn new_with_info(
        lexicon_set: &'a LexiconSet<'a>,
        word_id: WordId,
        word_info: WordInfo,
    ) -> Self {
        StringsCache {
            lexicon_set,
            word_id,
            dict_id: word_id.dict_id(),
            word_info: Some(word_info),
            surface: None,
            reading: None,
            normalized_form: None,
            dictionary_form: None,
        }
    }

    pub fn new_with_strings(
        lexicon_set: &'a LexiconSet<'a>,
        word_id: WordId,
        word_info: WordInfo,
        surface: String,
        reading: String,
        normalized_form: String,
        dictionary_form: String,
    ) -> Self {
        StringsCache {
            lexicon_set,
            word_id: word_id,
            dict_id: word_id.dict_id(),
            word_info: Some(word_info),
            surface: Some(surface),
            reading: Some(reading),
            normalized_form: Some(normalized_form),
            dictionary_form: Some(dictionary_form),
        }
    }

    pub fn new_with_single_string(
        lexicon_set: &'a LexiconSet<'a>,
        word_id: WordId,
        word_info: WordInfo,
        surface: String,
    ) -> Self {
        StringsCache::new_with_strings(
            lexicon_set,
            word_id,
            word_info,
            surface.clone(),
            surface.clone(),
            surface.clone(),
            surface,
        )
    }
}

impl StringsCache<'_> {
    const STRINGS_SUBSET: InfoSubset = InfoSubset::HEADWORD | InfoSubset::READING_FORM | InfoSubset::NORMALIZED_FORM | InfoSubset::DICTIONARY_FORM;

    pub fn word_info(&mut self) -> &WordInfo {
        if self.word_info.is_none() {
            let info = self
                .lexicon_set
                .get_word_info_subset(self.word_id, Self::STRINGS_SUBSET)
                .expect("WordInfo must exist for non-OOV word IDs");
            self.word_info = Some(info);
        }
        self.word_info.as_ref().unwrap()
    }

    pub fn surface(&mut self) -> &str {
        if self.surface.is_none() {
            let info = self.word_info();
            let strptr = info.headword_strptr();

            let s = self
                .lexicon_set
                .get_string(self.dict_id, strptr)
                .expect("Headword must exist for non-OOV word IDs");
            self.surface = Some(s);
        }
        self.surface.as_ref().unwrap()
    }

    pub fn reading(&mut self) -> &str {
        if self.reading.is_none() {
            let info = self.word_info();
            let strptr = info.reading_form_strptr();

            let s = self
                .lexicon_set
                .get_string(self.dict_id, strptr)
                .expect("Reading form must exist for non-OOV word IDs");
            self.reading = Some(s);
        }
        self.reading.as_ref().unwrap()
    }

    pub fn normalized_form(&mut self) -> &str {
        if self.normalized_form.is_none() {
            let info = self.word_info();
            let ref_word_id = info.normalized_form_word_id();
            let ref_word_info = self
                .lexicon_set
                .get_word_info_subset(ref_word_id, InfoSubset::HEADWORD)
                .expect("WordInfo must exist for non-OOV word IDs");
            let strptr = ref_word_info.headword_strptr();

            let s = self
                .lexicon_set
                .get_string(self.dict_id, strptr)
                .expect("Normalized form must exist for non-OOV word IDs");
            self.normalized_form = Some(s);
        }
        self.normalized_form.as_ref().unwrap()
    }

    pub fn dictionary_form(&mut self) -> &str {
        if self.dictionary_form.is_none() {
            let info = self.word_info();
            let ref_word_id = info.dictionary_form_word_id();
            let ref_word_info = self
                .lexicon_set
                .get_word_info_subset(ref_word_id, InfoSubset::HEADWORD)
                .expect("WordInfo must exist for non-OOV word IDs");
            let strptr = ref_word_info.headword_strptr();

            let s = self
                .lexicon_set
                .get_string(self.dict_id, strptr)
                .expect("Dictionary form must exist for non-OOV word IDs");
            self.dictionary_form = Some(s);
        }
        self.dictionary_form.as_ref().unwrap()
    }
}


