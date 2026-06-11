use assert_cmd::prelude::*;
mod support;
use predicates::prelude::*;
use std::fs;
use std::process::Stdio;
use support::{
    config_path, create_empty_cache, granola, legacy_cache_path, seed_cache, sqlite_cache_path,
    temp_home_with_cache,
};

#[test]
fn top_level_version_flag_prints_package_version() {
    let home = temp_home_with_cache();

    granola(home.path())
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "granola {}",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn doctor_human_output_includes_package_version() {
    let home = temp_home_with_cache();

    granola(home.path())
        .arg("doctor")
        .env("GRANOLA_CLI_DISABLE_UPDATE_CHECK", "1")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "version: {}",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn notes_search_accepts_multiple_query_arguments() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["notes", "search", "attendees:will", "async config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Async Config Refinement"))
        .stdout(predicate::str::contains("not_async"))
        .stdout(predicate::str::contains("transcript"));
}

#[test]
fn notes_search_reads_query_from_piped_stdin() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let mut command = granola(home.path());
    command
        .args(["notes", "search"])
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
        .args(["notes", "search", "mint", "--output", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("id: not_mint"))
        .stdout(predicate::str::contains("title: Mint Planning"))
        .stdout(predicate::str::contains("cached: full"))
        .stdout(predicate::str::contains("+").not());
}

#[test]
fn search_fields_id_emits_plain_ids_for_pipelines() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["notes", "search", "mint", "--fields", "id", "--limit", "1"])
        .assert()
        .success()
        .stdout("not_mint\n");
}

#[test]
fn search_fields_can_emit_cached_summary_and_transcript_text() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args([
            "notes",
            "search",
            "async",
            "--fields",
            "id,summary_text,transcript",
            "--output",
            "text",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "not_async\tWe discussed async config rollout details.\tspeaker: mint alpha",
        ));
}

#[test]
fn search_redacts_selected_fields_and_json_payloads() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args([
            "notes",
            "search",
            "async",
            "--fields",
            "id,owner,summary_text",
            "--redact",
            "emails",
            "--output",
            "text",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("not_async\t[redacted email]\t"))
        .stdout(predicate::str::contains("chris@example.com").not());

    granola(home.path())
        .args([
            "notes",
            "search",
            "async",
            "--redact",
            "emails",
            "--output",
            "json-compact",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("[redacted email]"))
        .stdout(predicate::str::contains("chris@example.com").not());
}

#[test]
fn unknown_search_field_errors() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args([
            "notes",
            "search",
            "mint",
            "--fields",
            "id,NODOESNTEXIST",
            "--output",
            "text",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown field 'NODOESNTEXIST'"))
        .stderr(predicate::str::contains("available fields: id, title"));
}

#[test]
fn search_supports_created_since_filter() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["notes", "search", "mint", "--since", "1d", "--fields", "id"])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn table_output_falls_back_to_list_when_terminal_is_narrow() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .env("COLUMNS", "20")
        .args(["notes", "search", "mint"])
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
        .args(["notes", "search", "mint", "--output", "table"])
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
fn sync_help_uses_singular_include_transcript_flag() {
    let home = temp_home_with_cache();

    granola(home.path())
        .args(["sync", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--include-transcript"))
        .stdout(predicate::str::contains("--include-transcripts").not());
}

#[test]
fn notes_get_help_uses_fields_for_transcripts() {
    let home = temp_home_with_cache();

    granola(home.path())
        .args(["notes", "get", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--fields"))
        .stdout(predicate::str::contains("--include-transcript").not())
        .stdout(predicate::str::contains("--view").not());
}

#[test]
fn notes_fields_lists_human_and_json_metadata() {
    let home = temp_home_with_cache();

    granola(home.path())
        .args(["notes", "fields", "get"])
        .assert()
        .success()
        .stdout(predicate::str::contains("summary"))
        .stdout(predicate::str::contains("transcript"));

    granola(home.path())
        .args(["notes", "fields", "get", "--output", "json-compact"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"transcript\""))
        .stdout(predicate::str::contains("\"requires_transcript\":true"));
}

#[test]
fn get_many_requires_explicit_selector_before_auth() {
    let home = temp_home_with_cache();

    granola(home.path())
        .args(["notes", "get-many"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no note selector provided"));
}

#[test]
fn context_requires_explicit_selector_before_auth() {
    let home = temp_home_with_cache();

    granola(home.path())
        .args(["context"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no note selector provided"));
}

#[test]
fn search_explains_when_existing_cache_has_no_matches() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["notes", "search", "definitelyabsent"])
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
        .args(["notes", "search", "mint"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No notes are cached yet."));
}

#[test]
fn search_errors_when_no_query_is_available() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args(["notes", "search"])
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

#[test]
fn cache_maintenance_commands_report_and_export_cache() {
    let home = temp_home_with_cache();
    seed_cache(home.path());
    let export_path = home.path().join("cache.jsonl");

    granola(home.path())
        .args(["cache", "path"])
        .assert()
        .success()
        .stdout(predicate::str::contains("notes.sqlite"));

    granola(home.path())
        .args(["cache", "verify", "--output", "json-compact"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":true"));

    granola(home.path())
        .args(["cache", "vacuum"])
        .assert()
        .success()
        .stdout(predicate::str::contains("vacuumed cache"));

    granola(home.path())
        .args([
            "cache",
            "export",
            "--format",
            "jsonl",
            "-o",
            export_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let exported = fs::read_to_string(export_path).unwrap();
    assert!(exported.contains("not_async"));
    assert!(exported.contains("cached_transcript"));
}

#[test]
fn saved_search_views_round_trip_through_config_and_cache() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    granola(home.path())
        .args([
            "views",
            "create",
            "mint-view",
            "--query",
            "mint",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("saved view 'mint-view'"));

    assert!(config_path(home.path()).exists());

    granola(home.path())
        .args(["views", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mint-view"))
        .stdout(predicate::str::contains("search"));

    granola(home.path())
        .args(["views", "show", "mint-view"])
        .assert()
        .success()
        .stdout(predicate::str::contains("selector: query=mint"));

    granola(home.path())
        .args(["views", "run", "mint-view", "--output", "json-compact"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"count\":1"));

    granola(home.path())
        .args(["views", "delete", "mint-view"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted view 'mint-view'"));
}
