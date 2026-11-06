//! Live link to the NetSketch Survey phone app over the home network.
//!
//! The phone finds this computer by itself (a broadcast "NETSKETCH?" on UDP 47879, answered with the
//! port and plan name), then — with the 4-digit code shown here — downloads the floor plan
//! (`GET /plan`) and sends each measurement as soon as it's taken (`POST /points`, a survey file).
//! Nothing runs until someone presses "Link a phone"; it stops when they press Stop or close the plan.
use crate::net::{Network, PX_PER_M};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const TCP_PORT: u16 = 47878;
pub const UDP_PORT: u16 = 47879;
/// Phones talking at once; more are turned away so nobody on the network can pile up threads.
const MAX_CONNECTIONS: usize = 4;
/// A whole request must arrive in this time, so a trickling sender can't hold a connection open.
const DEADLINE: Duration = Duration::from_secs(15);

pub struct PhoneLink {
    pub code: String,
    /// This computer's address on the home network and the port, for typing in by hand.
    pub addr: String,
    pub port: u16,
    plan: Arc<Mutex<String>>,
    title: Arc<Mutex<String>>,
    inbox: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    /// When a phone last talked to us (seconds since start), for the status line.
    pub last_seen: Arc<Mutex<Option<Instant>>>,
}

impl PhoneLink {
    /// Starts listening (ports 0 = any free port, for tests).
    pub fn start(tcp: u16, udp: u16, code: String, title: String) -> Result<Self, String> {
        let listener = TcpListener::bind(("0.0.0.0", tcp)).map_err(|e| format!("can't listen on port {tcp}: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let plan = Arc::new(Mutex::new(String::from("{}")));
        let inbox = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let last_seen = Arc::new(Mutex::new(None));
        let title = Arc::new(Mutex::new(title));
        {
            let (plan, inbox, stop, code, seen) = (plan.clone(), inbox.clone(), stop.clone(), code.clone(), last_seen.clone());
            let busy = Arc::new(AtomicUsize::new(0));
            let guard = Arc::new(Mutex::new(Guard::default()));
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut s, _)) => {
                            if busy.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                                busy.fetch_sub(1, Ordering::SeqCst);
                                reply(&mut s, "503 Busy", "{\"error\":\"busy\"}");
                                continue;
                            }
                            let (plan, inbox, code, seen, busy, guard) = (plan.clone(), inbox.clone(), code.clone(), seen.clone(), busy.clone(), guard.clone());
                            std::thread::spawn(move || {
                                handle(s, &code, &plan, &inbox, &seen, &guard);
                                busy.fetch_sub(1, Ordering::SeqCst);
                            });
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(80)),
                    }
                }
            });
        }
        // Discovery: answer "NETSKETCH?" with our port and the plan's name.
        if let Ok(sock) = UdpSocket::bind(("0.0.0.0", udp)) {
            let _ = sock.set_read_timeout(Some(Duration::from_millis(300)));
            let (stop, title) = (stop.clone(), title.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 64];
                while !stop.load(Ordering::Relaxed) {
                    if let Ok((n, from)) = sock.recv_from(&mut buf) {
                        if buf[..n].starts_with(b"NETSKETCH?") {
                            let title = title.lock().unwrap().clone();
                            let _ = sock.send_to(format!("NETSKETCH {port} {title}").as_bytes(), from);
                        }
                    }
                }
            });
        }
        Ok(PhoneLink { code, addr: local_ip(), port, plan, title, inbox, stop, last_seen })
    }

    /// What the phone gets as the floor plan, and the plan's name it sees when looking for computers.
    pub fn set_plan(&self, json: String, title: &str) {
        *self.plan.lock().unwrap() = json;
        title.clone_into(&mut self.title.lock().unwrap());
    }

    /// Survey files the phone sent since the last call.
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.inbox.lock().unwrap())
    }
}

impl Drop for PhoneLink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The address other devices on the home network reach this computer at (no packet is sent).
fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0").and_then(|s| s.connect("192.0.2.1:9").map(|_| s)).and_then(|s| s.local_addr()).map(|a| a.ip().to_string()).unwrap_or_else(|_| "this computer".into())
}

/// Wrong codes so far and when guessing may go on: 10,000 codes are too few to allow unlimited tries.
#[derive(Default)]
struct Guard {
    wrong: u32,
    until: Option<Instant>,
}

impl Guard {
    /// After 5 wrong codes each further one locks the link for twice as long (2 s, 4 s, … up to 5 minutes).
    fn wrong_code(&mut self) {
        self.wrong += 1;
        if self.wrong >= 5 {
            self.until = Some(Instant::now() + Duration::from_secs((1u64 << (self.wrong - 4).min(9)).min(300)));
        }
    }
    fn locked(&self) -> bool {
        self.until.is_some_and(|t| Instant::now() < t)
    }
}

/// Reads within the request's overall deadline.
fn read_by(s: &mut TcpStream, chunk: &mut [u8], end: Instant) -> Option<usize> {
    let left = end.checked_duration_since(Instant::now()).filter(|d| !d.is_zero())?;
    s.set_read_timeout(Some(left)).ok()?;
    s.read(chunk).ok()
}

/// One HTTP request; a phone with the right code counts as "seen". The code is checked before any body is read.
fn handle(mut s: TcpStream, code: &str, plan: &Mutex<String>, inbox: &Mutex<Vec<String>>, seen: &Mutex<Option<Instant>>, guard: &Mutex<Guard>) {
    let _ = s.set_nonblocking(false);
    let end = Instant::now() + DEADLINE;
    let mut buf = vec![];
    let mut chunk = [0u8; 8192];
    // Read the head, then as much body as Content-Length says (at most 5 MB).
    let (head_end, len) = loop {
        let Some(n) = read_by(&mut s, &mut chunk, end) else { return };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..i]).to_string();
            let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            break (i + 4, len);
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let given = query.split('&').find_map(|kv| kv.strip_prefix("code=")).unwrap_or("");
    {
        let mut g = guard.lock().unwrap();
        if g.locked() {
            reply(&mut s, "429 Too Many Requests", "{\"error\":\"too many wrong codes, wait a moment\"}");
            return;
        }
        if given != code {
            g.wrong_code();
            reply(&mut s, "403 Forbidden", "{\"error\":\"wrong code\"}");
            return;
        }
        g.wrong = 0;
    }
    if path == "/speed" {
        *seen.lock().unwrap() = Some(Instant::now());
        speed(s, method, query, &buf[head_end..], len, end);
        return;
    }
    if len > 5 * 1024 * 1024 {
        reply(&mut s, "413 Too Large", "{\"error\":\"too large\"}");
        return;
    }
    while buf.len() < head_end + len {
        let Some(n) = read_by(&mut s, &mut chunk, end) else { return };
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    *seen.lock().unwrap() = Some(Instant::now());
    match (method, path) {
        ("GET", "/plan") => {
            let p = plan.lock().unwrap().clone();
            reply(&mut s, "200 OK", &p);
        }
        ("POST", "/points") => {
            let body = String::from_utf8_lossy(&buf[head_end..(head_end + len).min(buf.len())]).to_string();
            inbox.lock().unwrap().push(body);
            reply(&mut s, "200 OK", "{\"ok\":true}");
        }
        _ => reply(&mut s, "404 Not Found", "{\"error\":\"unknown\"}"),
    }
}

/// The phone's Wi-Fi speed test to this computer: GET sends `mb` megabytes of zeros (at most 200; the phone stops
/// after 5 s), POST takes whatever the phone sends (chunked, until its last chunk) and answers once it's all in.
fn speed(mut s: TcpStream, method: &str, query: &str, got: &[u8], len: usize, end: Instant) {
    if method == "GET" {
        let mb = query.split('&').find_map(|kv| kv.strip_prefix("mb=")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(50).min(200);
        let total = mb * 1024 * 1024;
        if write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n").is_err() {
            return;
        }
        let zeros = [0u8; 64 * 1024];
        let mut sent = 0;
        while sent < total && Instant::now() < end {
            let n = zeros.len().min(total - sent);
            if s.write_all(&zeros[..n]).is_err() {
                return; // the phone has seen enough
            }
            sent += n;
        }
        return;
    }
    // Chunked upload of zeros: it ends with "\r\n0\r\n\r\n", which zeros and chunk sizes can't make by accident.
    let mut tail: Vec<u8> = got.iter().rev().take(8).rev().copied().collect();
    let mut chunk = vec![0u8; 64 * 1024];
    let mut read = got.len();
    // With a Content-Length instead, until that much is in.
    while if len > 0 { read < len } else { !tail.ends_with(b"0\r\n\r\n") } {
        let Some(n) = read_by(&mut s, &mut chunk, end) else { return };
        if n == 0 {
            break;
        }
        read += n;
        tail.extend_from_slice(&chunk[n.saturating_sub(8)..n]);
        let cut = tail.len().saturating_sub(8);
        tail.drain(..cut);
    }
    reply(&mut s, "200 OK", "{\"ok\":true}");
}

fn reply(s: &mut TcpStream, status: &str, body: &str) {
    let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

impl Network {
    /// The floor plan the phone shows (positions in metres): storeys, walls, room labels, devices and pins.
    pub fn plan_pack(&self) -> String {
        let m = |p: [f32; 2]| [p[0] / PX_PER_M, p[1] / PX_PER_M];
        let floors = self.used_floors();
        serde_json::json!({
            "format": "netsketch-plan",
            "title": self.title,
            "storeys": floors.iter().map(|f| serde_json::json!({"floor": f, "name": self.storey(*f)})).collect::<Vec<_>>(),
            "walls": self.walls.iter().map(|w| serde_json::json!({"floor": w.floor, "a": m(w.a), "b": m(w.b), "kind": format!("{:?}", w.kind)})).collect::<Vec<_>>(),
            "labels": self.labels.iter().map(|l| serde_json::json!({"floor": l.floor, "at": m(l.pos), "text": l.text})).collect::<Vec<_>>(),
            "devices": self.devices.iter().filter(|d| !d.kind.foreign()).map(|d| serde_json::json!({"floor": d.floor, "at": m(d.pos), "name": d.name, "kind": format!("{:?}", d.kind), "radio": d.kind.radio()})).collect::<Vec<_>>(),
            "networks": self.devices.iter().filter(|d| d.kind.radio() && !d.kind.foreign()).flat_map(Network::names_of).map(|(n, _)| n).filter(|n| !n.is_empty()).collect::<std::collections::BTreeSet<_>>(),
            "points": self.points.iter().filter(|p| p.placed).map(|p| serde_json::json!({"floor": p.floor, "at": m(p.pos), "name": p.name})).collect::<Vec<_>>(),
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http(port: u16, req: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(req.as_bytes()).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn a_phone_finds_the_plan_and_sends_points_with_the_right_code() {
        let n = crate::net::template("flat");
        let link = PhoneLink::start(0, 0, "4831".into(), n.title.clone()).unwrap();
        link.set_plan(n.plan_pack(), &n.title);
        let port = link.port;
        assert!(http(port, "GET /plan?code=0000 HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 403"));
        let plan = http(port, "GET /plan?code=4831 HTTP/1.1\r\n\r\n");
        assert!(plan.starts_with("HTTP/1.1 200") && plan.contains("netsketch-plan") && plan.contains("Bedroom"), "{plan}");
        let ssid = &n.devices.iter().find(|d| d.kind == crate::net::Kind::HomeRouter).unwrap().ssid;
        assert!(plan.contains(&format!("\"networks\":[\"{ssid}\"")), "the phone learns which networks are yours: {plan}");
        let body = r#"{"format":"netsketch-survey","points":[{"name":"Sofa","x":6.5,"y":3,"networks":[]}]}"#;
        let r = http(port, &format!("POST /points?code=4831 HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", body.len()));
        assert!(r.starts_with("HTTP/1.1 200"), "{r}");
        let got = link.take();
        assert_eq!(got, vec![body.to_string()]);
        let mut n = n;
        n.import_companion(&got[0]).unwrap();
        let p = n.points.last().unwrap();
        assert!(p.placed && (p.pos[0] - 6.5 * PX_PER_M).abs() < 0.01, "points the phone placed arrive in place");
        assert!(link.last_seen.lock().unwrap().is_some());
    }

    #[test]
    fn the_phone_measures_its_wifi_speed_to_this_computer() {
        let link = PhoneLink::start(0, 0, "4831".into(), "Test".into()).unwrap();
        let down = http(link.port, "GET /speed?code=4831&mb=2 HTTP/1.1\r\n\r\n");
        assert!(down.starts_with("HTTP/1.1 200") && down.len() > 2 * 1024 * 1024, "{}", down.len());
        assert!(http(link.port, "GET /speed?code=1111&mb=2 HTTP/1.1\r\n\r\n").starts_with("HTTP/1.1 403"));
        // A chunked upload is answered once its last chunk is in.
        let body: String = (0..20).map(|_| format!("400\r\n{}\r\n", "x".repeat(1024))).collect();
        let up = http(link.port, &format!("POST /speed?code=4831 HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n{body}0\r\n\r\n"));
        assert!(up.starts_with("HTTP/1.1 200"), "{up}");
    }

    #[test]
    fn guessing_the_code_locks_the_link_and_a_wrong_code_reads_no_body() {
        let link = PhoneLink::start(0, 0, "4831".into(), "Home".into()).unwrap();
        // A wrong code is refused straight away, even when the request claims a huge body that never comes.
        let r = http(link.port, "POST /points?code=0001 HTTP/1.1\r\nContent-Length: 4000000\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 403"), "{r}");
        for c in 2..6 {
            http(link.port, &format!("GET /plan?code={c:04} HTTP/1.1\r\n\r\n"));
        }
        let r = http(link.port, "GET /plan?code=4831 HTTP/1.1\r\n\r\n");
        assert!(r.starts_with("HTTP/1.1 429"), "even the right code waits out the lock: {r}");
        assert!(link.take().is_empty());
    }
}
