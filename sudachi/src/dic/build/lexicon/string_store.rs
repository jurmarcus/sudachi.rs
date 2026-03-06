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

use std::collections::HashMap;
use std::io::Write;

use crate::dic::lexicon::strings::StringPointer;
use crate::error::SudachiResult;

use super::RawLexiconEntry;

pub struct StringStore {
    map: HashMap<String, StringPointer>,
    bytes: Vec<u8>,
}

impl StringStore {
    pub fn from_entries(entries: &[RawLexiconEntry]) -> SudachiResult<Self> {
        let mut st = Self {
            map: HashMap::new(),
            bytes: Vec::new(),
        };
        st.insert("")?;
        for e in entries {
            st.insert(e.headword())?;
            st.insert(e.reading())?;
        }
        Ok(st)
    }

    pub fn resolve(&self, s: &str) -> StringPointer {
        *self
            .map
            .get(s)
            .unwrap_or_else(|| panic!("string pointer missing for {}", s))
    }

    pub fn write<W: Write>(&self, w: &mut W) -> SudachiResult<usize> {
        w.write_all(&self.bytes)?;
        Ok(self.bytes.len())
    }

    fn insert(&mut self, s: &str) -> SudachiResult<()> {
        if self.map.contains_key(s) {
            return Ok(());
        }

        let utf16: Vec<u16> = s.encode_utf16().collect();
        let len = utf16.len() as u32;
        let mut offset = (self.bytes.len() / 2) as u32;
        let ptr = loop {
            if let Ok(ptr) = StringPointer::checked(len, offset) {
                break ptr;
            }
            offset += 1;
        };
        let expected = (offset as usize) * 2;
        if self.bytes.len() < expected {
            self.bytes.resize(expected, 0);
        }
        for c in utf16 {
            self.bytes.extend_from_slice(&c.to_le_bytes());
        }
        self.map.insert(s.to_owned(), ptr);
        Ok(())
    }
}
