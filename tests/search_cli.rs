use assert_cmd::prelude::*;
mod support;
use predicates::prelude::*;
use std::fs;
use std::process::Stdio;
use support::{
    create_empty_cache, granola, legacy_cache_path, seed_cache, sqlite_cache_path,
    temp_home_with_cache,
};

#[test]
fn top_level_search_accepts_multiple_query_arguments() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["search", "attendees:will", "async config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Async Config Refinement"))
        .stdout(predicate::str::contains("not_async"))
        .stdout(predicate::str::contains("transcript"));
}

#[test]
fn top_level_search_reads_query_from_piped_stdin() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let mut command = granola(home.path());
    command
        .args(["search"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"attendees:will \"async config\"").unwrap();
    }
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Async Config Refinement"));
    assert!(stdout.contains("not_async"));
}

#[test]
fn notes_search_accepts_stdin_flag_and_limit() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let mut command = granola(home.path());
    command
        .args(["notes", "search", "--stdin", "--limit", "1"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"mint").unwrap();
    }
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("not_").count(), 1);
}

#[test]
fn search_supports_list_output() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["search", "mint", "--output", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id: not_mint"))
        .stdout(predicate::str::contains("title: Mint Planning"))
        .stdout(predicate::str::contains("cached: full"))
        .stdout(predicate::str::contains("+").not());
}

#[test]
fn table_output_falls_back_to_list_when_terminal_is_narrow() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .env("COLUMNS", "20")
        .args(["search", "mint"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id: not_mint"))
        .stdout(predicate::str::contains("title: Mint Planning"))
        .stdout(predicate::str::contains("+").not());
}

#[test]
fn explicit_table_output_is_respected_when_terminal_is_narrow() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .env("COLUMNS", "20")
        .args(["search", "mint", "--output", "table"])
        .assert()
        .success()
        .stdout(predicate::str::contains("+----------------"))
        .stdout(predicate::str::contains("| not_mint"));
}

#[test]
fn json_output_can_be_compact_or_pretty() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["cache", "status", "--output", "json-compact"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{\""))
        .stdout(predicate::str::contains("\n  \"").not());

    granola(home.path())
        .args(["cache", "status", "--output", "json-pretty"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{\n  \""));

    granola(home.path())
        .args(["cache", "status", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{\n  \""));
}

#[test]
fn compact_flag_is_not_supported() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["cache", "status", "--output", "json", "--compact"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--compact'"));
}

#[test]
fn search_explains_when_existing_cache_has_no_matches() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["search", "definitelyabsent"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No cached notes matched 'definitelyabsent'.",
        ))
        .stdout(predicate::str::contains(
            "Searched 3 cached summary note(s)",
        ));
}

#[test]
fn search_explains_when_cache_index_is_empty() {
    let home = temp_home_with_cache();
    create_empty_cache(home.path());

    granola(home.path())
        .args(["search", "mint"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No notes are cached yet."));
}

#[test]
fn search_errors_when_no_query_is_available() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["search"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "provide a search query as arguments or pass one on stdin",
        ));
}

#[test]
fn cache_status_reports_sqlite_counts_and_clear_removes_caches() {
    let home = temp_home_with_cache();
    seed_cache(home.path());
    let legacy_path = legacy_cache_path(home.path());
    fs::write(&legacy_path, "{}").unwrap();

    granola(home.path())
        .args(["cache", "status", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"summaries\": 3"))
        .stdout(predicate::str::contains("\"hydrated_notes\": 2"))
        .stdout(predicate::str::contains("\"transcript_notes\": 1"))
        .stdout(predicate::str::contains("\"legacy_json_exists\": true"));

    granola(home.path())
        .args(["cache", "clear"])
        .assert()
        .success();

    assert!(!sqlite_cache_path(home.path()).exists());
    assert!(!legacy_path.exists());
}
