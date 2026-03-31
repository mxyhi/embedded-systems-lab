use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
};

use libc::{ESRCH, kill};
use serde::Deserialize;

const ACTIVE_STALE_SECONDS: f64 = 8.0 * 60.0 * 60.0;
const LEGACY_ACTIVE_STALE_SECONDS: f64 = 120.0;
const MAX_VISIBLE_TITLES: usize = 4;
const TITLE_MAX_CHARS: usize = 28;
const HEARTBEAT_SECONDS: f64 = 2.0;
const SERVER_HELLO: &[u8] = b"HELLO codex-panel 1\n";

#[derive(Clone, Debug, PartialEq)]
pub struct SessionState {
    pub session_id: String,
    pub label: String,
    pub active: bool,
    pub updated_at: f64,
    pub parent_pid: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct StoredState {
    session_id: Option<String>,
    directory_label: Option<String>,
    cwd: Option<String>,
    active: Option<bool>,
    updated_at: Option<f64>,
    parent_pid: Option<u32>,
}

#[derive(Debug)]
struct ClientConnection {
    stream: TcpStream,
    rx_buffer: Vec<u8>,
    tx_buffer: Vec<u8>,
    last_message: String,
}

impl ClientConnection {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            rx_buffer: Vec::new(),
            tx_buffer: SERVER_HELLO.to_vec(),
            last_message: String::new(),
        }
    }

    fn queue(&mut self, payload: &[u8]) {
        self.tx_buffer.extend_from_slice(payload);
    }
}

pub struct PanelTcpServer {
    listener: TcpListener,
    clients: BTreeMap<i32, ClientConnection>,
    state_dir: PathBuf,
    interval: f64,
    last_payload: String,
    last_send_at: f64,
}

impl PanelTcpServer {
    pub fn bind(bind_host: &str, port: u16, state_dir: PathBuf, interval: f64) -> io::Result<Self> {
        fs::create_dir_all(&state_dir)?;

        let listener = TcpListener::bind((bind_host, port))?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            listener,
            clients: BTreeMap::new(),
            state_dir,
            interval,
            last_payload: String::new(),
            last_send_at: 0.0,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.listener.local_addr().expect("监听地址应可读取")
    }

    pub fn interval(&self) -> f64 {
        self.interval
    }

    pub fn poll_once(&mut self, now: f64) -> io::Result<()> {
        self.accept_new_clients()?;

        let client_ids: Vec<i32> = self.clients.keys().copied().collect();
        for client_id in client_ids {
            if !self.read_client(client_id)? {
                continue;
            }
            self.flush_client(client_id)?;
        }

        let payload = build_payload(&active_sessions(&load_sessions(&self.state_dir), now));
        if payload != self.last_payload || now - self.last_send_at >= HEARTBEAT_SECONDS {
            let encoded = payload.as_bytes().to_vec();
            let client_ids: Vec<i32> = self.clients.keys().copied().collect();
            for client_id in client_ids {
                if let Some(connection) = self.clients.get_mut(&client_id) {
                    connection.queue(&encoded);
                }
                self.flush_client(client_id)?;
            }
            self.last_payload = payload;
            self.last_send_at = now;
        }

        Ok(())
    }

    fn accept_new_clients(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, _peer)) => {
                    stream.set_nonblocking(true)?;
                    self.clients
                        .insert(stream.as_raw_fd(), ClientConnection::new(stream));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error),
            }
        }
    }

    fn read_client(&mut self, client_id: i32) -> io::Result<bool> {
        let mut dropped = false;

        if let Some(connection) = self.clients.get_mut(&client_id) {
            loop {
                let mut buffer = [0_u8; 4096];
                match connection.stream.read(&mut buffer) {
                    Ok(0) => {
                        dropped = true;
                        break;
                    }
                    Ok(size) => connection.rx_buffer.extend_from_slice(&buffer[..size]),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error),
                }
            }

            while let Some(newline_at) = connection.rx_buffer.iter().position(|byte| *byte == b'\n') {
                let line = connection.rx_buffer.drain(..=newline_at).collect::<Vec<_>>();
                if let Some((command, value)) = parse_client_line(&line) {
                    connection.last_message = format!("{command} {value}").trim().to_owned();
                }
            }
        }

        if dropped {
            self.clients.remove(&client_id);
            return Ok(false);
        }

        Ok(true)
    }

    fn flush_client(&mut self, client_id: i32) -> io::Result<()> {
        let mut should_drop = false;

        if let Some(connection) = self.clients.get_mut(&client_id) {
            while !connection.tx_buffer.is_empty() {
                match connection.stream.write(&connection.tx_buffer) {
                    Ok(0) => {
                        should_drop = true;
                        break;
                    }
                    Ok(size) => {
                        connection.tx_buffer.drain(..size);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error),
                }
            }
        }

        if should_drop {
            self.clients.remove(&client_id);
        }

        Ok(())
    }
}

pub fn sanitize_label(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let ascii_only = collapsed
        .chars()
        .map(|char| if (' '..='~').contains(&char) { char } else { '?' })
        .collect::<String>();

    if ascii_only.len() <= TITLE_MAX_CHARS {
        return ascii_only;
    }

    format!("{}...", &ascii_only[..TITLE_MAX_CHARS - 3])
}

pub fn directory_label(cwd: &str) -> String {
    let path = Path::new(cwd);
    let parts = path
        .components()
        .filter_map(|component| {
            let value = component.as_os_str().to_str()?;
            if value.is_empty() || value == "/" {
                None
            } else {
                Some(value)
            }
        })
        .collect::<Vec<_>>();

    match parts.as_slice() {
        [] => "/".to_owned(),
        [single] => sanitize_label(single),
        _ => sanitize_label(&parts[parts.len() - 2..].join("/")),
    }
}

pub fn load_sessions(state_dir: &Path) -> Vec<SessionState> {
    let mut sessions = Vec::new();
    let Ok(entries) = fs::read_dir(state_dir) else {
        return sessions;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }

        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<StoredState>(&content) else {
            continue;
        };

        let cwd = payload.cwd.unwrap_or_default();
        let label_source = if cwd.is_empty() {
            payload
                .directory_label
                .unwrap_or_else(|| path.file_stem().and_then(|value| value.to_str()).unwrap_or("unknown").to_owned())
        } else {
            directory_label(&cwd)
        };

        sessions.push(SessionState {
            session_id: payload
                .session_id
                .unwrap_or_else(|| path.file_stem().and_then(|value| value.to_str()).unwrap_or("unknown").to_owned()),
            label: sanitize_label(&label_source),
            active: payload.active.unwrap_or(false),
            updated_at: payload.updated_at.unwrap_or(0.0),
            parent_pid: payload.parent_pid,
        });
    }

    sessions.sort_by(|left, right| right.updated_at.partial_cmp(&left.updated_at).unwrap_or(std::cmp::Ordering::Equal));
    sessions
}

pub fn active_sessions(sessions: &[SessionState], now: f64) -> Vec<SessionState> {
    sessions
        .iter()
        .filter(|session| {
            session.active
                && match session.parent_pid {
                    Some(pid) => {
                        now - session.updated_at <= ACTIVE_STALE_SECONDS && process_is_alive(pid)
                    }
                    None => now - session.updated_at <= LEGACY_ACTIVE_STALE_SECONDS,
                }
        })
        .take(MAX_VISIBLE_TITLES)
        .cloned()
        .collect()
}

pub fn process_is_alive(pid: u32) -> bool {
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }

    let result = unsafe { kill(pid as i32, 0) };
    result == 0 || io::Error::last_os_error().raw_os_error() != Some(ESRCH)
}

pub fn build_payload(sessions: &[SessionState]) -> String {
    let mut lines = vec![
        "SNAP".to_owned(),
        format!("ACTIVE {}", usize::from(!sessions.is_empty())),
        format!("COUNT {}", sessions.len()),
    ];
    for (index, session) in sessions.iter().enumerate() {
        lines.push(format!("TITLE {index} {}", session.label));
    }
    lines.push("END".to_owned());
    lines.join("\n") + "\n"
}

pub fn parse_client_line(line: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(line).ok()?.trim();
    if text.is_empty() {
        return None;
    }

    let (command, value) = text.split_once(' ').unwrap_or((text, ""));
    if !matches!(command, "HELLO" | "PING") {
        return None;
    }

    Some((command.to_owned(), value.trim().to_owned()))
}

use std::os::fd::AsRawFd;
