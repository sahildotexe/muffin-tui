use std::{
    fs, io,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use qrcodegen::{QrCode, QrCodeEcc};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::codex::io_other;

const REMOTE_POLL_MS: u64 = 750;
const TUNNEL_START_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RemoteAction {
    Enter,
    ApproveYes,
    RejectNo,
    Interrupt,
}

#[derive(Clone, Debug, Serialize)]
struct RemoteSnapshot {
    mode_title: String,
    status: String,
    lines: Vec<String>,
}

impl Default for RemoteSnapshot {
    fn default() -> Self {
        Self {
            mode_title: "Shell".to_string(),
            status: "Remote stream waiting for a live session.".to_string(),
            lines: vec!["Waiting for session output...".to_string()],
        }
    }
}

struct SharedState {
    snapshot: RemoteSnapshot,
    actions: Vec<RemoteAction>,
}

pub struct RemoteShare {
    shared: Arc<Mutex<SharedState>>,
    running: Arc<AtomicBool>,
    tunnel: Child,
    url: String,
    qr_lines: Vec<String>,
    qr_svg_path: PathBuf,
}

impl RemoteShare {
    pub fn start() -> io::Result<Self> {
        let token = random_token(24);
        let qr_token = token.clone();

        let listener = TcpListener::bind(("0.0.0.0", 0))?;
        let port = listener.local_addr()?.port();
        let server = Server::from_listener(listener, None).map_err(io_other)?;
        let shared = Arc::new(Mutex::new(SharedState {
            snapshot: RemoteSnapshot::default(),
            actions: Vec::new(),
        }));
        let running = Arc::new(AtomicBool::new(true));

        spawn_server_thread(server, Arc::clone(&shared), Arc::clone(&running), token);
        wait_for_local_server(port, TUNNEL_START_TIMEOUT)?;

        let (tunnel, url) = start_tunnel(port)?;
        let qr_lines = render_qr_lines(&url)?;
        let qr_svg_path = write_qr_svg(&url, &qr_token)?;

        Ok(Self {
            shared,
            running,
            tunnel,
            url,
            qr_lines,
            qr_svg_path,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn qr_lines(&self) -> &[String] {
        &self.qr_lines
    }

    pub fn qr_svg_path(&self) -> &PathBuf {
        &self.qr_svg_path
    }
    pub fn update_snapshot(
        &self,
        mode_title: impl Into<String>,
        status: impl Into<String>,
        lines: Vec<String>,
    ) {
        if let Ok(mut shared) = self.shared.lock() {
            shared.snapshot = RemoteSnapshot {
                mode_title: mode_title.into(),
                status: status.into(),
                lines,
            };
        }
    }

    pub fn drain_actions(&self) -> Vec<RemoteAction> {
        if let Ok(mut shared) = self.shared.lock() {
            return shared.actions.drain(..).collect();
        }
        Vec::new()
    }
}

impl Drop for RemoteShare {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.tunnel.kill();
        let _ = self.tunnel.wait();
    }
}

fn spawn_server_thread(
    server: Server,
    shared: Arc<Mutex<SharedState>>,
    running: Arc<AtomicBool>,
    token: String,
) {
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(200)) else {
                continue;
            };

            let method = request.method().clone();
            let url = request.url().to_string();

            let response =
                if method == Method::Get && (url == "/" || url == format!("/pair/{token}")) {
                    html_response(render_mobile_page(&token))
                } else if method == Method::Get && url == format!("/snapshot/{token}") {
                    let body = if let Ok(shared) = shared.lock() {
                        serde_json::to_string(&shared.snapshot)
                            .unwrap_or_else(|_| "{\"status\":\"snapshot unavailable\"}".to_string())
                    } else {
                        "{\"status\":\"snapshot unavailable\"}".to_string()
                    };
                    json_response(body)
                } else if method == Method::Post && url.starts_with(&format!("/action/{token}/")) {
                    let action_name = url
                        .trim_start_matches(&format!("/action/{token}/"))
                        .trim_matches('/');
                    if let Some(action) = parse_action(action_name) {
                        if let Ok(mut shared) = shared.lock() {
                            shared.actions.push(action);
                        }
                        text_response("ok")
                    } else {
                        status_response(StatusCode(404), "unknown action")
                    }
                } else {
                    status_response(StatusCode(404), "not found")
                };

            let _ = request.respond(response);
        }
    });
}

fn parse_action(value: &str) -> Option<RemoteAction> {
    match value {
        "enter" => Some(RemoteAction::Enter),
        "yes" => Some(RemoteAction::ApproveYes),
        "no" => Some(RemoteAction::RejectNo),
        "interrupt" => Some(RemoteAction::Interrupt),
        _ => None,
    }
}

fn start_tunnel(port: u16) -> io::Result<(Child, String)> {
    let mut tunnel = spawn_ngrok_tunnel(port)?;
    match wait_for_ngrok_url(&mut tunnel, port, TUNNEL_START_TIMEOUT) {
        Ok(url) => Ok((tunnel, url)),
        Err(err) => {
            let _ = tunnel.kill();
            let _ = tunnel.wait();
            Err(err)
        }
    }
}

fn wait_for_local_server(port: u16, timeout: Duration) -> io::Result<()> {
    use std::io::{Read, Write};

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(mut stream) => {
                stream.write_all(
                    concat!(
                        "GET / HTTP/1.1\r\n",
                        "Host: 127.0.0.1\r\n",
                        "Connection: close\r\n",
                        "\r\n"
                    )
                    .as_bytes(),
                )?;
                stream.flush()?;

                let mut response = String::new();
                stream.read_to_string(&mut response)?;
                if response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200") {
                    return Ok(());
                }
            }
            Err(_) => {
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Err(io::Error::other(
        "local remote-share server did not start listening in time",
    ))
}

fn spawn_ngrok_tunnel(port: u16) -> io::Result<Child> {
    Command::new("ngrok")
        .args([
            "http",
            &format!("127.0.0.1:{port}"),
            "--log",
            "stdout",
            "--log-format",
            "logfmt",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

fn wait_for_ngrok_url(child: &mut Child, port: u16, timeout: Duration) -> io::Result<String> {
    let (receiver, _threads) = spawn_tunnel_log_threads(child);
    let deadline = Instant::now() + timeout;
    let mut recent_logs = Vec::new();

    while Instant::now() < deadline {
        if let Ok(url) = fetch_ngrok_public_url(port) {
            return Ok(url);
        }

        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "ngrok exited before publishing a URL (status {status}); {}",
                summarize_recent_logs(&recent_logs)
            )));
        }

        while let Ok(line) = receiver.try_recv() {
            push_recent_log_line(&mut recent_logs, line);
        }

        thread::sleep(Duration::from_millis(250));
    }

    while let Ok(line) = receiver.try_recv() {
        push_recent_log_line(&mut recent_logs, line);
    }

    Err(io::Error::other(format!(
        "ngrok did not report a public URL; make sure ngrok is installed, authenticated, and able to start a tunnel ({})",
        summarize_recent_logs(&recent_logs)
    )))
}

fn spawn_tunnel_log_threads(
    child: &mut Child,
) -> (mpsc::Receiver<String>, Vec<thread::JoinHandle<()>>) {
    let (sender, receiver) = mpsc::channel();
    let mut threads = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        let sender = sender.clone();
        threads.push(thread::spawn(move || read_tunnel_stream(stdout, sender)));
    }

    if let Some(stderr) = child.stderr.take() {
        let sender = sender.clone();
        threads.push(thread::spawn(move || read_tunnel_stream(stderr, sender)));
    }

    drop(sender);
    (receiver, threads)
}

fn read_tunnel_stream<T: io::Read + Send + 'static>(mut stream: T, sender: mpsc::Sender<String>) {
    let mut buf = [0u8; 2048];
    let mut pending = String::new();

    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                pending.push_str(&String::from_utf8_lossy(&buf[..n]));
                while let Some(idx) = pending.find('\n') {
                    let line = pending.drain(..=idx).collect::<String>();
                    let _ = sender.send(line);
                }
            }
            Err(_) => break,
        }
    }

    if !pending.trim().is_empty() {
        let _ = sender.send(pending);
    }
}

fn fetch_ngrok_public_url(port: u16) -> io::Result<String> {
    use std::io::{Read, Write};

    let mut stream = TcpStream::connect(("127.0.0.1", 4040))?;
    let request = concat!(
        "GET /api/tunnels HTTP/1.1\r\n",
        "Host: 127.0.0.1:4040\r\n",
        "Connection: close\r\n",
        "\r\n"
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| io::Error::other("ngrok API returned an invalid HTTP response"))?;

    extract_ngrok_url_from_api(body, port)
        .ok_or_else(|| io::Error::other("ngrok API has no active tunnel for this local port"))
}

fn extract_ngrok_url_from_api(body: &str, port: u16) -> Option<String> {
    let response: NgrokTunnelList = serde_json::from_str(body).ok()?;
    let expected_addrs = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("http://127.0.0.1:{port}"),
        format!("http://localhost:{port}"),
    ];

    response
        .tunnels
        .into_iter()
        .filter(|tunnel| tunnel.public_url.starts_with("https://"))
        .find(|tunnel| {
            tunnel
                .config
                .as_ref()
                .map(|config| {
                    expected_addrs
                        .iter()
                        .any(|expected| config.addr == *expected)
                })
                .unwrap_or(false)
                || tunnel
                    .forwards_to
                    .as_ref()
                    .map(|addr| expected_addrs.iter().any(|expected| addr == expected))
                    .unwrap_or(false)
        })
        .map(|tunnel| tunnel.public_url.trim_end_matches('/').to_string())
}

fn push_recent_log_line(lines: &mut Vec<String>, line: String) {
    if lines.len() >= 6 {
        lines.remove(0);
    }
    lines.push(line.trim().to_string());
}

fn summarize_recent_logs(lines: &[String]) -> String {
    if lines.is_empty() {
        "no tunnel logs captured".to_string()
    } else {
        lines.join(" | ")
    }
}

#[derive(Debug, Deserialize)]
struct NgrokTunnelList {
    tunnels: Vec<NgrokTunnel>,
}

#[derive(Debug, Deserialize)]
struct NgrokTunnel {
    public_url: String,
    #[serde(default)]
    config: Option<NgrokTunnelConfig>,
    #[serde(default)]
    forwards_to: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NgrokTunnelConfig {
    addr: String,
}

fn random_token(len: usize) -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .map(char::from)
        .take(len)
        .collect()
}

fn render_qr_lines(url: &str) -> io::Result<Vec<String>> {
    let qr = QrCode::encode_text(url, QrCodeEcc::Medium)
        .map_err(|err| io::Error::other(format!("failed to encode QR: {err:?}")))?;
    let border = 2;
    let size = qr.size();
    let mut lines = Vec::new();

    for y in ((-border)..(size + border)).step_by(2) {
        let mut line = String::new();
        for x in (-border)..(size + border) {
            let top = qr.get_module(x, y);
            let bottom = qr.get_module(x, y + 1);
            line.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        lines.push(line);
    }

    Ok(lines)
}

fn write_qr_svg(url: &str, token: &str) -> io::Result<PathBuf> {
    let qr = QrCode::encode_text(url, QrCodeEcc::Medium)
        .map_err(|err| io::Error::other(format!("failed to encode QR: {err:?}")))?;
    let border = 4;
    let size = qr.size() + (border * 2);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" shape-rendering="crispEdges">"#
    );
    svg.push_str(r##"<rect width="100%" height="100%" fill="#ffffff"/>"##);
    svg.push_str(r#"<path d=""#);

    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                let x = x + border;
                let y = y + border;
                svg.push_str(&format!("M{x},{y}h1v1h-1z"));
            }
        }
    }

    svg.push_str(r##"" fill="#000000"/></svg>"##);

    let path = std::env::temp_dir().join(format!("muffin-remote-{token}.svg"));
    fs::write(&path, svg)?;
    Ok(path)
}

fn render_mobile_page(token: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Muffin Remote</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #0b0f14;
      --panel: #111823;
      --panel-border: #243244;
      --text: #e6eef7;
      --muted: #8ea2b8;
      --accent: #64d7c8;
      --danger: #f85149;
      --success: #2ea043;
    }}
    body {{
      margin: 0;
      background: radial-gradient(circle at top, #132235, var(--bg) 60%);
      color: var(--text);
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    }}
    main {{
      max-width: 900px;
      margin: 0 auto;
      padding: 20px 16px 28px;
    }}
    .status {{
      color: var(--muted);
      margin-bottom: 12px;
      font-size: 14px;
    }}
    .controls {{
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
      margin: 16px 0;
    }}
    button {{
      border: 1px solid var(--panel-border);
      background: var(--panel);
      color: var(--text);
      padding: 14px 10px;
      border-radius: 12px;
      font-size: 16px;
      font-weight: 600;
    }}
    button.primary {{ border-color: var(--accent); }}
    button.success {{ border-color: var(--success); }}
    button.danger {{ border-color: var(--danger); }}
    pre {{
      white-space: pre-wrap;
      word-break: break-word;
      background: rgba(8, 12, 18, 0.95);
      border: 1px solid var(--panel-border);
      border-radius: 16px;
      padding: 14px;
      min-height: 55vh;
      overflow: auto;
      line-height: 1.35;
      font-size: 13px;
    }}
    h1 {{
      margin: 0 0 8px;
      font-size: 20px;
    }}
  </style>
</head>
<body>
  <main>
    <h1 id="title">Muffin Remote</h1>
    <div id="status" class="status">Connecting...</div>
    <div class="controls">
      <button class="primary" onclick="sendAction('enter')">Approve / Enter</button>
      <button class="success" onclick="sendAction('yes')">Send y</button>
      <button onclick="sendAction('no')">Send n</button>
      <button class="danger" onclick="sendAction('interrupt')">Ctrl+C</button>
    </div>
    <pre id="screen">Waiting for session output...</pre>
  </main>
  <script>
    const token = {token:?};
    const screen = document.getElementById('screen');
    const statusEl = document.getElementById('status');
    const titleEl = document.getElementById('title');

    async function refresh() {{
      try {{
        const response = await fetch(`/snapshot/${{token}}`, {{ cache: 'no-store' }});
        const snapshot = await response.json();
        titleEl.textContent = `${{snapshot.mode_title}} Remote`;
        statusEl.textContent = snapshot.status;
        screen.textContent = (snapshot.lines || []).join('\n');
      }} catch (error) {{
        statusEl.textContent = `Remote stream unavailable: ${{error}}`;
      }}
    }}

    async function sendAction(action) {{
      await fetch(`/action/${{token}}/${{action}}`, {{
        method: 'POST',
        cache: 'no-store',
      }});
      refresh();
    }}

    refresh();
    setInterval(refresh, {REMOTE_POLL_MS});
  </script>
</body>
</html>"#
    )
}

fn html_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    response_with_type(body, "text/html; charset=utf-8")
}

fn json_response(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    response_with_type(body, "application/json; charset=utf-8")
}

fn text_response(body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    response_with_type(body.to_string(), "text/plain; charset=utf-8")
}

fn status_response(status: StatusCode, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    response_with_type(body.to_string(), "text/plain; charset=utf-8").with_status_code(status)
}

fn response_with_type(body: String, content_type: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap())
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteAction, extract_ngrok_url_from_api, parse_action, render_mobile_page,
        render_qr_lines, summarize_recent_logs, write_qr_svg,
    };

    #[test]
    fn action_routes_map_to_expected_commands() {
        assert_eq!(parse_action("enter"), Some(RemoteAction::Enter));
        assert_eq!(parse_action("yes"), Some(RemoteAction::ApproveYes));
        assert_eq!(parse_action("no"), Some(RemoteAction::RejectNo));
        assert_eq!(parse_action("interrupt"), Some(RemoteAction::Interrupt));
        assert_eq!(parse_action("nope"), None);
    }

    #[test]
    fn qr_renderer_returns_multiple_rows() {
        let qr = render_qr_lines("http://100.64.0.1:9999/pair/token").unwrap();
        assert!(qr.len() > 5);
        assert!(qr.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn mobile_page_embeds_snapshot_and_action_routes() {
        let html = render_mobile_page("abc123");
        assert!(html.contains("/snapshot/${token}"));
        assert!(html.contains("sendAction('enter')"));
        assert!(html.contains("Approve / Enter"));
    }

    #[test]
    fn extracts_ngrok_tunnel_url_for_matching_port() {
        let body = r#"{
            "tunnels": [
                {
                    "public_url": "http://abc.ngrok-free.app",
                    "config": {
                        "addr": "127.0.0.1:1234"
                    }
                },
                {
                    "public_url": "https://abc.ngrok-free.app",
                    "config": {
                        "addr": "127.0.0.1:1234"
                    }
                }
            ]
        }"#;

        assert_eq!(
            extract_ngrok_url_from_api(body, 1234).as_deref(),
            Some("https://abc.ngrok-free.app")
        );
    }

    #[test]
    fn summarizes_recent_logs_when_present() {
        let logs = vec!["one".to_string(), "two".to_string()];
        assert_eq!(summarize_recent_logs(&logs), "one | two");
    }

    #[test]
    fn qr_svg_writer_persists_a_real_image_file() {
        let path = write_qr_svg("https://abc.ngrok-free.app", "test-token").unwrap();
        let svg = std::fs::read_to_string(&path).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("fill=\"#000000\""));
        let _ = std::fs::remove_file(path);
    }
}
