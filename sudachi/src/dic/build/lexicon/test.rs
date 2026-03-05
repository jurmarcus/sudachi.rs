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

use super::*;
use crate::dic::build::error::DicBuildError;
use crate::dic::build::DictBuilder;
use crate::dic::read::word_info::WordInfoParser;
use crate::dic::word_id::WordId;
use crate::error::SudachiError;
use claim::assert_matches;
use std::fmt::Write;

#[test]
fn parse_split_empty() {
    let mut rdr = LexiconReader::new();
    assert_eq!(rdr.parse_splits("", true).unwrap().0.len(), 0);
    assert_eq!(rdr.parse_splits("*", true).unwrap().0.len(), 0);
}

#[test]
fn parse_split_sys_ids() {
    let mut rdr = LexiconReader::new();
    let (splits, rel) = rdr.parse_splits("0/1/2", true).unwrap();
    assert_eq!(splits.len(), 3);
    assert_eq!(rel, 0);
    assert_eq!(splits[0], WordRef::Ref(WordId::new(0, 0)));
    assert_eq!(splits[1], WordRef::Ref(WordId::new(0, 1)));
    assert_eq!(splits[2], WordRef::Ref(WordId::new(0, 2)));
}

#[test]
fn parse_split_user_ids() {
    let mut rdr = LexiconReader::new();
    let (splits, rel) = rdr.parse_splits("0/U1/2", true).unwrap();
    assert_eq!(splits.len(), 3);
    assert_eq!(rel, 0);
    assert_eq!(splits[0], WordRef::Ref(WordId::new(0, 0)));
    assert_eq!(splits[1], WordRef::Ref(WordId::new(1, 1)));
    assert_eq!(splits[2], WordRef::Ref(WordId::new(0, 2)));
}

#[test]
fn parse_split_inline() {
    let mut rdr = LexiconReader::new();
    let (splits, rel) = rdr.parse_splits("0/あ,0,1,2,3,4,5,あ/2", true).unwrap();
    assert_eq!(splits.len(), 3);
    assert_eq!(rel, 1);
    assert_eq!(splits[0], WordRef::Ref(WordId::new(0, 0)));
    assert_eq!(
        splits[1],
        WordRef::Inline {
            surface: "あ".to_string(),
            pos: 0,
            reading: None
        }
    );
    assert_eq!(splits[2], WordRef::Ref(WordId::new(0, 2)));
}

#[test]
fn parse_split_inline_pos_id() {
    let mut rdr = LexiconReader::new();
    let (splits, rel) = rdr.parse_splits("0/あ,0,あ/2", true).unwrap();
    assert_eq!(splits.len(), 3);
    assert_eq!(rel, 1);
    assert_eq!(splits[0], WordRef::Ref(WordId::new(0, 0)));
    assert_eq!(
        splits[1],
        WordRef::Inline {
            surface: "あ".to_string(),
            pos: 0,
            reading: None
        }
    );
    assert_eq!(splits[2], WordRef::Ref(WordId::new(0, 2)));
}

#[test]
fn parse_split_disallow_numeric_ref() {
    let mut rdr = LexiconReader::new();
    assert_matches!(
        rdr.parse_splits("0/1", false),
        Err(BuildFailure::InvalidSplit(_))
    );
}

#[test]
fn parse_kyoto() {
    let mut rdr = LexiconReader::new();
    let data = "京都,6,6,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*";
    rdr.read_bytes(data.as_bytes()).unwrap();
    let entries = rdr.entries();
    assert_eq!(entries.len(), 1);
    let kyoto = &entries[0];
    assert_eq!("京都", kyoto.surface);
    assert_eq!(0, kyoto.pos);
    assert_eq!(
        "名詞,固有名詞,地名,一般,*,*",
        format!("{:?}", rdr.pos_obj(kyoto.pos).unwrap())
    );
    assert_eq!(6, kyoto.left_id);
    assert_eq!(6, kyoto.right_id);
    assert_eq!(5293, kyoto.cost);
    assert_eq!("キョウト", kyoto.reading());
    assert_eq!(Some("キョウト"), kyoto.reading.as_deref());
    assert_eq!("京都", kyoto.norm_form());
    assert_eq!(None, kyoto.norm_form);
    assert_eq!(Mode::A, kyoto.splitting);
    assert_eq!(0, kyoto.splits_a.len());
    assert_eq!(0, kyoto.splits_b.len());
    assert!(kyoto.should_index());
}

#[test]
fn parse_header_with_pos_id_only() {
    let mut rdr = LexiconReader::new();
    let data = concat!(
        "index_form,left_id,right_id,cost,headword,pos_id,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure,synonym_groups\n",
        "京都,6,6,5293,京都,0,キョウト,京都,,A,*,*,*,*"
    );
    // preload one POS to resolve pos_id=0
    let old = "京都,6,6,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*";
    rdr.read_bytes(old.as_bytes()).unwrap();
    let before = rdr.entries().len();
    rdr.read_bytes(data.as_bytes()).unwrap();
    let kyoto = &rdr.entries()[before];
    assert_eq!(kyoto.surface, "京都");
    assert_eq!(kyoto.pos, 0);
}

#[test]
fn parse_header_word_structure_triple_ref() {
    let mut rdr = LexiconReader::new();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure\n",
        "東京都,6,8,5320,名詞,固有名詞,地名,一般,*,*,トウキョウト,,,B,\"東京,0,トウキョウ/都,1,ト\",\"東京,0,トウキョウ/都,2,ト\",\"東京,0,トウキョウ/都,3,ト\"\n"
    );
    rdr.read_bytes(data.as_bytes()).unwrap();
    let e = &rdr.entries()[0];
    assert_eq!(e.splits_a.len(), 2);
    assert_eq!(e.splits_b.len(), 2);
    assert_eq!(e.word_structure.len(), 2);
    assert!(matches!(e.word_structure[0], WordRef::Inline { .. }));
}

#[test]
fn parse_header_dictionary_form_asterisk_fails() {
    let mut rdr = LexiconReader::new();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure\n",
        "東京都,6,8,5320,名詞,固有名詞,地名,一般,*,*,トウキョウト,,*,B,*,*,*\n"
    );
    assert_matches!(
        rdr.read_bytes(data.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplit(_),
            ..
        }))
    );
}

#[test]
fn resolve_header_normalized_form_ref() {
    let mut bldr = DictBuilder::new_system();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure\n",
        "東京,1,1,2816,名詞,固有名詞,地名,一般,*,*,トウキョウ,,,A,*,*,*\n",
        "トウキョウ,1,1,2816,名詞,固有名詞,地名,一般,*,*,トウキョウ,\"東京,0,トウキョウ\",,A,*,*,*\n"
    );
    bldr.read_lexicon(data.as_bytes()).unwrap();
    bldr.resolve().unwrap();
    let e = &bldr.lexicon.entries()[1];
    assert_eq!(e.norm_form(), "東京");
}

#[test]
fn resolve_header_normalized_form_headword_ref() {
    let mut bldr = DictBuilder::new_system();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure\n",
        "東京,1,1,2816,名詞,固有名詞,地名,一般,*,*,トウキョウ,,,A,*,*,*\n",
        "トーキョー,1,1,2816,名詞,固有名詞,地名,一般,*,*,トーキョー,東京,,A,*,*,*\n"
    );
    bldr.read_lexicon(data.as_bytes()).unwrap();
    bldr.resolve().unwrap();
    let e = &bldr.lexicon.entries()[1];
    assert_eq!(e.norm_form(), "東京");
}

#[test]
fn read_pos_table_and_parse_pos_id_lexicon() {
    let mut rdr = LexiconReader::new();
    let pos = "0,名詞,固有名詞,地名,一般,*,*\n1,名詞,一般,*,*,*,*";
    assert_eq!(rdr.read_pos_bytes(pos.as_bytes()).unwrap(), 2);

    let lex = concat!(
        "index_form,left_id,right_id,cost,headword,pos_id,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure,synonym_groups\n",
        "京都,6,6,5293,京都,0,キョウト,京都,,A,*,*,*,*"
    );
    rdr.read_bytes(lex.as_bytes()).unwrap();
    let e = &rdr.entries()[0];
    assert_eq!(e.pos, 0);
}

#[test]
fn read_pos_table_non_contiguous_id_fails() {
    let mut rdr = LexiconReader::new();
    let pos = "2,名詞,固有名詞,地名,一般,*,*";
    assert_matches!(
        rdr.read_pos_bytes(pos.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::InvalidSplit(_),
            ..
        }))
    );
}

#[test]
fn parse_header_reads_split_c_and_user_data() {
    let mut rdr = LexiconReader::new();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure,split_c,user_data\n",
        "東京都,6,8,5320,名詞,固有名詞,地名,一般,*,*,トウキョウト,,,C,\"東京,0,トウキョウ/都,1,ト\",\"東京,0,トウキョウ/都,2,ト\",\"東京,0,トウキョウ/都,3,ト\",\"東京,0,トウキョウ/都,4,ト\",meta"
    );
    rdr.read_bytes(data.as_bytes()).unwrap();
    let e = &rdr.entries()[0];
    assert_eq!(e.splits_c.len(), 2);
    assert_eq!(e.user_data, "meta");
}

#[test]
fn parse_header_mode_optional() {
    let mut rdr = LexiconReader::new();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,split_a,split_b,word_structure\n",
        "京都,6,6,5293,名詞,固有名詞,地名,一般,*,*,キョウト,京都,,*,*,*"
    );
    rdr.read_bytes(data.as_bytes()).unwrap();
    let e = &rdr.entries()[0];
    assert_eq!(e.splitting, Mode::C);
}

#[test]
fn resolve_header_normalized_form_literal_without_target() {
    let mut bldr = DictBuilder::new_system();
    let data = concat!(
        "index_form,left_id,right_id,cost,pos1,pos2,pos3,pos4,pos5,pos6,reading_form,normalized_form,dictionary_form,mode,split_a,split_b,word_structure\n",
        "舞台藝術,1,1,2816,名詞,普通名詞,一般,*,*,*,ブタイゲイジュツ,舞台芸術,,A,*,*,*"
    );
    bldr.read_lexicon(data.as_bytes()).unwrap();
    bldr.resolve().unwrap();
    assert_eq!(bldr.lexicon.entries().len(), 2);
    let e = &bldr.lexicon.entries()[0];
    assert_eq!(e.norm_form(), "舞台芸術");
    let phantom = &bldr.lexicon.entries()[1];
    assert_eq!(phantom.headword(), "舞台芸術");
    assert!(!phantom.should_index());
}

#[test]
fn parse_kyoto_ignored() {
    let mut rdr = LexiconReader::new();
    let data = "京都,-1,-1,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*";
    rdr.read_bytes(data.as_bytes()).unwrap();
    let entries = rdr.entries();
    assert_eq!(entries.len(), 1);
    let kyoto = &entries[0];
    assert!(!kyoto.should_index());
}

#[test]
fn parse_kyoto_synonym_opt() {
    let mut rdr = LexiconReader::new();
    // last field is omitted
    let data = "京都,1,1,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*";
    rdr.read_bytes(data.as_bytes()).unwrap();
    let entries = rdr.entries();
    assert_eq!(entries.len(), 1);
    let kyoto = &entries[0];
    assert_eq!(0, kyoto.synonym_groups.len());
}

#[test]
fn parse_kyoto_not_enough_fields() {
    let mut rdr = LexiconReader::new();
    // last field is omitted
    let data = "京都,1,1,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*";

    assert_matches!(
        rdr.read_bytes(data.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::NoRawField(_),
            line: 1,
            ..
        }))
    );
}

#[test]
fn parse_kyoto_ignored_empty_surface() {
    let mut rdr = LexiconReader::new();
    let data = ",-1,-1,5293,京都,名詞,固有名詞,地名,一般,*,*,キョウト,京都,*,A,*,*,*,*";
    assert_matches!(
        rdr.read_bytes(data.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::EmptySurface,
            line: 1,
            ..
        }))
    );
}

#[test]
fn parse_pos_exhausted() {
    let mut rdr = LexiconReader::new();
    let mut data = String::new();
    for i in 0..=MAX_POS_IDS + 1 {
        writeln!(
            data,
            "x,-1,-1,5293,京都,名詞,固有名詞,地名,一般,*,{},キョウト,京都,*,A,*,*,*,*",
            i
        )
        .unwrap()
    }

    assert_matches!(
        rdr.read_bytes(data.as_bytes()),
        Err(SudachiError::DictionaryCompilationError(DicBuildError {
            cause: BuildFailure::PosLimitExceeded(_),
            ..
        }))
    );
}

#[test]
fn resolve_inline_same_dict() {
    let mut rdr = DictBuilder::new_system();
    let nread = rdr
        .read_lexicon(include_bytes!("data_kyoto_inline.csv"))
        .unwrap();
    assert_eq!(nread, 3);
    let nresolved = rdr.resolve().unwrap();
    assert_eq!(nresolved, 2);
    let e2 = &rdr.lexicon.entries()[2];
    assert_eq!(e2.splits_a[0], WordRef::Ref(WordId::new(0, 1))); //　東
    assert_eq!(e2.splits_a[1], WordRef::Ref(WordId::new(0, 0))); // 京都
}

#[test]
#[ignore = "legacy word_info binary layout is being migrated"]
fn word_info_rw() {
    let mut rdr = LexiconReader::new();
    let data: &[u8] = include_bytes!("data_kyoto_inline.csv");
    rdr.read_bytes(data).unwrap();
    let mut u16w = Utf16Writer::new();
    let mut data: Vec<u8> = Vec::new();
    rdr.entries[0]
        .write_word_info(&mut u16w, &mut data)
        .unwrap();
    // `WordInfoParser` reads the final binary entry layout (params + word-info body).
    // prepend 6-byte word params to reuse the parser for this writer test.
    data.splice(0..0, [0_u8; 6]);

    let wi = WordInfoParser::default().parse(&data).unwrap();
    assert_eq!(wi.pos_id, 0);
    assert_eq!(wi.index_form_length, "京都".len() as i16);
    assert_eq!(wi.dictionary_form, WordId::INVALID.as_raw());
    assert_eq!(wi.a_unit_split.len(), 0);
    assert_eq!(wi.b_unit_split.len(), 0);
    assert_eq!(wi.word_structure.len(), 0);
    assert_eq!(wi.synonym_group_ids.len(), 0);
}
