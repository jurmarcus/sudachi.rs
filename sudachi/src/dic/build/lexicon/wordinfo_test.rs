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

use crate::dic::binary_loader::LoadedDictionary;
use crate::dic::build::DictBuilder;
use crate::dic::description::{Block, Description};
use crate::dic::subset::InfoSubset;
use crate::dic::word_info::parse::WordInfoParser;
use crate::dic::word_info::WordInfos;

#[test]
fn wordinfo_subset_surface() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::HEADWORD)
        .parse(&data)
        .unwrap();
    assert!(wi.headword_strptr.length > 0);
}

#[test]
fn wordinfo_subset_len() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::INDEX_FORM_LENGTH)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.index_form_length, 6);
}

#[test]
fn wordinfo_subset_pos() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::POS_ID)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.pos_id, 1);
}

#[test]
fn wordinfo_subset_norm() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::NORMALIZED_FORM)
        .parse(&data)
        .unwrap();
    assert_ne!(wi.normalized_form, 0);
}

#[test]
fn wordinfo_subset_reading() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::READING_FORM)
        .parse(&data)
        .unwrap();
    assert!(wi.reading_form_strptr.length > 0);
}

#[test]
fn wordinfo_subset_dic_form_id() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::DICTIONARY_FORM)
        .parse(&data)
        .unwrap();
    assert_ne!(wi.dictionary_form, 0);
}

#[test]
fn wordinfo_subset_dic_split_a() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::SPLIT_A)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.a_unit_split.len(), 2);
}

#[test]
fn wordinfo_subset_dic_split_b() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::SPLIT_B)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.b_unit_split.len(), 2);
}

#[test]
fn wordinfo_subset_dic_word_structure() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::WORD_STRUCTURE)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.word_structure.len(), 2);
}

#[test]
fn wordinfo_subset_dic_synonym() {
    let data = make_data();
    let wi = WordInfoParser::subset(InfoSubset::SYNONYM_GROUP_IDS)
        .parse(&data)
        .unwrap();
    assert_eq!(wi.synonym_group_ids, [7, 8]);
}

fn make_data() -> Vec<u8> {
    let mut bldr = DictBuilder::new_system();
    bldr.read_conn(include_bytes!("../test/matrix_10x10.def"))
        .unwrap();
    bldr.read_lexicon(include_bytes!("data_full_wordinfo.csv"))
        .unwrap();
    bldr.resolve().unwrap();

    let mut bin = Vec::new();
    bldr.compile(&mut bin).unwrap();

    let dic = LoadedDictionary::load_system(&bin).unwrap();
    let target = dic
        .lexicon_set
        .lookup("東京".as_bytes(), 0)
        .find(|e| e.end == "東京".len())
        .unwrap()
        .word_id;

    let desc = Description::load(&bin).unwrap();
    let entries = desc.slice(&bin, Block::Entries).unwrap();
    let offset = (target.entry().as_raw() as usize) << WordInfos::WORD_ID_ALIGNMENT_BITS;
    entries[offset..].to_vec()
}
