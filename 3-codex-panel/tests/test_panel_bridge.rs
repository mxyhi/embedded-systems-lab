#[path = "../src/panel_bridge_host.rs"]
mod panel_bridge_host;

use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use panel_bridge_host::{
    active_sessions,
    PanelTcpServer,
    SessionState,
    build_payload,
    directory_label,
    parse_client_line,
    process_is_alive,
    sanitize_label,
};

fn temp_state_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间应有效")
        .as_nanos();
    path.push(format!("codex-panel-test-{unique}"));
    fs::create_dir_all(&path).expect("临时目录应创建成功");
    path
}

#[test]
fn sanitize_label_replaces_non_ascii_and_truncates() {
    let label = sanitize_label(&format!("修复 wifi 面板 {}", "x".repeat(40)));
    assert!(label.ends_with("..."));
    assert!(label.contains('?'));
}

#[test]
fn directory_label_keeps_last_two_segments() {
    let label = directory_label("/Users/langhuam/workspace/self/embedded-systems-lab");
    assert_eq!(label, "self/embedded-systems-lab");
}

#[test]
fn build_payload_matches_line_protocol() {
    let payload = build_payload(&[
        SessionState {
            session_id: "a".to_owned(),
            label: "chat-a".to_owned(),
            active: true,
            updated_at: 10.0,
            parent_pid: None,
        },
        SessionState {
            session_id: "b".to_owned(),
            label: "chat-b".to_owned(),
            active: true,
            updated_at: 9.0,
            parent_pid: None,
        },
    ]);

    assert!(payload.contains("SNAP\n"));
    assert!(payload.contains("ACTIVE 1\n"));
    assert!(payload.contains("COUNT 2\n"));
    assert!(payload.contains("TITLE 0 chat-a\n"));
    assert!(payload.ends_with("END\n"));
}

#[test]
fn process_is_alive_matches_current_process_and_rejects_invalid_pid() {
    assert!(process_is_alive(std::process::id()));
    assert!(!process_is_alive(u32::MAX));
}

#[test]
fn active_sessions_ignores_dead_process_entries() {
    let now = 100.0;
    let sessions = vec![
        SessionState {
            session_id: "alive".to_owned(),
            label: "chat-a".to_owned(),
            active: true,
            updated_at: now,
            parent_pid: Some(std::process::id()),
        },
        SessionState {
            session_id: "dead".to_owned(),
            label: "chat-b".to_owned(),
            active: true,
            updated_at: now,
            parent_pid: Some(u32::MAX),
        },
    ];

    let live = active_sessions(&sessions, now);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].session_id, "alive");
}

#[test]
fn active_sessions_expires_legacy_entries_without_pid_quickly() {
    let now = 500.0;
    let sessions = vec![
        SessionState {
            session_id: "legacy-stale".to_owned(),
            label: "chat-a".to_owned(),
            active: true,
            updated_at: now - 180.0,
            parent_pid: None,
        },
        SessionState {
            session_id: "legacy-fresh".to_owned(),
            label: "chat-b".to_owned(),
            active: true,
            updated_at: now - 30.0,
            parent_pid: None,
        },
    ];

    let live = active_sessions(&sessions, now);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].session_id, "legacy-fresh");
}

#[test]
fn parse_client_line_supports_hello_and_ping() {
    assert_eq!(
        parse_client_line(b"HELLO esp32-panel\n"),
        Some(("HELLO".to_owned(), "esp32-panel".to_owned()))
    );
    assert_eq!(
        parse_client_line(b"PING 12345\n"),
        Some(("PING".to_owned(), "12345".to_owned()))
    );
    assert_eq!(parse_client_line(b"NOPE ???\n"), None);
}

#[test]
fn tcp_server_pushes_snapshot_to_connected_board() {
    let state_dir = temp_state_dir();
    let payload = r#"{
        "session_id":"session-1",
        "active":true,
        "updated_at":123.0,
        "cwd":"/Users/langhuam/workspace/self/embedded-systems-lab/3-codex-panel"
    }"#;
    fs::write(state_dir.join("session-1.json"), payload).expect("状态文件应写入成功");

    let mut server = PanelTcpServer::bind("127.0.0.1", 0, state_dir.clone(), 0.01)
        .expect("TCP server 应启动成功");
    let address = server.address();
    assert_eq!(server.interval(), 0.01);

    let mut client = TcpStream::connect(address).expect("客户端应连上 server");
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("读超时应设置成功");
    client
        .write_all(b"HELLO esp32-panel\n")
        .expect("客户端 hello 应发送成功");

    let deadline = SystemTime::now() + Duration::from_secs(1);
    let mut received = Vec::new();
    while SystemTime::now() < deadline {
        server.poll_once(123.0).expect("server 轮询应成功");

        let mut chunk = [0_u8; 512];
        match client.read(&mut chunk) {
            Ok(0) => {}
            Ok(size) => received.extend_from_slice(&chunk[..size]),
            Err(_) => {}
        }

        if received.windows(b"SNAP\n".len()).any(|window| window == b"SNAP\n")
            && received
                .windows(b"TITLE 0 embedded-systems-lab/3-co...\n".len())
                .any(|window| window == b"TITLE 0 embedded-systems-lab/3-co...\n")
        {
            break;
        }

        thread::sleep(Duration::from_millis(10));
    }

    let text = String::from_utf8(received).expect("输出应是 ASCII");
    assert!(text.contains("HELLO codex-panel 1\n"));
    assert!(text.contains("SNAP\n"));
    assert!(text.contains("ACTIVE 1\n"));
    assert!(text.contains("TITLE 0 embedded-systems-lab/3-co...\n"));

    fs::remove_dir_all(state_dir).ok();
}
