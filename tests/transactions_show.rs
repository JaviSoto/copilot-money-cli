use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serde_json::{Value, json};

fn transaction_page(ids: Vec<String>, end_cursor: &str, has_next_page: bool) -> String {
    let edges = ids
        .into_iter()
        .map(|id| {
            json!({
                "cursor": id,
                "node": {
                    "id": id,
                    "date": "2025-01-02",
                    "name": "Fixture transaction",
                    "amount": "-12.34"
                }
            })
        })
        .collect::<Vec<_>>();

    json!({
        "data": {
            "transactions": {
                "edges": edges,
                "pageInfo": {
                    "endCursor": end_cursor,
                    "hasNextPage": has_next_page
                }
            }
        }
    })
    .to_string()
}

fn read_request(stream: &mut std::net::TcpStream) -> Value {
    let mut buf = Vec::new();
    let mut header_end = None;
    while header_end.is_none() {
        let mut chunk = [0u8; 1024];
        let n = stream.read(&mut chunk).unwrap();
        assert_ne!(n, 0, "request ended before headers");
        buf.extend_from_slice(&chunk[..n]);
        header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
    }

    let header_end = header_end.unwrap();
    let headers = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .and_then(|length| length.trim().parse::<usize>().ok())
        .unwrap();
    while buf.len() - header_end < content_length {
        let mut chunk = vec![0u8; content_length - (buf.len() - header_end)];
        let n = stream.read(&mut chunk).unwrap();
        assert_ne!(n, 0, "request ended before body");
        buf.extend_from_slice(&chunk[..n]);
    }

    serde_json::from_slice(&buf[header_end..header_end + content_length]).unwrap()
}

#[test]
fn transactions_show_finds_an_id_past_the_first_page() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let first_page = transaction_page(
        (0..200).map(|index| format!("older_{index}")).collect(),
        "cursor_200",
        true,
    );
    let second_page = transaction_page(
        vec!["target_2025_transaction".into()],
        "cursor_target",
        false,
    );

    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut after_values = Vec::new();
        while after_values.len() < 2 && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let request = read_request(&mut stream);
                    assert_eq!(request["operationName"], "Transactions");
                    after_values.push(request["variables"]["after"].clone());
                    let body = if after_values.len() == 1 {
                        &first_page
                    } else {
                        &second_page
                    };
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("mock server accept failed: {error}"),
            }
        }
        after_values
    });

    let tmp_home = tempfile::tempdir().unwrap();
    let output = Command::new(assert_cmd::cargo::cargo_bin!("copilot"))
        .env("HOME", tmp_home.path())
        .env_remove("COPILOT_TOKEN")
        .env_remove("COPILOT_TOKEN_FILE")
        .args([
            "--base-url",
            &base_url,
            "--color",
            "never",
            "transactions",
            "show",
            "target_2025_transaction",
        ])
        .output()
        .unwrap();
    let after_values = server.join().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(after_values, vec![Value::Null, json!("cursor_200")]);
    assert!(String::from_utf8_lossy(&output.stdout).contains("target_2025_transaction"));
}
