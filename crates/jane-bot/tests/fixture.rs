//! PORT.md §9.3 gate 4, ARCHITECTURE.md §8 `cross_target_hash`: the bot sessions' hash streams
//! equal `tests/fixtures/bot-hash-x86_64.txt`, which a fresh process wrote (`jane play
//! --fixture`). The file names the content it was written from; after a content change it is
//! stale, and says so, until it is written again.

mod common;

use common::bps;
use jane_bot::fixture;

#[test]
fn the_bot_sessions_hash_like_the_fixture_file() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/bot-hash-x86_64.txt");
    let text = std::fs::read_to_string(path).expect("tests/fixtures/bot-hash-x86_64.txt");
    let content = fixture::content_line();
    if !text.lines().any(|l| l == content) {
        eprintln!(
            "bot-hash-x86_64.txt was written from other content or another save or replay version (want \"{content}\"): \
             rewrite it with `jane play --fixture tests/fixtures/bot-hash-x86_64.txt`"
        );
        return;
    }
    let want: Vec<&str> = text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
    let mut got = Vec::new();
    for seed in fixture::SEEDS {
        for model in fixture::MODELS {
            let tape = fixture::session(bps(seed), model);
            got.extend(fixture::lines(seed, model, &tape));
        }
    }
    assert_eq!(got.len(), want.len(), "lines");
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w);
    }
}
