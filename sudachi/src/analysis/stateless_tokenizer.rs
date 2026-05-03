/*
 *  Copyright (c) 2021-2024 Works Applications Co., Ltd.
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

use crate::analysis::node::ResultNode;
use crate::analysis::stateful_tokenizer::StatefulTokenizer;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

use crate::dic::grammar::Grammar;
use crate::dic::lexicon_set::LexiconSet;
use crate::dic::subset::InfoSubset;
use crate::error::SudachiResult;
use crate::input_text::InputBuffer;
use crate::plugin::input_text::InputTextPlugin;
use crate::plugin::oov::OovProviderPlugin;
use crate::plugin::path_rewrite::PathRewritePlugin;

use super::mlist::MorphemeList;
use super::{Mode, Tokenize};

// Per-thread pool of StatefulTokenizers, keyed by (lexicon pointer, T's
// TypeId).
//
// - Lexicon pointer is stable across Arc clones, so all StatelessTokenizers
//   wrapping the same dictionary share a single pooled tokenizer per thread.
// - TypeId disambiguates wrapper types (e.g. Arc<JapaneseDictionary> vs
//   &'static JapaneseDictionary) sharing the same lexicon pointer, avoiding
//   downcast collisions.
// - Values are boxed as `dyn Any` because thread_local cannot be generic.
// - Tokenizers stay borrowed for the duration of one tokenize call. The
//   closure must not call back into a tokenize on the same dict — RefCell
//   borrow_mut would panic.
thread_local! {
    static POOL: RefCell<HashMap<(usize, TypeId), Box<dyn Any>>> =
        RefCell::new(HashMap::new());
}

/// Provides access to dictionary data
pub trait DictionaryAccess {
    fn grammar(&self) -> &Grammar<'_>;
    fn lexicon(&self) -> &LexiconSet<'_>;
    fn input_text_plugins(&self) -> &[Box<dyn InputTextPlugin + Sync + Send>];
    fn oov_provider_plugins(&self) -> &[Box<dyn OovProviderPlugin + Sync + Send>];
    fn path_rewrite_plugins(&self) -> &[Box<dyn PathRewritePlugin + Sync + Send>];
}

impl<T> DictionaryAccess for T
where
    T: Deref,
    <T as Deref>::Target: DictionaryAccess,
{
    fn grammar(&self) -> &Grammar<'_> {
        <T as Deref>::deref(self).grammar()
    }

    fn lexicon(&self) -> &LexiconSet<'_> {
        <T as Deref>::deref(self).lexicon()
    }

    fn input_text_plugins(&self) -> &[Box<dyn InputTextPlugin + Sync + Send>] {
        <T as Deref>::deref(self).input_text_plugins()
    }

    fn oov_provider_plugins(&self) -> &[Box<dyn OovProviderPlugin + Sync + Send>] {
        <T as Deref>::deref(self).oov_provider_plugins()
    }

    fn path_rewrite_plugins(&self) -> &[Box<dyn PathRewritePlugin + Sync + Send>] {
        <T as Deref>::deref(self).path_rewrite_plugins()
    }
}

/// Implementation of a Tokenizer which does not have tokenization state.
///
/// This is a wrapper which is generic over dictionary pointers.
/// Usable where dictionary is a struct itself, &, &mut, Rc<.>, Arc<.>.
pub struct StatelessTokenizer<T> {
    dict: T,
}

impl<T: DictionaryAccess> StatelessTokenizer<T> {
    pub fn new(dict: T) -> StatelessTokenizer<T> {
        StatelessTokenizer { dict }
    }
}

impl<T> StatelessTokenizer<T>
where
    T: Deref,
    <T as Deref>::Target: DictionaryAccess,
{
    pub fn as_dict(&self) -> &<T as Deref>::Target {
        return Deref::deref(&self.dict);
    }
}

impl<T> StatelessTokenizer<T>
where
    T: DictionaryAccess + Clone + 'static,
{
    /// Tokenize a batch of inputs sequentially, sharing the per-thread
    /// pooled `StatefulTokenizer`.
    ///
    /// Equivalent to calling [`Tokenize::tokenize`] in a loop, but acquires
    /// the thread-local pool only once instead of once per call. For
    /// parallel batches, chunk the inputs and run `tokenize_batch` on
    /// multiple worker threads — the per-thread pool handles isolation.
    ///
    /// Returns `Vec<MorphemeList<T>>` in the same order as `inputs`. Aborts
    /// the batch on the first error.
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
    /// # use sudachi::analysis::Mode;
    /// # use sudachi::dic::dictionary::JapaneseDictionary;
    /// # let dict: Arc<JapaneseDictionary> = unimplemented!();
    /// let tokenizer = StatelessTokenizer::new(dict);
    /// let inputs = ["今日", "明日", "昨日"];
    /// let results = tokenizer
    ///     .tokenize_batch(&inputs, Mode::C, false)
    ///     .unwrap();
    /// assert_eq!(results.len(), inputs.len());
    /// ```
    pub fn tokenize_batch<S: AsRef<str>>(
        &self,
        inputs: &[S],
        mode: Mode,
        enable_debug: bool,
    ) -> SudachiResult<Vec<MorphemeList<T>>> {
        let key = (
            self.dict.lexicon() as *const _ as usize,
            TypeId::of::<StatefulTokenizer<T>>(),
        );
        POOL.with(|pool_cell| {
            let mut pool = pool_cell.borrow_mut();
            let entry = pool.entry(key).or_insert_with(|| {
                Box::new(StatefulTokenizer::create(
                    self.dict.clone(),
                    enable_debug,
                    mode,
                ))
            });
            let tok: &mut StatefulTokenizer<T> = entry
                .downcast_mut::<StatefulTokenizer<T>>()
                .expect("pool entry type mismatch (TypeId-keyed; should be impossible)");
            tok.set_mode(mode);

            let mut out = Vec::with_capacity(inputs.len());
            for input in inputs {
                tok.reset().push_str(input.as_ref());
                tok.do_tokenize()?;
                let mut list = MorphemeList::empty(self.dict.clone());
                list.collect_results(tok)?;
                out.push(list);
            }
            Ok(out)
        })
    }
}

impl<T> Tokenize for StatelessTokenizer<T>
where
    T: DictionaryAccess + Clone + 'static,
{
    type Dictionary = T;

    /// Tokenize `input` using a thread-local cached `StatefulTokenizer`.
    ///
    /// The first call on a given thread for a given dictionary constructs a
    /// `StatefulTokenizer` and stores it in a thread-local pool keyed by
    /// the underlying lexicon pointer. Subsequent calls reuse it, avoiding
    /// per-call construction of the lattice, OOV buffer, and top-path Vec.
    ///
    /// # Invariants
    /// - The closure passed to `POOL.with` must not call back into
    ///   `tokenize` for the same dictionary; the inner `RefCell::borrow_mut`
    ///   would panic.
    /// - `StatefulTokenizer<T>` must be `'static` so it can live in the
    ///   pool. This applies to common wrappers (`Arc<JapaneseDictionary>`,
    ///   owned `JapaneseDictionary`, `&'static JapaneseDictionary`) but
    ///   excludes non-static borrows.
    fn tokenize<'a>(
        &'a self,
        input: &'a str,
        mode: Mode,
        enable_debug: bool,
    ) -> SudachiResult<MorphemeList<Self::Dictionary>> {
        let key = (
            self.dict.lexicon() as *const _ as usize,
            TypeId::of::<StatefulTokenizer<T>>(),
        );
        POOL.with(|pool_cell| {
            let mut pool = pool_cell.borrow_mut();
            let entry = pool.entry(key).or_insert_with(|| {
                Box::new(StatefulTokenizer::create(
                    self.dict.clone(),
                    enable_debug,
                    mode,
                ))
            });
            // Safe: the key includes TypeId::of::<StatefulTokenizer<T>>(),
            // so the value at this key must be StatefulTokenizer<T>.
            let tok: &mut StatefulTokenizer<T> = entry
                .downcast_mut::<StatefulTokenizer<T>>()
                .expect("pool entry type mismatch (TypeId-keyed; should be impossible)");
            tok.set_mode(mode);
            tok.reset().push_str(input);
            tok.do_tokenize()?;
            let mut list = MorphemeList::empty(self.dict.clone());
            list.collect_results(tok)?;
            Ok(list)
        })
    }
}

pub(super) fn split_path<T: DictionaryAccess + ?Sized>(
    dict: &T,
    path: Vec<ResultNode>,
    mode: Mode,
    subset: InfoSubset,
    input: &InputBuffer,
) -> SudachiResult<Vec<ResultNode>> {
    if mode == Mode::C {
        return Ok(path);
    }

    let mut new_path = Vec::with_capacity(path.len() * 3 / 2);
    for node in path {
        let split_len = node.num_splits(mode);
        if split_len <= 1 {
            new_path.push(node);
        } else {
            new_path.extend(node.split(mode, dict.lexicon(), subset, input));
        }
    }

    Ok(new_path)
}

pub(super) fn dump_path(path: &Vec<ResultNode>) {
    for (i, node) in path.iter().enumerate() {
        println!("{}: {}", i, node);
    }
}
