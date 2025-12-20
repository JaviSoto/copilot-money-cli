use std::path::PathBuf;

use anyhow::Context;

use crate::client::CopilotClient;
use crate::config::{
    ensure_private_dir, load_token, save_token, session_path, token_helper_path, token_path,
};

use super::render::{KeyValueRow, render_output};
use super::{AuthCmd, AuthLoginMode, Cli};

pub(super) fn run_auth(cli: &Cli, client: &CopilotClient, cmd: AuthCmd) -> anyhow::Result<()> {
    match cmd {
        AuthCmd::Status => {
            let token = match cli.token.clone() {
                Some(t) => Some(("env".to_string(), t)),
                None => {
                    let p = cli.token_file.clone().unwrap_or_else(token_path);
                    load_token(&p).ok().map(|t| ("file".to_string(), t))
                }
            };

            let mut rows = Vec::new();
            rows.push(KeyValueRow {
                key: "token_configured".to_string(),
                value: token.is_some().to_string(),
            });

            let valid = token.as_ref().map(|_| client.try_user_query().is_ok());
            rows.push(KeyValueRow {
                key: "token_valid".to_string(),
                value: valid
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            });

            render_output(cli, rows)
        }
        AuthCmd::Login(args) => {
            if cli.dry_run {
                println!("dry-run: would obtain token via browser helper (tools/get_token.py)");
                return Ok(());
            }

            let mut token: Option<String> = None;

            if let Some(helper) = token_helper_path() {
                let mut cmd = std::process::Command::new("python3");
                cmd.arg(helper);
                cmd.args(["--timeout-seconds", &args.timeout_seconds.to_string()]);

                if args.persist_session {
                    let dir = cli.session_dir.clone().unwrap_or_else(session_path);
                    ensure_private_dir(&dir)?;
                    cmd.args(["--user-data-dir", dir.to_string_lossy().as_ref()]);
                }

                match args.mode {
                    AuthLoginMode::Interactive => {
                        cmd.args(["--mode", "interactive", "--headful"]);
                    }
                    AuthLoginMode::EmailLink => {
                        cmd.args(["--mode", "email-link"]);
                        if let Some(email) = &args.email {
                            cmd.args(["--email", email]);
                        }
                        if let Some(p) = args.secrets_file {
                            cmd.args(["--secrets-file", p.to_string_lossy().as_ref()]);
                        }
                    }
                    AuthLoginMode::Credentials => {
                        cmd.args(["--mode", "credentials"]);
                        let p = args.secrets_file.clone().unwrap_or_else(|| {
                            let mut p = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
                            p.push(".codex");
                            p.push("secrets");
                            p.push("copilot_money");
                            p
                        });
                        cmd.args(["--secrets-file", p.to_string_lossy().as_ref()]);
                    }
                };

                match cmd.output() {
                    Ok(out) => {
                        if out.status.success() {
                            let t = String::from_utf8(out.stdout)?.trim().to_string();
                            if !t.is_empty() {
                                token = Some(t);
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            eprintln!(
                                "warning: token helper failed; falling back to manual token entry\n\n{stderr}",
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "warning: token helper failed to start; falling back to manual token entry\n\n{e}",
                        );
                    }
                }
            }

            if token.is_none() {
                eprintln!(
                    "Paste a Copilot bearer token from your browser network inspector (Authorization: Bearer …)",
                );
                let t = rpassword::prompt_password("Token (input hidden): ")?;
                if t.trim().is_empty() {
                    anyhow::bail!("empty token");
                }
                token = Some(t.trim().to_string());
            }

            let p = cli.token_file.clone().unwrap_or_else(token_path);
            save_token(&p, token.as_ref().unwrap())?;

            println!("saved token to {}", p.display());
            Ok(())
        }
        AuthCmd::Refresh(args) => {
            if cli.dry_run {
                println!("dry-run: would refresh token via persisted session");
                return Ok(());
            }

            let dir = cli.session_dir.clone().unwrap_or_else(session_path);
            if !dir.exists() {
                anyhow::bail!(
                    "no persisted session found at {} (run `copilot auth login --persist-session` once)",
                    dir.display()
                );
            }
            ensure_private_dir(&dir)?;

            let Some(helper) = token_helper_path() else {
                anyhow::bail!(
                    "token refresh helper not found (install python3 + playwright, or re-run `copilot auth set-token`)"
                );
            };

            let out = std::process::Command::new("python3")
                .arg(helper)
                .args(["--mode", "session"])
                .args(["--user-data-dir", dir.to_string_lossy().as_ref()])
                .args(["--timeout-seconds", &args.timeout_seconds.to_string()])
                .output()
                .context("failed to run token helper")?;

            if !out.status.success() {
                anyhow::bail!("token helper failed");
            }
            let token = String::from_utf8(out.stdout)?.trim().to_string();
            if token.is_empty() {
                anyhow::bail!("token helper returned empty token");
            }

            let p = cli.token_file.clone().unwrap_or_else(token_path);
            save_token(&p, &token)?;
            println!("refreshed token (saved to {})", p.display());
            Ok(())
        }
        AuthCmd::SetToken(args) => {
            if cli.dry_run {
                println!("dry-run: would prompt for token and write it to disk");
                return Ok(());
            }

            let token = if let Some(t) = cli.token.clone() {
                t
            } else {
                rpassword::prompt_password("Paste Copilot bearer token (input hidden): ")?
            };

            if token.trim().is_empty() {
                anyhow::bail!("empty token");
            }

            let p = args
                .token_file
                .or_else(|| cli.token_file.clone())
                .unwrap_or_else(token_path);
            save_token(&p, token.trim())?;
            println!("saved token to {}", p.display());
            Ok(())
        }
        AuthCmd::Logout => {
            let p = cli.token_file.clone().unwrap_or_else(token_path);
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
            println!("removed token at {}", p.display());
            Ok(())
        }
    }
}
