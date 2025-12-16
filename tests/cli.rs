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
fn hello_json_works() {
    Command::new(assert_cmd::cargo::cargo_bin!("copilot"))
        .args(["--output", "json", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\""));
}
