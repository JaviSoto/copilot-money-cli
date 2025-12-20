use copilot_money_cli::config::{load_token, save_token, token_path};
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn save_and_load_token_work_and_permissions_are_locked_down() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("conf").join("token");
    save_token(&p, "test_token").unwrap();
    let loaded = load_token(&p).unwrap();
    assert_eq!(loaded, "test_token");

    #[cfg(unix)]
    {
        let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn load_token_rejects_empty_file() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("token");
    fs::write(&p, "\n").unwrap();
    assert!(load_token(&p).is_err());
}

#[test]
fn token_path_has_expected_suffix() {
    let p = token_path();
    let s = p.to_string_lossy();
    assert!(
        s.ends_with("/.config/copilot-money-cli/token")
            || s.ends_with("\\.config\\copilot-money-cli\\token")
    );
}
