//! Shared bench fixtures: dictionary loader and sample corpora.
//!
//! Dictionary discovery (in priority order):
//!   1. `SUDACHI_DICT_PATH` env var (full path to a `system_*.dic` file)
//!   2. `~/.local/share/sudachi/sudachi-dictionary-latest/system_full.dic`
//!   3. Newest `~/.local/share/sudachi/sudachi-dictionary-*/system_full.dic`
//!
//! Panics with an actionable message if no dictionary can be found.

use std::path::PathBuf;
use std::sync::Arc;

use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;

pub fn load_dict() -> Arc<JapaneseDictionary> {
    let dict_path = resolve_dict_path();
    let cfg = Config::new(None, None, Some(dict_path.clone()))
        .unwrap_or_else(|e| panic!("failed to build sudachi Config from {dict_path:?}: {e}"));
    Arc::new(
        JapaneseDictionary::from_cfg(&cfg)
            .unwrap_or_else(|e| panic!("failed to load JapaneseDictionary from {dict_path:?}: {e}")),
    )
}

fn resolve_dict_path() -> PathBuf {
    if let Ok(p) = std::env::var("SUDACHI_DICT_PATH") {
        let path = PathBuf::from(p);
        assert!(path.exists(), "SUDACHI_DICT_PATH={path:?} does not exist");
        return path;
    }

    let home = std::env::var("HOME").expect("HOME must be set");
    let base = PathBuf::from(&home).join(".local/share/sudachi");

    let latest = base.join("sudachi-dictionary-latest/system_full.dic");
    if latest.exists() {
        return latest;
    }

    if let Ok(entries) = std::fs::read_dir(&base) {
        let mut candidates: Vec<PathBuf> = entries
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
        if let Some(p) = candidates.pop() {
            return p;
        }
    }

    panic!(
        "no sudachi dictionary found. Set SUDACHI_DICT_PATH or install one under \
         {base:?} (e.g. sudachi-dictionary-YYYYMMDD/system_full.dic)"
    );
}

/// Short, common Japanese sentences (~10–25 chars). Tokenizes fast; measures per-call overhead.
pub const SHORT_SENTENCES: &[&str] = &[
    "今日は良い天気ですね。",
    "私は学生です。",
    "東京駅まで何分かかりますか。",
    "コーヒーを一杯ください。",
    "明日また会いましょう。",
];

/// Medium passages (~80–150 chars). Representative of vocab-card sentences.
pub const MEDIUM_PASSAGES: &[&str] = &[
    "彼は毎朝六時に起きて、犬を連れて公園を散歩するのが日課になっている。",
    "新しいプロジェクトの締め切りが来週に迫っているので、今夜は残業しなければならない。",
    "日本語の文法を勉強するときは、例文をたくさん読むことが大切だと思います。",
];

/// One long doc (~400 chars). Stresses lattice build.
pub const LONG_DOC: &str = "\
日本の四季は、それぞれに独特の魅力を持っている。春には桜が咲き乱れ、人々は花見を楽しむ。\
夏は祭りや花火大会で賑わい、海や山へ出かける家族連れも多い。秋になると紅葉が美しく、\
寺社仏閣を訪れる旅行者で観光地はにぎわう。冬は雪景色の中で温泉に浸かり、こたつでみかんを\
食べるのが日本人の典型的な楽しみ方の一つだろう。こうした季節ごとの風物詩は、日本文化の\
重要な要素として、長い歴史の中で大切に受け継がれてきた。";
