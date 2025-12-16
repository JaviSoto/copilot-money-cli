use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub fn token_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    let mut p = PathBuf::from(home);
    p.push(".config");
    p.push("copilot-money-api");
    p.push("token");
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
