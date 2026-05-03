# Sudachi.rs Optimization Guide

Practical recipes for embedding sudachi-rs efficiently. The defaults are
fine for one-off CLI use; this guide covers the patterns that matter
when sudachi is in the hot path of a larger system (search indexers,
batch processors, language-learning pipelines).

All snippets assume:

```rust
use std::sync::Arc;
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::{Mode, OwnedMorpheme, Tokenize};
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;
```

---

## 1. Use `StatelessTokenizer<Arc<JapaneseDictionary>>` and let it pool

The `Tokenize::tokenize` impl on
`StatelessTokenizer<T: DictionaryAccess + Clone + 'static>` keeps a
thread-local pool of `StatefulTokenizer` instances keyed by the
underlying dictionary. The first call on a thread constructs and stores
a tokenizer; subsequent calls reuse it, avoiding the
~9 internal `Vec/String` allocations and the lattice initialization that
a fresh `StatefulTokenizer` requires.

```rust
let cfg = Config::new(None, None, Some(dict_path)).unwrap();
let dict = Arc::new(JapaneseDictionary::from_cfg(&cfg).unwrap());
let tokenizer = Arc::new(StatelessTokenizer::new(dict));

// Share `tokenizer` across rayon workers — each thread picks up its
// own pooled StatefulTokenizer transparently.
texts.par_iter().for_each(|t| {
    let result = tokenizer.tokenize(t, Mode::C, false).unwrap();
    consume(result);
});
```

There is no explicit `pooled_tokenize` method — the pool is the default
behavior of `tokenize`. If you need a non-`'static` borrow (e.g.
`StatelessTokenizer::new(&dict)`), construct a `StatefulTokenizer`
directly instead.

**Bench**: ~17% faster than the pre-pool implementation on
`stateless/short_x5`, `stateless/medium_x3`, and `stateless/long_doc`
benchmarks.

---

## 2. Batch many inputs with `tokenize_batch`

When tokenizing N inputs sequentially, `tokenize_batch` acquires the
thread-local pool once instead of N times. With the per-thread pool the
saving is small (one `RefCell` borrow + `HashMap` lookup), but the API
documents the recommended pattern:

```rust
let results: Vec<_> = tokenizer
    .tokenize_batch(&inputs, Mode::C, false)
    .unwrap();
```

For parallel batches, chunk the inputs and run `tokenize_batch` per
worker thread:

```rust
inputs.par_chunks(64).map(|chunk| {
    tokenizer.tokenize_batch(chunk, Mode::C, false)
}).collect::<Result<Vec<_>, _>>()?;
```

No `rayon` dependency is added by sudachi-rs — parallelism is the
caller's responsibility, made safe by the per-thread pool.

---

## 3. Snapshot morphemes with `Morpheme::into_owned`

`Morpheme<'a, T>` borrows from the parent `MorphemeList` (and through it
from the dictionary mmap and the input buffer). To escape this borrow —
for owned tokens, cross-thread sends, caches that survive a tokenizer
reset — the naive pattern allocates ~5 small `String`s per morpheme:

```rust
// Naive: 5+ small allocations per morpheme.
let token = Token {
    surface:        m.surface().to_owned(),
    dictionary_form: m.dictionary_form().to_owned(),
    normalized_form: m.normalized_form().to_owned(),
    reading_form:    m.reading_form().to_owned(),
    part_of_speech:  m.part_of_speech().to_vec(),
};
```

`Morpheme::into_owned()` packs all string fields into a single
`Box<str>` arena with byte-range indices, collapsing this to **one**
allocation per morpheme:

```rust
let owned: Vec<OwnedMorpheme> = list.iter().map(|m| m.into_owned()).collect();
// `owned` survives `list` going out of scope.
```

`OwnedMorpheme` exposes `&str`-returning accessors with the same names
as `Morpheme`. Use it for any pipeline stage that needs to keep
morpheme data past the originating `MorphemeList`'s lifetime.

**Bench**: 2.5× faster than the naive 5-clone pattern on
`morpheme_escape/into_owned` vs `morpheme_escape/naive_5_clones`.

---

## 4. Match parts of speech by ID, not by string

`Morpheme::part_of_speech()` returns `&[String]` — convenient but
expensive to filter against (string comparison per token, per check).
`Morpheme::part_of_speech_id()` returns a `u16` handle that uniquely
identifies the POS combination in the dictionary's grammar.

Pre-compute the IDs you care about once at startup using
`Grammar::get_part_of_speech_id`, then compare integers per token:

```rust
let grammar = dict.grammar();
let noun_id = grammar
    .get_part_of_speech_id(&["名詞", "普通名詞", "一般", "*", "*", "*"])
    .expect("noun POS not registered in this dictionary");
let verb_id = grammar
    .get_part_of_speech_id(&["動詞", "一般", "*", "*", "*", "*"])
    .expect("verb POS not registered in this dictionary");

for m in list.iter() {
    let pid = m.part_of_speech_id();
    if pid == noun_id || pid == verb_id {
        // ...
    }
}
```

For partial-match queries (e.g. "any noun, regardless of subcategory"),
build the set of matching IDs at startup by iterating the grammar's
`pos_list` once:

```rust
let noun_ids: Vec<u16> = (0..u16::MAX)
    .filter(|&id| {
        let comps = grammar.pos_components(id);
        comps.first().map(String::as_str) == Some("名詞")
    })
    .collect();
```

**Note**: `OwnedMorpheme::part_of_speech_id()` returns the same `u16`,
so this technique works equally well after `into_owned()`.

---

## 5. Multi-mode from a single lattice

Search-style consumers commonly tokenize the same input at both
`Mode::B` (medium granularity, suitable for keyword extraction) and
`Mode::C` (coarsest, suitable for compound names). Two separate calls
duplicate the most expensive work — input rewrite, lattice build,
best-path resolve, and path-rewrite plugins.

`tokenize_multi_mode` runs that shared prefix once and applies the
mode-specific `split_path` step per requested mode:

```rust
let results = tokenizer
    .tokenize_multi_mode(input, &[Mode::B, Mode::C], false)
    .unwrap();
let b_result = &results[0];
let c_result = &results[1];
```

All returned `MorphemeList`s share a single `Rc<RefCell<InputPart>>` —
one input clone is paid (for the first list); subsequent lists clone the
Rc, not the buffer. All N lists are independently usable past the call.

**Bench**: 1.73× faster than two sequential `tokenize` calls on
`multi_mode/multi_b_c` vs `multi_mode/two_calls_b_then_c`. The win
scales roughly with the number of modes (the shared prefix is amortized
across them).

**Implementation note**: As of `71b58647ee61` (May 2026), the per-list
`InputBuffer::clone()` is replaced with `MorphemeList::from_components_shared`
which clones only the input `Rc` for lists 2..N. Earlier versions cloned
the full buffer (~5–20 KB, ~9 internal Vec/String fields) per list. Net
effect on the bench was small (~3% on `multi_mode/multi_b_c`) but the
allocator-traffic reduction is meaningful for memory-pressure-sensitive
consumers.

---

## 6. What we deliberately did NOT do

A few optimizations were considered but proved infeasible or
unproductive — recording here so future readers don't redo the analysis.

### Borrowed OOV surface (`Cow<'a, str>`)

The OOV path at `analysis/stateful_tokenizer.rs:180` allocates a new
`String` per OOV word. The natural fix is a `Cow<'a, str>` borrowing
from the input buffer.

This is **not feasible without an architectural change**:
`WordInfoData.surface` must be owned `String` because the dictionary
path parses owned strings from the mmap binary. Switching to
`Cow<'a, str>` would propagate `'a` through `WordInfo<'a>` →
`ResultNode<'a>` → `MorphemeList<'a>`, which conflicts with the
canonical `tokenize → keep MorphemeList → reset → tokenize new text`
usage pattern.

The fallback "make the alloc faster" idea also doesn't apply:
`.to_owned()` on `&str` already uses `Vec::to_owned`, which is
single-allocation and pre-sized via `with_capacity(len)`. There is no
growth re-alloc to eliminate.

### Lattice `Vec<Vec<VNode>>` flat-CSR redesign

The lattice stores per-position node lists as `Vec<Vec<VNode>>`. This
has a known ~25% padding inefficiency (per the comment at
`analysis/lattice.rs:32`). A flat `Vec<VNode>` with per-position
offsets (CSR-style) would be cache-friendlier.

This would touch every lattice consumer (`insert`, `connect_node`,
`fill_top_path`) and is a substantial refactor. Worth doing but out
of scope for the current optimization series; the benches don't show
lattice ops as the dominant cost relative to allocations.
