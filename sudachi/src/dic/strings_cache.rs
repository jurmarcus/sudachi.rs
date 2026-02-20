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
use crate::dic::word_info::{ WordInfoData};
use crate::dic::word_id::WordId;
use crate::dic::subset::InfoSubset;


#[derive(Clone, Debug, Default)]
pub(super) struct StringsCache {
    surface: Option<String>,
    reading: Option<String>,
    normalized_form: Option<String>,
    dictionary_form: Option<String>,
}

impl StringsCache {
    /// Creates a new StringsCache for the WordId
    pub fn new() -> Self {
        StringsCache {
            surface: None,
            reading: None,
            normalized_form: None,
            dictionary_form: None,
        }
    }

    /// Creates a new StringsCache for the WordId with all strings pre-fetched
    pub fn new_with_strings(
        surface: String,
        reading: String,
        normalized_form: String,
        dictionary_form: String,
    ) -> Self {
        StringsCache {
            surface: Some(surface),
            reading: Some(reading),
            normalized_form: Some(normalized_form),
            dictionary_form: Some(dictionary_form),
        }
    }

    pub fn new_with_single_string(
        surface: String,
    ) -> Self {
        StringsCache::new_with_strings(
            surface.clone(),
            surface.clone(),
            surface.clone(),
            surface,
        )
    }
}

impl StringsCache {
    pub fn surface(&mut self, lexicon_set: &LexiconSet, word_info: &WordInfoData, word_id: WordId) -> &str {
        if self.surface.is_none() {
            let strptr = word_info.headword_strptr();

            let s = lexicon_set
                .get_string(word_id, strptr)
                .expect("Headword must exist for non-OOV word IDs");
            self.surface = Some(s);
        }
        self.surface.as_ref().unwrap()
    }

    pub fn reading(&mut self, lexicon_set: &LexiconSet, word_info: &WordInfoData, word_id: WordId) -> &str {
        if self.reading.is_none() {
            let strptr = word_info.reading_form_strptr();

            let s = lexicon_set
                .get_string(word_id, strptr)
                .expect("Reading form must exist for non-OOV word IDs");
            self.reading = Some(s);
        }
        self.reading.as_ref().unwrap()
    }

    pub fn normalized_form(&mut self, lexicon_set: &LexiconSet, word_info: &WordInfoData, word_id: WordId) -> &str {
        if self.normalized_form.is_none() {
            let ref_word_id = word_info.normalized_form_word_id();
            let ref_word_info = lexicon_set
                .get_word_info_subset(ref_word_id, InfoSubset::HEADWORD)
                .expect("WordInfo must exist for non-OOV word IDs");
            let strptr = ref_word_info.headword_strptr();

            let s = lexicon_set
                .get_string(word_id, strptr)
                .expect("Normalized form must exist for non-OOV word IDs");
            self.normalized_form = Some(s);
        }
        self.normalized_form.as_ref().unwrap()
    }

    pub fn dictionary_form(&mut self, lexicon_set: &LexiconSet, word_info: &WordInfoData, word_id: WordId) -> &str {
        if self.dictionary_form.is_none() {
            let ref_word_id = word_info.dictionary_form_word_id();
            let ref_word_info = lexicon_set
                .get_word_info_subset(ref_word_id, InfoSubset::HEADWORD)
                .expect("WordInfo must exist for non-OOV word IDs");
            let strptr = ref_word_info.headword_strptr();

            let s = lexicon_set
                .get_string(word_id, strptr)
                .expect("Dictionary form must exist for non-OOV word IDs");
            self.dictionary_form = Some(s);
        }
        self.dictionary_form.as_ref().unwrap()
    }
}
