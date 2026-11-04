//! Planning extras: everyday app traffic, Power over Ethernet, firewall rules, IPv6, the guest network,
//! real measurements for calibration, floor plan pictures and a library of device models.
use crate::net::{parse_ip, same_net, Device, Kind, Medium, Network, Outcome, SITES};
use crate::wifi::Band;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::Ipv4Addr;

/// VLAN the guest Wi-Fi lives in.
pub const GUEST_VLAN: u16 = 4000;

// ---------- app traffic ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Use {
    Browsing,
    VideoCall,
    StreamHd,
    Stream4k,
    Gaming,
    Backup,
    Download,
    Camera,
    Music,
}

impl Use {
    pub fn label(self) -> &'static str {
        match self {
            Use::Browsing => "Web & apps",
            Use::VideoCall => "Video calls",
            Use::StreamHd => "HD streaming",
            Use::Stream4k => "4K streaming",
            Use::Gaming => "Online gaming",
            Use::Backup => "Cloud / NAS backup",
            Use::Download => "Big downloads (game updates)",
            Use::Camera => "Camera recording",
            Use::Music => "Music streaming",
        }
    }
    /// Typical busy-hour demand (down, up) in Mbit/s.
    pub fn demand(self) -> (f32, f32) {
        match self {
            Use::Browsing => (3.0, 0.5),
            Use::VideoCall => (4.0, 3.0),
            Use::StreamHd => (6.0, 0.2),
            Use::Stream4k => (25.0, 0.3),
            Use::Gaming => (1.0, 0.5),
            Use::Backup => (1.0, 40.0),
            Use::Download => (150.0, 2.0),
            Use::Camera => (0.1, 4.0),
            Use::Music => (0.4, 0.0),
        }
    }
    /// Suffers badly from delay and jitter.
    pub fn sensitive(self) -> bool {
        matches!(self, Use::VideoCall | Use::Gaming)
    }
    /// Goes to a storage box in the house when there is one.
    fn local(self) -> bool {
        matches!(self, Use::Backup | Use::Camera)
    }
}

pub fn default_uses(kind: Kind) -> Vec<Use> {
    match kind {
        Kind::Tv => vec![Use::Stream4k],
        Kind::Phone => vec![Use::VideoCall],
        Kind::Laptop | Kind::Desktop => vec![Use::Browsing],
        Kind::Console => vec![Use::Gaming],
        Kind::Camera => vec![Use::Camera],
        Kind::Speaker => vec![Use::Music],
        _ => vec![],
    }
}

/// Busy-hour traffic on every link.
#[derive(Default, Debug)]
pub struct Load {
    /// Per link: (towards the device, away from it) summed — kept per direction for full-duplex cables.
    pub per_link: HashMap<u32, (f32, f32)>,
    /// Who causes load on each link.
    pub who: HashMap<u32, Vec<u32>>,
}

impl Network {
    /// Where a device's app traffic goes: the internet, or a NAS/server in the house for backups and cameras.
    fn target(&self, u: Use) -> Option<Ipv4Addr> {
        if u.local() {
            if let Some(ip) = self.devices.iter().find(|d| matches!(d.kind, Kind::Nas | Kind::Server)).and_then(|d| d.first_ip()) {
                return Some(ip);
            }
        }
        self.devices.iter().any(|d| d.kind == Kind::Internet).then(|| crate::net::site_ip(0))
    }

    /// Adds up everyone's apps along their real paths.
    pub fn loads(&self) -> Load {
        let mut out = Load::default();
        for d in self.devices.iter().filter(|d| !d.uses.is_empty() && !d.off) {
            let Some(src) = d.first_ip() else { continue };
            for u in &d.uses {
                let act = crate::more::activity(*u, self.hour);
                if act < 0.01 {
                    continue;
                }
                let Some(dst) = self.target(*u) else { continue };
                let t = self.send(d.id, src, dst, "TCP", 443, None);
                if !t.ok {
                    continue;
                }
                let (down, up) = u.demand();
                let (down, up) = (down * act, up * act);
                for w in t.path.windows(2) {
                    if let Some(l) = self.link_between(w[0], w[1]) {
                        // Direction "a→b" vs "b→a" for full-duplex cables.
                        let fwd = l.a == w[0];
                        let e = out.per_link.entry(l.id).or_default();
                        if fwd {
                            e.0 += up;
                            e.1 += down;
                        } else {
                            e.0 += down;
                            e.1 += up;
                        }
                        out.who.entry(l.id).or_default().push(d.id);
                    }
                }
            }
        }
        out
    }

    /// How full a link is (0..): radio links share the air both ways, cables are full duplex.
    pub fn utilisation(&self, load: &Load, link: u32) -> f32 {
        let Some(l) = self.links.iter().find(|l| l.id == link) else { return 0.0 };
        let Some(&(a, b)) = load.per_link.get(&link) else { return 0.0 };
        let rate = self.link_rate(l).max(0.1);
        if l.medium.wireless() { (a + b) / rate } else { a.max(b) / rate }
    }

    /// Airtime an access point needs on its channel for all its clients (1.0 = the channel is full).
    pub fn airtime(&self, load: &Load, ap: u32) -> f32 {
        let links: Vec<&crate::net::Link> = self.links.iter().filter(|l| l.medium == Medium::WiFi && self.wifi_ends(l).is_some_and(|(a, _)| a.id == ap)).collect();
        // Wi-Fi 6 and later (OFDMA) pack many small transmissions into one: crowds cost less airtime.
        let modern = |l: &&crate::net::Link| self.wifi_ends(l).is_some_and(|(a, c)| a.wifi >= crate::net::WifiStd::Wifi6 && c.wifi >= crate::net::WifiStd::Wifi6);
        let packing = if links.len() > 4 { 0.8 } else { 1.0 };
        links.iter().map(|l| self.utilisation(load, l.id) * if modern(l) { packing } else { 1.0 }).sum()
    }

    /// Extra delay (ms) from full queues along a path ("bufferbloat").
    pub fn queue_ms(&self, load: &Load, path: &[u32]) -> f32 {
        path.windows(2)
            .filter_map(|w| self.link_between(w[0], w[1]))
            .map(|l| {
                let u = self.utilisation(load, l.id).min(0.98);
                if self.sqm_on(l) {
                    // SQM keeps the queue short even when the line is full.
                    0.5 + 2.0 * u
                } else {
                    // A queue at load u waits u/(1-u) service times; home routers buffer ~5 ms per packet burst.
                    5.0 * u / (1.0 - u)
                }
            })
            .sum()
    }
}

// ---------- Power over Ethernet ----------

/// Per-port PoE standards: (label, watts at the switch port).
pub const POE: [(&str, f32); 5] = [("No PoE", 0.0), ("PoE 802.3af — 15.4 W/port", 15.4), ("PoE+ 802.3at — 30 W/port", 30.0), ("PoE++ 802.3bt — 60 W/port", 60.0), ("PoE++ 802.3bt — 90 W/port", 90.0)];

/// Typical power a device draws over the cable (W); 0 = it has its own power supply.
pub fn default_poe_draw(kind: Kind) -> f32 {
    match kind {
        Kind::AccessPoint => 18.0,
        Kind::Camera => 6.5,
        _ => 0.0,
    }
}

impl Network {
    /// PoE problems: (device, text, the switch that should supply it).
    pub fn poe_problems(&self) -> Vec<(u32, String, Option<u32>)> {
        let mut v = vec![];
        let mut used: HashMap<u32, f32> = HashMap::new();
        for d in self.devices.iter().filter(|d| d.poe_draw > 0.0) {
            let Some(l) = self.links.iter().find(|l| (l.a == d.id || l.b == d.id) && !l.medium.wireless() && l.medium != Medium::Powerline) else { continue };
            let src = self.dev(l.other(d.id).0).unwrap();
            let port_w = POE[src.poe_class as usize].1;
            // The cable loses a little on the way (up to ~16% at 100 m).
            let need = d.poe_draw * (1.0 + 0.16 * (self.link_len(l) / 100.0).min(1.0));
            if port_w == 0.0 {
                v.push((d.id, format!("{} needs about {:.0} W over the cable, but {} has no PoE. Use a PoE switch or an injector.", d.name, d.poe_draw, src.name), Some(src.id)));
            } else if need > port_w {
                v.push((d.id, format!("{} needs {:.0} W but {}'s ports give only {:.1} W ({}). It won't start or keeps rebooting.", d.name, need, src.name, port_w, POE[src.poe_class as usize].0), Some(src.id)));
            } else {
                *used.entry(src.id).or_default() += need;
            }
        }
        for (sw, w) in used {
            let s = self.dev(sw).unwrap();
            if w > s.poe_budget {
                v.push((sw, format!("{} has a PoE budget of {:.0} W but its devices need {:.0} W. The last ones to start get no power.", s.name, s.poe_budget, w), None));
            }
        }
        v
    }
}

// ---------- firewall rules ----------

fn private(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

impl Network {
    /// Does `what` ("any", "lan", "guest", "internet", an address or a network/prefix) cover `ip`?
    fn matches(&self, what: &str, ip: Ipv4Addr) -> bool {
        match what {
            "any" | "*" => true,
            "lan" => private(ip) && !self.is_guest(ip),
            "guest" => self.is_guest(ip),
            "internet" | "wan" => !private(ip),
            w => match w.split_once('/') {
                Some((n, p)) => parse_ip(n).zip(p.parse().ok()).is_some_and(|(n, p)| same_net(n, ip, p)),
                None => parse_ip(w) == Some(ip),
            },
        }
    }

    fn is_guest(&self, ip: Ipv4Addr) -> bool {
        self.devices.iter().any(|d| d.ifaces.iter().any(|f| f.vlan == GUEST_VLAN && f.addr().is_some_and(|(a, p)| same_net(a, ip, p))))
    }

    /// First matching allow/deny line on a router. None = no rule matched (allowed).
    pub fn rule_verdict(&self, d: &Device, src: Ipv4Addr, dst: Ipv4Addr, port: u16) -> Option<(bool, String)> {
        for line in d.rules.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let w: Vec<&str> = line.split_whitespace().collect();
            let allow = match w.first() {
                Some(&"allow") => true,
                Some(&"deny") | Some(&"block") => false,
                _ => continue,
            };
            let (from, to) = (w.get(1).copied().unwrap_or("any"), w.get(2).copied().unwrap_or("any"));
            let port_ok = w.get(3).is_none_or(|p| *p == "any" || p.parse() == Ok(port));
            if self.matches(from, src) && self.matches(to, dst) && port_ok {
                return Some((allow, line.to_string()));
            }
        }
        None
    }

    /// Port forwarding on a home router: "forward 443 192.168.1.10".
    pub fn forward_of(&self, d: &Device, port: u16) -> Option<Ipv4Addr> {
        d.rules.lines().find_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            (w.first() == Some(&"forward") && w.get(1).and_then(|p| p.parse().ok()) == Some(port)).then(|| w.get(2).and_then(|i| parse_ip(i))).flatten()
        })
    }
}

// ---------- IPv6 ----------

impl Network {
    /// The /64 a home router hands to its network (documentation range 2001:db8::/32).
    pub fn v6_prefix(&self, router: &Device) -> Option<String> {
        (router.kind == Kind::HomeRouter && router.ipv6).then(|| format!("2001:db8:{:x}:{:x}", (self.seed & 0xffff) as u16, router.id))
    }

    /// A device's global IPv6 address (SLAAC: prefix + its own interface id), if its network has IPv6.
    pub fn v6_of(&self, d: &Device) -> Option<String> {
        if d.kind == Kind::HomeRouter {
            return self.v6_prefix(d).map(|p| format!("{p}::1"));
        }
        let segs = self.segments();
        let seg = segs.get(&(d.id, 0))?;
        let r = self.devices.iter().find(|r| r.kind == Kind::HomeRouter && segs.get(&(r.id, 0)) == Some(seg))?;
        let prefix = self.v6_prefix(r)?;
        let h = crate::sim::hash(self.seed, d.id as u64, 0x66);
        Some(format!("{prefix}:{:x}:{:x}:{:x}:{:x}", (h >> 48) as u16, (h >> 32) as u16, (h >> 16) as u16, h as u16))
    }

    /// ping6: outbound works without NAT; inbound from the internet needs a firewall rule.
    pub fn ping6(&self, id: u32, target: &str, o: &mut Outcome) {
        let me = self.dev(id).unwrap();
        let site = SITES.iter().position(|s| s.0 == target);
        let other = self.devices.iter().find(|d| d.hostname() == target.to_lowercase() || d.name.eq_ignore_ascii_case(target));
        let from_internet = me.kind == Kind::Internet;
        let mine = if from_internet { Some("2001:db8:ffff::1".to_string()) } else { self.v6_of(me) };
        let Some(mine) = mine else {
            o.lines.push(format!("{} has no IPv6 address: its router doesn't offer IPv6 (or it isn't connected).", me.name));
            return;
        };
        let (dst6, path_from, path_to) = match (site, other) {
            (Some(i), _) => (format!("2001:db8:113::{:x}", 10 * (i + 1)), id, crate::net::site_ip(i)),
            (None, Some(t)) => {
                let Some(v6) = self.v6_of(t) else {
                    o.lines.push(format!("{} has no IPv6 address.", t.name));
                    return;
                };
                // shortcut: IPv6 reuses the IPv4 path (same cables and routers); fine for home networks, not for v6-only routing.
                if from_internet {
                    let Some(r) = self.devices.iter().find(|r| r.kind == Kind::HomeRouter && r.ipv6 && self.v6_of(t).is_some_and(|a| a.starts_with(&self.v6_prefix(r).unwrap_or_default()))) else {
                        return;
                    };
                    let allowed = self.rule_verdict(r, Ipv4Addr::new(198, 51, 100, 99), t.first_ip().unwrap_or(Ipv4Addr::UNSPECIFIED), 0).is_some_and(|v| v.0);
                    o.lines.push(format!("PING {v6} from {mine}"));
                    if !allowed {
                        o.lines.push(format!("Blocked by {}'s firewall: with IPv6 every device has a public address, so the router drops unsolicited traffic from the internet. Add 'allow internet {} any' to open it.", r.name, t.first_ip().map(|i| i.to_string()).unwrap_or_default()));
                        o.lines.push("4 packets transmitted, 0 received, 100% packet loss".into());
                    } else {
                        o.lines.push(format!("64 bytes from {v6}: icmp_seq=1..4 time≈20 ms — allowed by a rule on {}", r.name));
                        o.lines.push("4 packets transmitted, 4 received, 0% packet loss".into());
                    }
                    return;
                }
                (v6, id, t.first_ip().unwrap_or(Ipv4Addr::UNSPECIFIED))
            }
            _ => {
                o.lines.push(format!("ping6: unknown host {target}"));
                return;
            }
        };
        let Some(src) = me.first_ip() else { return };
        let t = self.send(path_from, src, path_to, "ICMPv6", 0, None);
        o.lines.push(format!("PING {dst6} from {mine}"));
        if t.ok {
            o.lines.push(format!("64 bytes from {dst6}: icmp_seq=1..4 time={:.1} ms — no NAT: the address is the same end to end", t.ms * 2.0));
            o.lines.push("4 packets transmitted, 4 received, 0% packet loss".into());
        } else {
            o.lines.push(format!("No reply: {}", t.error));
        }
        o.traces.push(t);
    }
}

// ---------- guest network ----------

impl Network {
    /// Turns a home router's guest Wi-Fi on or off (its own network 192.168.99.0/24, isolated from the home).
    pub fn set_guest(&mut self, id: u32, on: bool) {
        let d = self.dev_mut(id).unwrap();
        let has = d.ifaces.iter().position(|f| f.vlan == GUEST_VLAN);
        match (on, has) {
            (true, None) => {
                let mac = d.ifaces[0].mac.clone();
                d.ifaces.push(crate::net::Iface { name: "guest".into(), ip: "192.168.99.1".into(), prefix: 24, mac, vlan: GUEST_VLAN, parent: Some(0) });
                if d.guest_ssid.is_empty() {
                    d.guest_ssid = format!("{}-Guest", d.ssid);
                }
            }
            (false, Some(i)) => {
                d.ifaces.remove(i);
            }
            _ => {}
        }
    }
}

// ---------- real measurements ----------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sample {
    pub pos: [f32; 2],
    pub floor: i8,
    /// Access point the reading belongs to.
    pub ap: u32,
    pub rssi: f32,
}

impl Network {
    /// What the model predicts for a reading (strongest enabled band, like survey apps show).
    pub fn predict(&self, s: &Sample) -> Option<f32> {
        let ap = self.dev(s.ap)?;
        Some(Band::ALL.iter().map(|b| self.rssi_at(ap, s.pos, s.floor, *b)).fold(-120.0, f32::max))
    }

    /// Mean absolute error of the model against the readings.
    pub fn survey_error(&self) -> Option<f32> {
        let e: Vec<f32> = self.survey.iter().filter_map(|s| self.predict(s).map(|p| (p - s.rssi).abs())).collect();
        (!e.is_empty()).then(|| e.iter().sum::<f32>() / e.len() as f32)
    }

    /// Fits each access point's calibration offset to the readings (least squares = mean difference).
    pub fn calibrate(&mut self) -> String {
        let before = self.survey_error();
        let mut sums: HashMap<u32, (f32, usize)> = HashMap::new();
        for s in &self.survey {
            if let Some(p) = self.predict(s) {
                let e = sums.entry(s.ap).or_default();
                e.0 += s.rssi - p;
                e.1 += 1;
            }
        }
        let mut done = vec![];
        for (ap, (sum, n)) in sums {
            let d = self.dev_mut(ap).unwrap();
            d.cal_db = (d.cal_db + sum / n as f32).clamp(-25.0, 25.0);
            done.push(format!("{} {:+.1} dB", d.name, d.cal_db));
        }
        match (before, self.survey_error()) {
            (Some(b), Some(a)) => format!("Calibrated ({}): average error {b:.1} → {a:.1} dB.", done.join(", ")),
            _ => "No readings to calibrate with.".into(),
        }
    }

    /// Reads a survey file: one reading per line, `x_m, y_m, floor, access point name, dBm` (header lines are skipped).
    pub fn import_survey(&mut self, text: &str) -> Result<usize, String> {
        let mut n = 0;
        for (i, line) in text.lines().enumerate() {
            let f: Vec<&str> = line.split([',', ';', '\t']).map(str::trim).collect();
            if f.len() < 5 || f[0].parse::<f32>().is_err() {
                continue;
            }
            let num = |s: &str| s.parse::<f32>().map_err(|_| format!("line {}: '{s}' isn't a number", i + 1));
            let (x, y, floor, rssi) = (num(f[0])?, num(f[1])?, num(f[2])? as i8, num(f[4])?);
            let ap = self.devices.iter().find(|d| d.name.eq_ignore_ascii_case(f[3]) || d.ssid.eq_ignore_ascii_case(f[3])).map(|d| d.id).ok_or(format!("line {}: no access point called '{}'", i + 1, f[3]))?;
            if !(-110.0..=0.0).contains(&rssi) {
                return Err(format!("line {}: {rssi} dBm isn't a Wi-Fi reading", i + 1));
            }
            self.survey.push(Sample { pos: [x * crate::net::PX_PER_M, y * crate::net::PX_PER_M], floor, ap, rssi });
            n += 1;
        }
        if n == 0 { Err("No readings found. Expected lines like: 4.5, 3.0, 0, Home router, -61".into()) } else { Ok(n) }
    }
}

// ---------- floor plan pictures ----------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FloorPlan {
    pub floor: i8,
    pub path: String,
    /// Top-left corner on the map (px) and real width (m).
    pub pos: [f32; 2],
    pub width_m: f32,
    pub opacity: f32,
}

// ---------- model library ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub spec: Device,
}

impl Model {
    pub fn from(name: &str, d: &Device) -> Model {
        let mut spec = d.clone();
        spec.model = name.into();
        Model { name: name.into(), spec }
    }
    /// Copies the hardware (not the name, place or addresses) onto a device.
    pub fn apply(&self, d: &mut Device) {
        let s = &self.spec;
        d.model = self.name.clone();
        d.wifi = s.wifi;
        d.streams = s.streams;
        d.bands = s.bands;
        d.width = s.width;
        d.tx = s.tx;
        d.backhaul = s.backhaul;
        d.ports = s.ports.clone();
        d.poe_class = s.poe_class;
        d.poe_budget = s.poe_budget;
        d.poe_draw = s.poe_draw;
        d.price = s.price;
        d.stp_short = s.stp_short;
    }
}

/// The user's model library, shared by all plans.
pub fn library_path() -> std::path::PathBuf {
    // XDG on Linux, %APPDATA% on Windows, ~/.config elsewhere.
    let base = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("APPDATA").map(std::path::PathBuf::from)).or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config"))).unwrap_or_else(|| ".".into());
    base.join("netsketch").join("models.json")
}

pub fn load_library() -> Vec<Model> {
    std::fs::read_to_string(library_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_library(models: &[Model]) -> Result<(), String> {
    let p = library_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(models).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{template_seeded, Kind};

    fn id(n: &Network, name: &str) -> u32 {
        n.devices.iter().find(|d| d.name == name).unwrap().id
    }

    #[test]
    fn app_traffic_fills_links_and_adds_delay() {
        let mut n = template_seeded("home", 3);
        let tv = id(&n, "Living room TV");
        let quiet = n.loads();
        let base = n.utilisation(&quiet, n.links.iter().find(|l| l.a == tv || l.b == tv).unwrap().id);
        assert!(base > 0.0);
        // Everyone starts a big download: the internet link fills up and queues grow.
        for d in n.devices.iter_mut().filter(|d| d.kind.is_host()) {
            d.uses.push(Use::Download);
        }
        let busy = n.loads();
        let inet = n.links.iter().find(|l| n.dev(l.a).unwrap().kind == Kind::Internet || n.dev(l.b).unwrap().kind == Kind::Internet).unwrap().id;
        assert!(n.utilisation(&busy, inet) > 1.0);
        let pc = id(&n, "Office PC");
        let path = n.send(pc, n.dev(pc).unwrap().first_ip().unwrap(), crate::net::site_ip(0), "TCP", 443, None).path;
        assert!(n.queue_ms(&busy, &path) > n.queue_ms(&quiet, &path) + 20.0);
        assert!(n.check().iter().any(|i| i.text.contains("busy hour")), "{:?}", n.check());
    }

    #[test]
    fn sqm_and_microwaves() {
        let mut n = template_seeded("home", 3);
        for d in n.devices.iter_mut().filter(|d| d.kind.is_host()) {
            d.uses.push(Use::Download);
        }
        let pc = id(&n, "Office PC");
        let path = n.send(pc, n.dev(pc).unwrap().first_ip().unwrap(), crate::net::site_ip(0), "TCP", 443, None).path;
        let before = n.queue_ms(&n.loads(), &path);
        let fix = n.check().into_iter().find_map(|i| i.fix.filter(|f| matches!(f, crate::issues::Fix::Sqm(_)))).expect("SQM suggested");
        n.apply_fix(&fix);
        assert!(n.queue_ms(&n.loads(), &path) < before / 5.0);
        // A microwave next to the router eats 2.4 GHz airtime but leaves 5 GHz alone.
        let r = id(&n, "Home router");
        let rp = n.dev(r).unwrap().pos;
        let (c24, c5) = (n.contention(n.dev(r).unwrap(), Band::B24).0, n.contention(n.dev(r).unwrap(), Band::B5).0);
        let mw = n.add_device(Kind::Interference, [rp[0] + 60.0, rp[1]]);
        n.dev_mut(mw).unwrap().ch[0] = n.dev(r).unwrap().ch[0];
        assert!(n.contention(n.dev(r).unwrap(), Band::B24).0 > c24 + 0.3);
        assert_eq!(n.contention(n.dev(r).unwrap(), Band::B5).0, c5);
        assert!(n.connect(mw, r, Medium::Cat6).is_err());
    }

    #[test]
    fn poe_budget_and_classes() {
        let mut n = template_seeded("blank", 1);
        let sw = n.add_device(Kind::Switch, [0.0, 0.0]);
        let ap = n.add_device(Kind::AccessPoint, [60.0, 0.0]);
        n.connect(sw, ap, Medium::Cat6).unwrap();
        assert!(n.poe_problems().iter().any(|p| p.1.contains("no PoE")));
        n.dev_mut(sw).unwrap().poe_class = 1; // 15.4 W, AP wants 18 W
        assert!(n.poe_problems().iter().any(|p| p.1.contains("only")));
        n.dev_mut(sw).unwrap().poe_class = 2;
        n.dev_mut(sw).unwrap().poe_budget = 10.0;
        assert!(n.poe_problems().iter().any(|p| p.1.contains("budget")));
        n.dev_mut(sw).unwrap().poe_budget = 60.0;
        assert!(n.poe_problems().is_empty());
    }

    #[test]
    fn firewall_forwarding_and_guest_isolation() {
        let mut n = template_seeded("home", 5);
        let (r, pc, nas, phone) = (id(&n, "Home router"), id(&n, "Office PC"), id(&n, "NAS 1"), id(&n, "Phone"));
        let nas_ip = n.dev(nas).unwrap().first_ip().unwrap().to_string();
        // Rules apply where traffic passes the router: here, the way out to the internet.
        n.dev_mut(r).unwrap().rules = "deny lan internet 443".into();
        let out = n.exec(pc, "open example.com").lines.join(" ");
        assert!(out.contains("firewall"), "{out}");
        assert!(n.exec(pc, "ping example.com").lines.join(" ").contains(", 4 received"));
        n.dev_mut(r).unwrap().rules.clear();
        n.set_guest(r, true);
        n.dev_mut(phone).unwrap().ifaces[0].vlan = GUEST_VLAN;
        n.run_dhcp();
        assert!(n.dev(phone).unwrap().ifaces[0].ip.starts_with("192.168.99."), "{:?}", n.dev(phone).unwrap().ifaces);
        assert!(n.exec(phone, "ping example.com").lines.join(" ").contains(", 4 received"));
        assert!(n.exec(phone, &format!("ping {nas_ip}")).lines.join(" ").contains("isolated"));
        // Port forwarding: the internet reaches the NAS's web page through the router's public address.
        let wan = n.dev(r).unwrap().ifaces[1].ip.clone();
        n.dev_mut(r).unwrap().rules = format!("forward 443 {nas_ip}");
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        assert!(n.exec(inet, &format!("open {wan}")).lines.join(" ").contains("200 OK"));
        // IPv6: outbound fine, inbound blocked until a rule allows it.
        assert!(n.exec(pc, "ping6 example.com").lines.join(" ").contains("no NAT"));
        assert!(n.exec(inet, "ping6 office-pc").lines.join(" ").contains("Blocked"));
    }

    #[test]
    fn calibration_pulls_the_model_to_the_readings() {
        let mut n = template_seeded("home", 2);
        let r = id(&n, "Home router");
        let rp = n.dev(r).unwrap().pos;
        // Readings 8 dB weaker than the model everywhere (an antenna behind a metal cabinet, say).
        let mut csv = String::from("x,y,floor,ap,dbm\n");
        for (dx, dy) in [(60.0, 0.0), (0.0, 120.0), (200.0, 40.0)] {
            let s = Sample { pos: [rp[0] + dx, rp[1] + dy], floor: 0, ap: r, rssi: 0.0 };
            let p = n.predict(&s).unwrap();
            csv += &format!("{},{},0,Home router,{}\n", s.pos[0] / crate::net::PX_PER_M, s.pos[1] / crate::net::PX_PER_M, p - 8.0);
        }
        assert_eq!(n.import_survey(&csv).unwrap(), 3);
        assert!(n.survey_error().unwrap() > 7.0);
        n.calibrate();
        assert!(n.survey_error().unwrap() < 0.5);
        assert!(n.import_survey("1,2,0,Nobody,-50").is_err());
    }
}
