use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn token_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut p = PathBuf::from(home);
    p.push(".config");
    p.push("copilot-money-cli");
    p.push("token");
    p
}

pub fn session_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut p = PathBuf::from(home);
    p.push(".config");
    p.push("copilot-money-cli");
    p.push("playwright-session");
    p
}

pub fn load_token(path: &Path) -> anyhow::Result<String> {
    let s = fs::read_to_string(path)?;
    let t = s.trim().to_string();
    if t.is_empty() {
        anyhow::bail!("empty token file");
    }
    Ok(t)
}

pub fn save_token(path: &Path, token: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(path)?;
    #[cfg(unix)]
    f.set_permissions(fs::Permissions::from_mode(0o600))?;
    f.write_all(token.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

pub fn ensure_private_dir(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn token_helper_path() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Dev/test path (only exists in a source checkout).
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/get_token.py"));

    // Installed layouts:
    // - tarball users: ./copilot + ./libexec/copilot-money-cli/get_token.py
    // - Homebrew: <prefix>/bin/copilot + <prefix>/libexec/copilot-money-cli/get_token.py
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join("libexec/copilot-money-cli/get_token.py"));
        candidates.push(dir.join("../libexec/copilot-money-cli/get_token.py"));
    }

    candidates.into_iter().find(|p| p.exists())
}

/// Command invocation for the Python token helper, including dependency bootstrapping.
#[derive(Debug, PartialEq, Eq)]
pub struct TokenHelperCommand {
    /// Program to execute for the helper invocation.
    pub program: OsString,
    /// Arguments passed to the helper program.
    pub args: Vec<OsString>,
}

impl TokenHelperCommand {
    /// Converts the value object into a process command ready for caller-specific args.
    pub fn into_command(self) -> Command {
        let mut command = Command::new(self.program);
        command.args(self.args);
        command
    }
}

/// Builds the token helper command using the current process PATH.
pub fn token_helper_command(helper: &Path) -> TokenHelperCommand {
    token_helper_command_with_path(helper, std::env::var_os("PATH"))
}

/// Builds the token helper command with an injected PATH, preferring uv when available.
pub fn token_helper_command_with_path(
    helper: &Path,
    path_env: Option<OsString>,
) -> TokenHelperCommand {
    if executable_on_path("uv", path_env.as_deref()) {
        return TokenHelperCommand {
            program: OsString::from("uv"),
            args: vec![
                OsString::from("run"),
                OsString::from("--with"),
                OsString::from("playwright"),
                OsString::from("python"),
                helper.as_os_str().to_os_string(),
            ],
        };
    }

    TokenHelperCommand {
        program: OsString::from("python3"),
        args: vec![helper.as_os_str().to_os_string()],
    }
}

fn executable_on_path(name: &str, path_env: Option<&std::ffi::OsStr>) -> bool {
    let Some(path_env) = path_env else {
        return false;
    };
    for dir in std::env::split_paths(path_env) {
        let candidate = dir.join(name);
        if is_executable_file(&candidate) {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
        && fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}
