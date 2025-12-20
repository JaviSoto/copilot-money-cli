use assert_cmd::Command;
use predicates::prelude::*;

fn cmd_with_fixtures(tmp_home: &tempfile::TempDir) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("copilot"));
    cmd.env("HOME", tmp_home.path());
    cmd.env_remove("COPILOT_TOKEN");
    cmd.env_remove("COPILOT_TOKEN_FILE");
    cmd.env("COPILOT_FIXTURES_DIR", "tests/fixtures/graphql");
    cmd
}

#[test]
fn version_works() {
    let tmp_home = tempfile::tempdir().unwrap();
    cmd_with_fixtures(&tmp_home)
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("copilot-money-cli"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn dashdash_version_works() {
    let tmp_home = tempfile::tempdir().unwrap();
    cmd_with_fixtures(&tmp_home)
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("copilot"))
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn auth_status_json_works_without_token() {
    let tmp_home = tempfile::tempdir().unwrap();
    cmd_with_fixtures(&tmp_home)
        .args(["--output", "json", "auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("token_configured"));
}

#[test]
fn auth_login_dry_run_works() {
    let tmp_home = tempfile::tempdir().unwrap();
    cmd_with_fixtures(&tmp_home)
        .args(["--dry-run", "auth", "login"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run: would obtain token"));
}

#[test]
fn auth_set_token_and_logout_work() {
    let tmp_home = tempfile::tempdir().unwrap();

    cmd_with_fixtures(&tmp_home)
        .args(["--token", "dummy_token", "auth", "set-token"])
        .assert()
        .success()
        .stdout(predicate::str::contains("saved token"));

    cmd_with_fixtures(&tmp_home)
        .args(["--output", "json", "auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"token_configured\""))
        .stdout(predicate::str::contains("\"true\""));

    cmd_with_fixtures(&tmp_home)
        .args(["auth", "logout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("removed token"));

    cmd_with_fixtures(&tmp_home)
        .args(["--output", "json", "auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"token_configured\""))
        .stdout(predicate::str::contains("\"false\""));
}

#[test]
fn mutations_require_yes_or_dry_run() {
    let tmp_home = tempfile::tempdir().unwrap();

    cmd_with_fixtures(&tmp_home)
        .args(["--dry-run", "transactions", "review", "txn_1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run: would mark reviewed"));

    cmd_with_fixtures(&tmp_home)
        .args(["transactions", "review", "txn_1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing to write"))
        .stderr(predicate::str::contains("--yes"));

    cmd_with_fixtures(&tmp_home)
        .args([
            "--yes",
            "--output",
            "json",
            "transactions",
            "review",
            "txn_1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": \"txn_1\""));
}

#[test]
fn tags_list_and_create_work() {
    let tmp_home = tempfile::tempdir().unwrap();

    cmd_with_fixtures(&tmp_home)
        .args(["tags", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Shopping"));

    cmd_with_fixtures(&tmp_home)
        .args(["--dry-run", "tags", "create", "New Tag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run: would create tag"));

    cmd_with_fixtures(&tmp_home)
        .args(["tags", "create", "New Tag"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    cmd_with_fixtures(&tmp_home)
        .args(["--yes", "tags", "create", "New Tag"])
        .assert()
        .success()
        .stdout(predicate::str::contains("tag_new"));
}

#[test]
fn tags_delete_requires_yes_or_dry_run() {
    let tmp_home = tempfile::tempdir().unwrap();

    cmd_with_fixtures(&tmp_home)
        .args(["--dry-run", "tags", "delete", "tag_1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run: would delete tag"));

    cmd_with_fixtures(&tmp_home)
        .args(["tags", "delete", "tag_1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    cmd_with_fixtures(&tmp_home)
        .args(["--yes", "--output", "json", "tags", "delete", "tag_1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deleted\""))
        .stdout(predicate::str::contains("\"true\""));
}
