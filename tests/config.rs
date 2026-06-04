use copilot_money_cli::config::{
    load_token, save_token, token_helper_command_with_path, token_path,
};
use std::ffi::OsString;
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

#[test]
fn token_helper_command_uses_uv_when_available_on_path() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let uv = bin.join("uv");
    fs::write(&uv, "").unwrap();
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&uv).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&uv, permissions).unwrap();
    }

    let command = token_helper_command_with_path(
        std::path::Path::new("/tmp/get_token.py"),
        Some(OsString::from(bin.as_os_str())),
    );

    assert_eq!(command.program, "uv");
    assert_eq!(
        command
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["run", "--with", "playwright", "python", "/tmp/get_token.py"]
    );
}

#[test]
#[cfg(unix)]
fn token_helper_command_ignores_non_executable_uv_on_path() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("uv"), "").unwrap();

    let command = token_helper_command_with_path(
        std::path::Path::new("/tmp/get_token.py"),
        Some(OsString::from(bin.as_os_str())),
    );

    assert_eq!(command.program, "python3");
    assert_eq!(
        command
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["/tmp/get_token.py"]
    );
}

#[test]
fn token_helper_command_falls_back_to_python3_without_uv() {
    let tmp = tempfile::tempdir().unwrap();
    let command = token_helper_command_with_path(
        std::path::Path::new("/tmp/get_token.py"),
        Some(OsString::from(tmp.path().as_os_str())),
    );

    assert_eq!(command.program, "python3");
    assert_eq!(
        command
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        vec!["/tmp/get_token.py"]
    );
}
