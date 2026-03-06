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

mod with_analysis;

use crate::dic::binary_loader::{BinaryDictionary, LoadedDictionary};
use crate::dic::build::error::{BuildFailure, DicBuildError};
use crate::dic::build::DictBuilder;
use crate::dic::LexiconAccess;
use crate::error::SudachiError;
use std::io::sink;

static MATRIX_10_10: &[u8] = include_bytes!("test/matrix_10x10.def");

#[test]
fn read_pos_then_read_lexicon_with_pos_id() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    let pos = "0,名詞,固有名詞,地名,一般,*,*\n1,名詞,一般,*,*,*,*";
    bldr.read_pos(pos.as_bytes()).unwrap();

    let lex = concat!(
        "index_form,left_id,right_id,cost,headword,pos_id,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure,synonym_groups\n",
        "京都,6,6,5293,京都,0,キョウト,京都,,A,*,*,*,*"
    );
    assert_eq!(1, bldr.read_lexicon(lex.as_bytes()).unwrap());
    bldr.resolve().unwrap();
    let mut out = Vec::new();
    bldr.compile(&mut out).unwrap();
}

#[test]
fn read_pos_after_lexicon_fails() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(include_bytes!("test/data_1word.csv"))
        .unwrap();
    let pos = "0,名詞,固有名詞,地名,一般,*,*";
    claim::assert_matches!(
        bldr.read_pos(pos.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplit(_),
            ..
        }))
    );
}

#[test]
fn build_grammar() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    assert_eq!(
        1,
        bldr.read_lexicon(include_bytes!("test/data_1word.csv"))
            .unwrap()
    );
    let mut built = Vec::new();
    bldr.compile(&mut built).unwrap();
    let dic = LoadedDictionary::load_system(&built).unwrap();
    let grammar = &dic.grammar;
    assert_eq!(grammar.pos_list.len(), 1);
    assert_eq!(
        grammar.pos_list[0],
        &["名詞", "固有名詞", "地名", "一般", "*", "*"]
    );
    let conn = grammar.conn_matrix();
    assert_eq!(conn.num_left(), 10);
    assert_eq!(conn.num_right(), 10);
}

#[test]
fn build_lexicon_1word() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    assert_eq!(
        1,
        bldr.read_lexicon(include_bytes!("test/data_1word.csv"))
            .unwrap()
    );
    let mut built = Vec::new();
    bldr.compile(&mut built).unwrap();
    let dic = LoadedDictionary::load_system(&built).unwrap();
    let mut iter = dic.lexicon().lookup("京都".as_bytes(), 0);
    let entry = iter.next().unwrap();
    assert_eq!(entry.end, 6);
    assert_eq!(entry.word_id.dict().as_raw(), 0);
    assert_eq!(iter.next(), None);
    assert_eq!((6, 6, 5293), dic.lexicon().get_word_param(entry.word_id));
    let wi = dic.lexicon().get_word_info(entry.word_id).unwrap();
    assert_eq!(wi.headword(&dic), "京都");
    assert_eq!(wi.normalized_form(&dic), "京都");
    assert_eq!(wi.dictionary_form(&dic), "京都");
    assert_eq!(wi.reading_form(&dic), "キョウト");
}

#[test]
fn build_system_1word() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    assert_eq!(
        1,
        bldr.read_lexicon(include_bytes!("test/data_1word.csv"))
            .unwrap()
    );
    let mut built = Vec::new();
    bldr.compile(&mut built).unwrap();
    let dic = LoadedDictionary::load_system(&built).unwrap();

    let entry = dic.lexicon().lookup("京都".as_bytes(), 0).next().unwrap();
    assert_eq!(entry.word_id.dict().as_raw(), 0);
    let info = dic.lexicon().get_word_info(entry.word_id).unwrap();
    assert_eq!(info.headword(&dic), "京都");
    assert_eq!(info.reading_form(&dic), "キョウト");
}

#[test]
fn build_system_3words() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    assert_eq!(
        3,
        bldr.read_lexicon(include_bytes!("test/data_3words.csv"))
            .unwrap()
    );
    bldr.resolve().unwrap();
    let mut built = Vec::new();
    bldr.compile(&mut built).unwrap();
    let dic = LoadedDictionary::load_system(&built).unwrap();
    let mut iter = dic.lexicon().lookup("東京".as_bytes(), 0);
    let _short = iter.next().unwrap();
    let entry = iter.next().unwrap();
    assert_eq!(entry.end, 6);
    assert_eq!(entry.word_id.dict().as_raw(), 0);
    assert_eq!(iter.next(), None);
    let info = dic.lexicon().get_word_info(entry.word_id).unwrap();
    assert_eq!(info.headword(&dic), "京都");
}

#[test]
fn build_user_dictionary_crossrefs() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(include_bytes!("test/matrix_10x10.def"))
        .unwrap();
    assert_eq!(
        3,
        bldr.read_lexicon(include_bytes!("test/data_3words.csv"))
            .unwrap()
    );
    bldr.resolve().unwrap();
    let mut system_bin = Vec::new();
    bldr.compile(&mut system_bin).unwrap();
    let dic = LoadedDictionary::load_system(&system_bin).unwrap();
    // user dictionary
    let mut bldr2 = DictBuilder::new_user(&dic);
    assert_eq!(
        2,
        bldr2
            .read_lexicon(include_bytes!("test/data_2words_3w_refs.csv"))
            .unwrap()
    );
    bldr2.resolve().unwrap();
    let mut user_dic = Vec::new();
    bldr2.compile(&mut user_dic).unwrap();
    let udic = BinaryDictionary::load_user(&user_dic).unwrap();
    let dic = dic.merge_dictionary(udic).unwrap();
    let mut iter = dic.lexicon_set.lookup("東".as_bytes(), 0);
    let entry_to = iter.next().unwrap();

    let mut iter = dic.lexicon_set.lookup("関東".as_bytes(), 0);
    let entry_kan = iter.next().unwrap();
    assert_eq!(entry_kan.word_id.dict().as_raw(), 1);
    let winfo = dic.lexicon_set.get_word_info(entry_kan.word_id).unwrap();
    assert_eq!(
        dic.lexicon_set.get_word_param(entry_kan.word_id),
        (4, 4, 4000)
    );
    assert_eq!(winfo.headword(&dic), "関");
    assert_eq!(winfo.a_unit_split().len(), 0);
    assert_eq!(winfo.synonym_group_ids(), [0, 1]);

    let entry_kanto = iter.next().unwrap();
    assert_eq!(entry_kanto.word_id.dict().as_raw(), 1);
    assert_eq!(
        dic.lexicon_set.get_word_param(entry_kanto.word_id),
        (5, 5, 5000)
    );
    let winfo = dic.lexicon_set.get_word_info(entry_kanto.word_id).unwrap();
    assert_eq!(winfo.headword(&dic), "関東");
    assert_eq!(winfo.a_unit_split(), [entry_kan.word_id, entry_to.word_id]);
    assert_eq!(winfo.b_unit_split(), [entry_kan.word_id, entry_to.word_id]);
    assert_eq!(iter.next(), None);
}

#[test]
fn conn_id_too_big_left() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,10,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*".as_bytes(),
    )
    .unwrap();
    let mut sink = sink();
    claim::assert_matches!(bldr.compile(&mut sink), Err(_));
}

#[test]
fn conn_id_too_big_right() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,10,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*".as_bytes(),
    )
    .unwrap();
    let mut sink = sink();
    claim::assert_matches!(bldr.compile(&mut sink), Err(_));
}

#[test]
fn word_id_too_big_dicform() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,5,A,*,*,*,*".as_bytes(),
    )
    .unwrap();
    claim::assert_matches!(
        bldr.resolve(),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplitWordReference(_),
            ..
        }))
    );
}

#[test]
fn word_id_too_big_split_a() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,C,0/5,*,*,*".as_bytes(),
    )
    .unwrap();
    claim::assert_matches!(
        bldr.resolve(),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplitWordReference(_),
            ..
        }))
    );
}

#[test]
fn word_id_too_big_split_b() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,C,*,0/5,*,*".as_bytes(),
    )
    .unwrap();
    claim::assert_matches!(
        bldr.resolve(),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplitWordReference(_),
            ..
        }))
    );
}

#[test]
fn word_id_too_big_word_structure() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,C,*,*,0/5,*".as_bytes(),
    )
    .unwrap();
    claim::assert_matches!(
        bldr.resolve(),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplitWordReference(_),
            ..
        }))
    );
}

#[test]
fn word_id_too_big_dicform_userdic_insystem() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*".as_bytes(),
    )
    .unwrap();
    let mut data = Vec::new();
    bldr.compile(&mut data).unwrap();
    let dic = LoadedDictionary::load_system(&data).unwrap();
    let mut bldr = DictBuilder::new_user(&dic);
    bldr.read_lexicon("東,6,6,5293,東,名詞,一般,*,*,*,*,ヒガシ,*,10,A,*,*,*,*".as_bytes())
        .unwrap();
    let mut sink = sink();

    claim::assert_matches!(
        bldr.compile(&mut sink),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::UnresolvedSplits,
            ..
        }))
    );
}

#[test]
fn word_id_too_big_dicform_userdic_inuser() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(
        "京都,5,5,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*".as_bytes(),
    )
    .unwrap();
    let mut data = Vec::new();
    bldr.compile(&mut data).unwrap();
    let dic = LoadedDictionary::load_system(&data).unwrap();
    let mut bldr = DictBuilder::new_user(&dic);
    bldr.read_lexicon("東,6,6,5293,東,名詞,一般,*,*,*,*,ヒガシ,*,U15,A,*,*,*,*".as_bytes())
        .unwrap();
    let mut sink = sink();

    claim::assert_matches!(
        bldr.compile(&mut sink),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::UnresolvedSplits,
            ..
        }))
    );
}

#[test]
fn resolve_user_entry_without_system_in_trie() {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(MATRIX_10_10).unwrap();
    bldr.read_lexicon(include_bytes!("test/sys_no_entry.csv"))
        .unwrap();
    bldr.resolve().unwrap();
    let mut data = Vec::new();
    bldr.compile(&mut data).unwrap();
    let dic = LoadedDictionary::load_system(&data).unwrap();
    let mut iter = dic.lexicon().lookup("東京".as_bytes(), 0);
    let e = iter.next().unwrap();
    assert_eq!(e.end, 6);
    assert_eq!(iter.next(), None);
    drop(iter);

    let mut bldr = DictBuilder::new_user(&dic);
    bldr.read_lexicon(include_bytes!("test/data_2words_3w_refs.csv"))
        .unwrap();
    bldr.resolve().unwrap();
    let mut data2 = Vec::new();
    bldr.compile(&mut data2).unwrap();
    let udic = BinaryDictionary::load_user(&data2).unwrap();
    let dic = dic.merge_dictionary(udic).unwrap();
    let mut iter = dic.lexicon().lookup("関東".as_bytes(), 0);
    let _ = iter.next().unwrap();
    let e = iter.next().unwrap();
    assert_eq!(iter.next(), None);
    let winfo = dic.lexicon_set.get_word_info(e.word_id).unwrap();
    assert_eq!(winfo.a_unit_split().len(), 2);
}
