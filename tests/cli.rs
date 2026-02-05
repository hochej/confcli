use assert_cmd::Command;
use predicates::prelude::*;

fn confcli() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("confcli").unwrap()
}

#[test]
fn help_flag() {
    confcli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("scrappy little Confluence CLI"));
}

#[test]
fn version_flag() {
    confcli()
        .args(["--version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("confcli"));
}

#[test]
fn auth_help() {
    confcli()
        .args(["auth", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Manage authentication"));
}

#[test]
fn space_help() {
    confcli()
        .args(["space", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List and inspect spaces"));
}

#[test]
fn page_help() {
    confcli()
        .args(["page", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "List, view, create, and manage pages",
        ));
}

#[test]
fn search_help() {
    confcli()
        .args(["search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Search content"));
}

#[test]
fn attachment_help() {
    confcli()
        .args(["attachment", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "List, download, upload, and manage attachments",
        ));
}

#[test]
fn label_help() {
    confcli()
        .args(["label", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "List, add, and remove page labels",
        ));
}

#[test]
fn completions_bash() {
    confcli()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("confcli"));
}

#[test]
fn completions_zsh() {
    confcli()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("confcli"));
}

#[test]
fn completions_fish() {
    confcli()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("confcli"));
}

#[test]
fn invalid_output_format() {
    confcli()
        .args(["space", "list", "-o", "xml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'xml'"));
}

#[test]
fn search_requires_query() {
    confcli()
        .args(["search"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("<QUERY>").or(predicate::str::contains("required")));
}

#[test]
#[cfg(feature = "write")]
fn page_create_missing_space() {
    confcli()
        .args(["page", "create", "--title", "Test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--space"));
}

#[test]
fn dry_run_flag_accepted() {
    // --dry-run should be accepted as a global flag (not rejected by arg parsing).
    // We test with --help to avoid needing credentials.
    confcli()
        .args(["--dry-run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn auth_status_not_logged_in() {
    let temp_dir = tempfile::tempdir().unwrap();
    confcli()
        .args(["auth", "status"])
        // Run from a temp dir so dotenvy doesn't load the project's .env
        .current_dir(temp_dir.path())
        .env("XDG_CONFIG_HOME", temp_dir.path())
        .env_remove("CONFLUENCE_DOMAIN")
        .env_remove("CONFLUENCE_BASE_URL")
        .env_remove("CONFLUENCE_URL")
        .env_remove("CONFLUENCE_EMAIL")
        .env_remove("CONFLUENCE_TOKEN")
        .env_remove("CONFLUENCE_BEARER_TOKEN")
        .assert()
        .success()
        .stdout(predicate::str::contains("Not logged in"));
}

#[test]
fn page_history_help() {
    confcli()
        .args(["page", "history", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("version history"));
}

#[test]
fn page_open_help() {
    confcli()
        .args(["page", "open", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("browser"));
}
