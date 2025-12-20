use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use copilot_money_cli::client::{ClientMode, CopilotClient};

fn serve_one(status: u16, body: &'static str, assert_bearer: Option<&'static str>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();

        let mut buf = Vec::new();
        let mut header_end = None;
        while header_end.is_none() {
            let mut tmp = [0u8; 1024];
            let n = stream.read(&mut tmp).unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = Some(i + 4);
            }
        }

        let header_end = header_end.expect("did not receive full headers");
        let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let lower = headers.to_lowercase();
        assert!(lower.starts_with("post /api/graphql"));
        if let Some(t) = assert_bearer {
            assert!(lower.contains(&format!("authorization: bearer {t}")));
        }

        let content_length = lower
            .lines()
            .find_map(|l| l.strip_prefix("content-length: "))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);

        let mut body_buf = buf[header_end..].to_vec();
        while body_buf.len() < content_length {
            let mut tmp = vec![0u8; content_length - body_buf.len()];
            let n = stream.read(&mut tmp).unwrap();
            if n == 0 {
                break;
            }
            body_buf.extend_from_slice(&tmp[..n]);
        }
        let req_body = String::from_utf8_lossy(&body_buf[..content_length]).to_string();
        assert!(req_body.contains("\"operationName\":\"User\""));

        let resp = format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(resp.as_bytes()).unwrap();
    });

    format!("http://{}", addr)
}

fn serve_two(
    first_status: u16,
    first_body: &'static str,
    first_assert_bearer: Option<&'static str>,
    second_status: u16,
    second_body: &'static str,
    second_assert_bearer: Option<&'static str>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        for (status, body, assert_bearer) in [
            (first_status, first_body, first_assert_bearer),
            (second_status, second_body, second_assert_bearer),
        ] {
            let (mut stream, _) = listener.accept().unwrap();

            let mut buf = Vec::new();
            let mut header_end = None;
            while header_end.is_none() {
                let mut tmp = [0u8; 1024];
                let n = stream.read(&mut tmp).unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(i + 4);
                }
            }

            let header_end = header_end.expect("did not receive full headers");
            let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let lower = headers.to_lowercase();
            assert!(lower.starts_with("post /api/graphql"));
            if let Some(t) = assert_bearer {
                assert!(lower.contains(&format!("authorization: bearer {t}")));
            }

            let content_length = lower
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);

            let mut body_buf = buf[header_end..].to_vec();
            while body_buf.len() < content_length {
                let mut tmp = vec![0u8; content_length - body_buf.len()];
                let n = stream.read(&mut tmp).unwrap();
                if n == 0 {
                    break;
                }
                body_buf.extend_from_slice(&tmp[..n]);
            }
            let req_body = String::from_utf8_lossy(&body_buf[..content_length]).to_string();
            assert!(req_body.contains("\"operationName\":\"User\""));

            let resp = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).unwrap();
        }
    });

    format!("http://{}", addr)
}

#[test]
fn http_mode_sends_bearer_and_accepts_success() {
    let base_url = serve_one(200, r#"{"data":{"user":{"id":"u1"}}}"#, Some("abc"));
    let tmp = tempfile::tempdir().unwrap();
    let client = CopilotClient::new(ClientMode::Http {
        base_url,
        token: Some("abc".to_string()),
        token_file: tmp.path().join("token"),
        session_dir: None,
    });
    client.try_user_query().unwrap();
}

#[test]
fn http_mode_errors_on_graphql_errors_key() {
    let base_url = serve_one(200, r#"{"errors":[{"message":"nope"}]}"#, None);
    let tmp = tempfile::tempdir().unwrap();
    let client = CopilotClient::new(ClientMode::Http {
        base_url,
        token: None,
        token_file: tmp.path().join("token"),
        session_dir: None,
    });
    assert!(client.try_user_query().is_err());
}

#[test]
fn http_mode_formats_graphql_error_with_code() {
    let base_url = serve_one(
        400,
        r#"{"errors":[{"extensions":{"code":"BAD_USER_INPUT"},"message":"Value does not exist"}]}"#,
        None,
    );
    let tmp = tempfile::tempdir().unwrap();
    let client = CopilotClient::new(ClientMode::Http {
        base_url,
        token: None,
        token_file: tmp.path().join("token"),
        session_dir: None,
    });

    let err = client.try_user_query().unwrap_err().to_string();
    assert!(err.contains("graphql error (BAD_USER_INPUT): Value does not exist"));
}

#[test]
fn http_mode_errors_on_http_status() {
    let base_url = serve_one(401, r#"{"data":null}"#, None);
    let tmp = tempfile::tempdir().unwrap();
    let client = CopilotClient::new(ClientMode::Http {
        base_url,
        token: None,
        token_file: tmp.path().join("token"),
        session_dir: None,
    });
    assert!(client.try_user_query().is_err());
}

#[test]
fn http_mode_refreshes_token_on_unauthenticated_and_retries_once() {
    // NOTE: In Rust 2024 edition, mutating process env is `unsafe` due to potential UB with
    // concurrent access. This test runs single-threaded with a narrowly-scoped env var used
    // only by the refresh hook.
    unsafe { std::env::set_var("COPILOT_TEST_REFRESH_TOKEN", "refreshed_token") };

    let base_url = serve_two(
        401,
        r#"{"errors":[{"extensions":{"code":"UNAUTHENTICATED"},"message":"User is not authenticated"}]}"#,
        Some("expired_token"),
        200,
        r#"{"data":{"user":{"id":"u1"}}}"#,
        Some("refreshed_token"),
    );

    let tmp = tempfile::tempdir().unwrap();
    let session_dir = tmp.path().join("session");
    std::fs::create_dir_all(&session_dir).unwrap();
    let token_file = tmp.path().join("token");

    let client = CopilotClient::new(ClientMode::Http {
        base_url,
        token: Some("expired_token".to_string()),
        token_file: token_file.clone(),
        session_dir: Some(session_dir),
    });

    client.try_user_query().unwrap();

    let saved = std::fs::read_to_string(&token_file).unwrap();
    assert_eq!(saved.trim(), "refreshed_token");

    unsafe { std::env::remove_var("COPILOT_TEST_REFRESH_TOKEN") };
}
