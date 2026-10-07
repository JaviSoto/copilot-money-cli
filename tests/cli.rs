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

#[cfg(unix)]
fn credential_helper_command(
    tmp_home: &tempfile::TempDir,
    trace_path: &std::path::Path,
) -> std::process::Command {
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin!("copilot"));
    command
        .args([
            "auth",
            "login",
            "--mode",
            "credentials",
            "--no-persist-session",
            "--timeout-seconds",
            "1",
        ])
        .env("HOME", tmp_home.path())
        .env(
            "PYTHONPATH",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/token_helper"),
        )
        .env("COPILOT_TEST_TRACE", trace_path)
        .env("CODEX_INTEGRATIONS_VENV_REEXEC", "1")
        .env("COPILOT_XVFB_REEXEC", "1")
        .env_remove("CODEX_SECRET_FD")
        .env_remove("COPILOT_TOKEN")
        .env_remove("COPILOT_TOKEN_FILE");
    command
}

#[cfg(unix)]
fn private_fixture_pipe() -> (std::os::fd::OwnedFd, String) {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

    let mut descriptors = [0; 2];
    assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
    let reader = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
    let writer = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
    let mut writer = std::fs::File::from(writer);
    writer
        .write_all(br#"{"email":"fixture@example.test","password":"fixture-only"}"#)
        .unwrap();
    drop(writer);

    let descriptor = reader.as_raw_fd();
    assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) }, 0);
    (reader, descriptor.to_string())
}

#[cfg(unix)]
#[test]
fn credential_login_transports_private_fd_through_real_python_helper() {
    use std::os::fd::AsRawFd;

    const FIXTURE_TOKEN: &str = "eyJhbGciOiJub25lIn0.eyJleHAiOjQxMDI0NDQ4MDB9.fixture";
    let tmp_home = tempfile::tempdir().unwrap();
    let trace_path = tmp_home.path().join("fixture-trace.txt");
    let (reader, descriptor) = private_fixture_pipe();
    assert_ne!(reader.as_raw_fd(), libc::STDIN_FILENO);
    let mut command = credential_helper_command(&tmp_home, &trace_path);
    command.env("CODEX_SECRET_FD", descriptor);

    let output = command.output().unwrap();
    drop(reader);
    let fixture_trace = std::fs::read_to_string(&trace_path)
        .unwrap_or_else(|_| "<no fixture trace was written>".to_string());

    assert!(
        output.status.success(),
        "fixture login failed: {}; trace: {fixture_trace}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture-only"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("fixture-only"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains(FIXTURE_TOKEN));
    let saved_token = tmp_home.path().join(".config/copilot-money-cli/token");
    assert_eq!(
        std::fs::read_to_string(saved_token).unwrap().trim(),
        FIXTURE_TOKEN
    );

    let trace = std::fs::read_to_string(trace_path).unwrap();
    assert!(trace.contains("browser:stubbed-no-network"));
    assert!(trace.contains("filled-email:fixture@example.test"));
    assert!(trace.contains("filled-password:fixture"));
    assert!(trace.contains("browser:fixture-request-token"));
    assert!(trace.contains("browser-context:closed"));
    assert!(!trace.contains("fixture-only"));
    assert!(!trace.contains("network request sent"));
    assert!(!trace.contains("gmail:"));
}

#[cfg(unix)]
#[test]
fn credential_login_fails_closed_without_descriptor_instead_of_prompting() {
    let tmp_home = tempfile::tempdir().unwrap();
    let trace_path = tmp_home.path().join("fixture-trace.txt");
    let output = credential_helper_command(&tmp_home, &trace_path)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("credentials login helper failed; no token was saved"));
    assert!(!stderr.contains("Paste a Copilot bearer token"));
    assert!(
        !tmp_home
            .path()
            .join(".config/copilot-money-cli/token")
            .exists()
    );
    assert!(!trace_path.exists());
}

#[cfg(unix)]
#[test]
fn credential_login_fails_closed_with_malformed_descriptor_instead_of_prompting() {
    let tmp_home = tempfile::tempdir().unwrap();
    let trace_path = tmp_home.path().join("fixture-trace.txt");
    let mut command = credential_helper_command(&tmp_home, &trace_path);
    command.env("CODEX_SECRET_FD", "not-a-descriptor");
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("credentials login helper failed; no token was saved"));
    assert!(!stderr.contains("Paste a Copilot bearer token"));
    assert!(
        !tmp_home
            .path()
            .join(".config/copilot-money-cli/token")
            .exists()
    );
    assert!(!trace_path.exists());
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
