mod support;

use serde_json::{json, Value};
use support::{granola, seed_cache, temp_home_with_cache};

#[test]
fn default_table_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(home.path(), &["search", "Planning"], &[("COLUMNS", "200")]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn adaptive_default_list_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(home.path(), &["search", "Planning"], &[("COLUMNS", "20")]);

    insta::assert_snapshot!(stdout);
}

#[test]
fn explicit_table_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(
        home.path(),
        &["search", "Planning", "--output", "table"],
        &[("COLUMNS", "20")],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn explicit_list_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(
        home.path(),
        &["search", "Planning", "--output", "list"],
        &[],
    );

    insta::assert_snapshot!(stdout);
}

#[test]
fn json_compact_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(
        home.path(),
        &["search", "Planning", "--output", "json-compact"],
        &[],
    );

    insta::assert_snapshot!(normalize_json_stdout(&stdout, home.path(), true));
}

#[test]
fn json_pretty_output_snapshot() {
    let home = temp_home_with_cache();
    seed_cache(home.path());

    let stdout = run_stdout(
        home.path(),
        &["search", "Planning", "--output", "json-pretty"],
        &[],
    );

    insta::assert_snapshot!(normalize_json_stdout(&stdout, home.path(), false));
}

fn run_stdout(home: &std::path::Path, args: &[&str], envs: &[(&str, &str)]) -> String {
    let mut command = granola(home);
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn normalize_json_stdout(stdout: &str, home: &std::path::Path, compact: bool) -> String {
    let mut value: Value = serde_json::from_str(stdout).unwrap();
    normalize_json_value(&mut value, home);
    if compact {
        serde_json::to_string(&value).unwrap()
    } else {
        format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
    }
}

fn normalize_json_value(value: &mut Value, home: &std::path::Path) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "rank" {
                    *value = json!("<rank>");
                } else if key == "path" {
                    *value = json!("$CACHE/notes.sqlite");
                } else {
                    normalize_json_value(value, home);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_json_value(item, home);
            }
        }
        Value::String(text) => {
            *text = text.replace(&home.display().to_string(), "$HOME");
        }
        _ => {}
    }
}
