//! Allocation profile of a tokenization workload.
//!
//! Run with:
//!     cargo run --release --example dhat_alloc
//!
//! Output: dhat-heap.json in CWD. View at https://nnethercote.github.io/dh_view/dh_view.html
//! or open dhat-heap.json with the dhat-viewer crate.
//!
//! What this measures:
//! - Total bytes allocated and total allocation count over the run.
//! - Per-call-stack allocation totals (which functions allocate most).
//! - Maximum heap size during the run.
//!
//! Used to inform the lattice flat-CSR plan: confirms whether per-position
//! Vec growth/headers actually dominate allocation cost during tokenize.

use std::path::PathBuf;
use std::sync::Arc;

use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::{Mode, Tokenize};
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn resolve_dict_path() -> PathBuf {
    if let Ok(p) = std::env::var("SUDACHI_DICT_PATH") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").expect("HOME must be set");
    let base = PathBuf::from(&home).join(".local/share/sudachi");
    let latest = base.join("sudachi-dictionary-latest/system_full.dic");
    if latest.exists() {
        return latest;
    }
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("sudachi-dictionary-"))
        })
        .map(|p| p.join("system_full.dic"))
        .filter(|p| p.exists())
        .collect();
    candidates.sort();
    candidates.pop().expect("no sudachi dict found")
}

const SHORT_SENTENCES: &[&str] = &[
    "今日は良い天気ですね。",
    "私は学生です。",
    "東京駅まで何分かかりますか。",
    "コーヒーを一杯ください。",
    "明日また会いましょう。",
];

const MEDIUM_PASSAGES: &[&str] = &[
    "彼は毎朝六時に起きて、犬を連れて公園を散歩するのが日課になっている。",
    "新しいプロジェクトの締め切りが来週に迫っているので、今夜は残業しなければならない。",
    "日本語の文法を勉強するときは、例文をたくさん読むことが大切だと思います。",
];

const LONG_DOC: &str = "\
日本の四季は、それぞれに独特の魅力を持っている。春には桜が咲き乱れ、人々は花見を楽しむ。\
夏は祭りや花火大会で賑わい、海や山へ出かける家族連れも多い。秋になると紅葉が美しく、\
寺社仏閣を訪れる旅行者で観光地はにぎわう。冬は雪景色の中で温泉に浸かり、こたつでみかんを\
食べるのが日本人の典型的な楽しみ方の一つだろう。こうした季節ごとの風物詩は、日本文化の\
重要な要素として、長い歴史の中で大切に受け継がれてきた。";

fn main() {
    // Profile boundary 1: dictionary load + tokenizer setup. Allocations
    // here are amortized away in real usage; we want to ignore them when
    // analyzing per-tokenize cost.
    let dict_path = resolve_dict_path();
    let cfg = Config::new(None, None, Some(dict_path)).expect("config");
    let dict = Arc::new(JapaneseDictionary::from_cfg(&cfg).expect("dict load"));
    let tokenizer = StatelessTokenizer::new(Arc::clone(&dict));

    // Warm the per-thread pool with one tokenize so the pool entry exists
    // before we start the profiler. This avoids attributing pool init to
    // the steady-state numbers.
    let _ = tokenizer.tokenize("今日", Mode::C, false).unwrap();

    // Profile boundary 2: start dhat HERE so only steady-state per-tokenize
    // allocations are captured.
    let _profiler = dhat::Profiler::new_heap();

    // Workload: 1000 iterations across short/medium/long. This gives dhat
    // a thick sample of typical access patterns. Iterations are sized so
    // total allocations are well above dhat's per-site reporting threshold.
    for _ in 0..200 {
        for s in SHORT_SENTENCES {
            let _ = tokenizer.tokenize(s, Mode::C, false).unwrap();
        }
        for s in MEDIUM_PASSAGES {
            let _ = tokenizer.tokenize(s, Mode::C, false).unwrap();
        }
        let _ = tokenizer.tokenize(LONG_DOC, Mode::C, false).unwrap();
    }

    // _profiler dropped here → writes dhat-heap.json to CWD.
}
