use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
