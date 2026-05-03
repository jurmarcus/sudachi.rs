// Baseline (2026-05-04, M1 Pro/macOS, dict=system_full 20260428):
//   stateless/short_x5     ~24.0 µs   (208 K elem/s)
//   stateful/short_x5_hot  ~16.1 µs   (310 K elem/s)
//   stateless/medium_x3    ~46.0 µs   ( 65 K elem/s)
//   stateless/long_doc     ~70.8 µs   (8.1 MB/s)
//
// stateless is 1.5x slower than stateful on the same input — the gap is
// pure per-call StatefulTokenizer construction overhead.

mod common;

use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use sudachi::analysis::stateful_tokenizer::StatefulTokenizer;
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::{Mode, Tokenize};

use common::{load_dict, LONG_DOC, MEDIUM_PASSAGES, SHORT_SENTENCES};

fn bench_stateless_short(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let mut group = c.benchmark_group("stateless");
    group.throughput(Throughput::Elements(SHORT_SENTENCES.len() as u64));
    group.bench_function("short_x5", |b| {
        b.iter(|| {
            for s in SHORT_SENTENCES {
                let _ = tokenizer.tokenize(black_box(*s), Mode::C, false).unwrap();
            }
        });
    });
    group.finish();
}

fn bench_stateful_hot(c: &mut Criterion) {
    let dict = load_dict();
    let mut tokenizer = StatefulTokenizer::create(Arc::clone(&dict), false, Mode::C);
    let mut group = c.benchmark_group("stateful");
    group.throughput(Throughput::Elements(SHORT_SENTENCES.len() as u64));
    group.bench_function("short_x5_hot", |b| {
        b.iter(|| {
            for s in SHORT_SENTENCES {
                tokenizer.reset().push_str(black_box(*s));
                tokenizer.do_tokenize().unwrap();
            }
        });
    });
    group.finish();
}

fn bench_medium(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let mut group = c.benchmark_group("stateless");
    group.throughput(Throughput::Elements(MEDIUM_PASSAGES.len() as u64));
    group.bench_function("medium_x3", |b| {
        b.iter(|| {
            for s in MEDIUM_PASSAGES {
                let _ = tokenizer.tokenize(black_box(*s), Mode::C, false).unwrap();
            }
        });
    });
    group.finish();
}

fn bench_long_doc(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let mut group = c.benchmark_group("stateless");
    group.throughput(Throughput::Bytes(LONG_DOC.len() as u64));
    group.bench_function("long_doc", |b| {
        b.iter(|| {
            let _ = tokenizer.tokenize(black_box(LONG_DOC), Mode::C, false).unwrap();
        });
    });
    group.finish();
}

fn bench_batch_short(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let mut group = c.benchmark_group("stateless");
    group.throughput(Throughput::Elements(SHORT_SENTENCES.len() as u64));
    group.bench_function("batch_short_x5", |b| {
        b.iter(|| {
            let _ = tokenizer
                .tokenize_batch(black_box(SHORT_SENTENCES), Mode::C, false)
                .unwrap();
        });
    });
    group.finish();
}

fn bench_multi_mode_b_c(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let s = MEDIUM_PASSAGES[0];
    let mut group = c.benchmark_group("multi_mode");

    group.bench_function("two_calls_b_then_c", |b| {
        b.iter(|| {
            let _ = tokenizer.tokenize(black_box(s), Mode::B, false).unwrap();
            let _ = tokenizer.tokenize(black_box(s), Mode::C, false).unwrap();
        });
    });

    group.bench_function("multi_b_c", |b| {
        b.iter(|| {
            let _ = tokenizer
                .tokenize_multi_mode(black_box(s), &[Mode::B, Mode::C], false)
                .unwrap();
        });
    });

    group.finish();
}

fn bench_morpheme_escape(c: &mut Criterion) {
    let dict = load_dict();
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));
    let lists: Vec<_> = MEDIUM_PASSAGES
        .iter()
        .map(|p| tokenizer.tokenize(p, Mode::C, false).unwrap())
        .collect();

    let total_morphemes: usize = lists.iter().map(|l| l.len()).sum();
    let mut group = c.benchmark_group("morpheme_escape");
    group.throughput(Throughput::Elements(total_morphemes as u64));

    group.bench_function("into_owned", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(total_morphemes);
            for l in &lists {
                for m in l.iter() {
                    out.push(m.into_owned());
                }
            }
            black_box(out);
        });
    });

    group.bench_function("naive_5_clones", |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(total_morphemes);
            for l in &lists {
                for m in l.iter() {
                    out.push((
                        m.surface().to_string(),
                        m.dictionary_form().to_owned(),
                        m.normalized_form().to_owned(),
                        m.reading_form().to_owned(),
                        m.part_of_speech().to_vec(),
                    ));
                }
            }
            black_box(out);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_stateless_short,
    bench_stateful_hot,
    bench_medium,
    bench_long_doc,
    bench_batch_short,
    bench_multi_mode_b_c,
    bench_morpheme_escape,
);
criterion_main!(benches);
