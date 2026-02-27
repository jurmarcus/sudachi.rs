/*
 * Copyright (c) 2026 Works Applications Co., Ltd.
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

use crate::dic::connect::ConnectionMatrix;
use crate::dic::description::{Block, Description};
use crate::dic::header::HeaderError;
use crate::dic::lexicon::strings::CompactedStrings;
use crate::dic::lexicon::trie::Trie;
use crate::dic::lexicon::word_id_table::WordIdTable;
use crate::dic::lexicon::word_infos::WordInfos;
use crate::dic::lexicon::word_params::WordParams;
use crate::dic::pos::PosList;
use crate::prelude::*;

/// A single system or user dictionary
pub struct BinaryDictionary<'a> {
    pub description: Description,
    pub grammar: BinaryGrammar<'a>,
    pub lexicon: BinaryLexicon<'a>,
}

impl<'a> BinaryDictionary<'a> {
    /// Load a binary dictionary from bytes
    ///
    /// # Safety
    /// This function is marked unsafe because it does not perform header validation
    unsafe fn load(buf: &'a [u8]) -> SudachiResult<Self> {
        let description = Description::load(buf)?;
        let grammar = BinaryGrammar::load(buf, &description)?;
        let lexicon = BinaryLexicon::load(buf, &description)?;

        Ok(BinaryDictionary {
            description,
            grammar,
            lexicon,
        })
    }

    pub fn load_system(buf: &'a [u8]) -> SudachiResult<Self> {
        let dict = unsafe { Self::load(buf)? };

        if dict.description.is_system_dictionary() {
            Ok(dict)
        } else {
            // TODO: fix error type
            Err(SudachiError::InvalidHeader(
                HeaderError::InvalidSystemDictVersion,
            ))
        }
    }

    pub fn load_user(buf: &'a [u8]) -> SudachiResult<Self> {
        let dict = unsafe { Self::load(buf)? };

        if dict.description.is_user_dictionary() {
            Ok(dict)
        } else {
            // TODO: fix error type
            Err(SudachiError::InvalidHeader(
                HeaderError::InvalidUserDictVersion,
            ))
        }
    }
}

/// Grammar part of the single binary dictionary
pub struct BinaryGrammar<'a> {
    /// The list of part of speechs
    pub pos_list: PosList,

    /// The overloadable connection cost matrix
    ///
    /// Only system dictionary has this.
    pub connection: Option<ConnectionMatrix<'a>>,
}

impl<'a> BinaryGrammar<'a> {
    /// load a grammar from bytes
    pub fn load(buf: &'a [u8], description: &Description) -> SudachiResult<Self> {
        let connection_bytes = description.slice_or_none(buf, Block::ConnectionMatrix)?;
        let connection = match connection_bytes {
            Some(bytes) => {
                let connection = ConnectionMatrix::from_bytes(bytes)?;
                Some(connection)
            }
            None => None,
        };

        let pos_list = PosList::from_bytes(description.slice(buf, Block::POSTable)?)?;

        Ok(Self {
            pos_list,
            connection,
        })
    }
}

/// Lexicon part of the single binary dictionary
pub struct BinaryLexicon<'a> {
    /// TRIE (double array), mapping from index form to WordIdTable offset
    pub trie: Trie<'a>,
    /// list of word ids that have the same index form
    pub word_id_table: WordIdTable<'a>,
    /// list of word information (for analysis)
    pub word_params: WordParams<'a>,
    /// list of word information (for non-analysis)
    pub word_infos: WordInfos<'a>,
    /// Stotage of strings in the lixicon (normalized form etc.)
    pub strings: CompactedStrings<'a>,
    /// The number of entries in the lexicon
    pub num_total_entries: u32,
}

impl<'a> BinaryLexicon<'a> {
    /// load a lexicon from bytes
    pub fn load(buf: &'a [u8], description: &Description) -> SudachiResult<Self> {
        let trie = Trie::from_bytes(description.slice(buf, Block::TRIEIndex)?);
        let word_id_table = WordIdTable::from_bytes(description.slice(buf, Block::WordPointers)?);

        // word_params and word_infos share the same byte range.
        // the first 8 bytes of a word entry is the paramaters and rest is the infos.
        // handle separately because we use them in different steps; during/after analysis.
        let entries_bytes = description.slice(buf, Block::Entries)?;
        let word_params = WordParams::from_bytes(entries_bytes);
        let word_infos = WordInfos::from_bytes(entries_bytes);

        let strings = CompactedStrings::from_bytes(description.slice(buf, Block::Strings)?);

        Ok(Self {
            trie,
            word_id_table,
            word_params,
            word_infos,
            strings,
            num_total_entries: description.num_total_entries(),
        })
    }
}
