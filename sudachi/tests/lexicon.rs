/*
 * Copyright (c) 2021-2026 Works Applications Co., Ltd.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

extern crate lazy_static;

mod common;
use common::{LEXICON, LEXICON_SET};
use sudachi::dic::lexicon::LexiconEntry;
use sudachi::dic::word_id::{EntryId, WordId};

#[test]
fn lookup() {
    let res: Vec<LexiconEntry> = LEXICON.lookup("東京都".as_bytes(), 0).collect();
    assert_eq!(3, res.len());
    assert_eq!(LexiconEntry::new(WordId::new(0, 4), 3), res[0]); // 東
    assert_eq!(LexiconEntry::new(WordId::new(0, 5), 6), res[1]); // 東京
    assert_eq!(LexiconEntry::new(WordId::new(0, 6), 9), res[2]); // 東京都

    let res: Vec<LexiconEntry> = LEXICON.lookup("東京都に".as_bytes(), 9).collect();
    assert_eq!(2, res.len());
    assert_eq!(LexiconEntry::new(WordId::new(0, 1), 12), res[0]); // に(接続助詞)
    assert_eq!(LexiconEntry::new(WordId::new(0, 2), 12), res[1]); // に(格助詞)

    let res: Vec<LexiconEntry> = LEXICON.lookup("あれ".as_bytes(), 0).collect();
    assert_eq!(0, res.len());
}

#[test]
fn parameters() {
    // た
    assert_eq!((1, 1, 8729), LEXICON.get_word_param(EntryId::new(0)));

    // 東京都
    assert_eq!((6, 8, 5320), LEXICON.get_word_param(EntryId::new(6)));

    // 都
    assert_eq!((8, 8, 2914), LEXICON.get_word_param(EntryId::new(9)));
}

#[test]
fn word_info() {
    // た
    let wi = LEXICON_SET
        .get_word_info(WordId::new(0, 0))
        .expect("failed to get word_info");
    assert_eq!("た", wi.headword(&LEXICON_SET));
    assert_eq!(3, wi.index_form_length());
    assert_eq!(0, wi.pos_id());
    assert_eq!("た", wi.normalized_form(&LEXICON_SET));
    assert_eq!(WordId::INVALID, wi.borrow_data().dictionary_form_word_id());
    assert_eq!("た", wi.dictionary_form(&LEXICON_SET));
    assert_eq!("タ", wi.reading_form(&LEXICON_SET));
    assert!(wi.a_unit_split().is_empty());
    assert!(wi.b_unit_split().is_empty());
    assert!(wi.word_structure().is_empty());

    // 東京都
    let wi = LEXICON_SET
        .get_word_info(WordId::new(0, 6))
        .expect("failed to get word_info");
    assert_eq!("東京都", wi.headword(&LEXICON_SET));
    assert_eq!([WordId::new(0, 5), WordId::new(0, 9)], wi.a_unit_split());
    assert!(wi.b_unit_split().is_empty());
    assert_eq!([WordId::new(0, 5), WordId::new(0, 9)], wi.word_structure());
    assert!(wi.synonym_group_ids().is_empty());

    // 行っ
    let wi = LEXICON_SET
        .get_word_info(WordId::new(0, 8))
        .expect("failed to get word_info");
    assert_eq!("行っ", wi.headword(&LEXICON_SET));
    assert_eq!("行く", wi.normalized_form(&LEXICON_SET));
    assert_eq!(
        WordId::new(0, 7),
        wi.borrow_data().dictionary_form_word_id()
    );
    assert_eq!("行く", wi.dictionary_form(&LEXICON_SET));
}

#[test]
fn word_info_with_longword() {
    // 0123456789 * 30
    let wi = LEXICON_SET
        .get_word_info(WordId::new(0, 36))
        .expect("failed to get word_info");
    assert_eq!(300, wi.headword(&LEXICON_SET).chars().count());
    assert_eq!(300, wi.index_form_length());
    assert_eq!(300, wi.normalized_form(&LEXICON_SET).chars().count());
    assert_eq!(WordId::INVALID, wi.borrow_data().dictionary_form_word_id());
    assert_eq!(300, wi.dictionary_form(&LEXICON_SET).chars().count());
    assert_eq!(570, wi.reading_form(&LEXICON_SET).chars().count());
}

#[test]
fn size() {
    assert_eq!(39, LEXICON.size())
}
