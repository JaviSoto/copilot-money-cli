use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn version_works() {
    Command::new(assert_cmd::cargo::cargo_bin!("copilot"))
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("copilot-money-api"));
}

#[test]
fn auth_status_json_works_without_token() {
    Command::new(assert_cmd::cargo::cargo_bin!("copilot"))
        .args(["--output", "json", "auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("token_configured"));
}
