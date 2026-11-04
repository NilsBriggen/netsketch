//! Planning extras: everyday app traffic with QoS priorities, Power over Ethernet, firewall rules, IPv6, the guest network,
//! real measurements for calibration, floor plan pictures and a library of device models.
use crate::net::{parse_ip, same_net, Device, Kind, Medium, Network, Outcome, Qos, SITES};
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
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
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
    /// Typical busy-hour demand (down, up) in Mbit/s. A big download really takes whatever the internet
    /// line has left (see `loads`); 150 is its demand where there's no internet line.
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
    /// The same per link and QoS priority of the device that causes it.
    pub per_class: HashMap<(u32, Qos), (f32, f32)>,
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
        // (device, app, how active, path, goes out to the internet)
        let mut flows = vec![];
        for d in self.devices.iter().filter(|d| !d.uses.is_empty() && !d.off) {
            let Some(src) = d.first_ip() else { continue };
            for u in &d.uses {
                let act = crate::more::activity(*u, self.hour);
                if act < 0.01 {
                    continue;
                }
                let Some(dst) = self.target(*u) else { continue };
                let t = self.send(d.id, src, dst, "TCP", 443, None);
                if t.ok {
                    flows.push((d.id, *u, act, t.path, t.site.is_some()));
                }
            }
        }
        // A big download (TCP) fills whatever the internet line has left, shared between all of them.
        let line = self.devices.iter().find(|d| d.kind == Kind::Internet).map(|d| d.plan_mbps);
        let busy: f32 = flows.iter().filter(|f| f.4 && f.1 != Use::Download).map(|f| f.1.demand().0 * f.2).sum();
        let downloads: f32 = flows.iter().filter(|f| f.4 && f.1 == Use::Download).map(|f| f.2).sum();
        for (id, u, act, path, net) in flows {
            let qos = self.dev(id).map_or(Qos::Normal, |d| d.qos);
            let (mut down, up) = u.demand();
            if let (Use::Download, true, Some(l)) = (u, net, line) {
                down = (l - busy).max(l * 0.1) / downloads.max(1.0);
            }
            let (down, up) = (down * act, up * act);
            {
                for w in path.windows(2) {
                    if let Some(l) = self.link_between(w[0], w[1]) {
                        // Direction "a→b" vs "b→a" for full-duplex cables.
                        let (to_a, to_b) = if l.a == w[0] { (up, down) } else { (down, up) };
                        for e in [out.per_link.entry(l.id).or_default(), out.per_class.entry((l.id, qos)).or_default()] {
                            e.0 += to_a;
                            e.1 += to_b;
                        }
                        out.who.entry(l.id).or_default().push(id);
                    }
                }
            }
        }
        out
    }

    /// How full a link is (0..): radio links share the air both ways, cables are full duplex.
    pub fn utilisation(&self, load: &Load, link: u32) -> f32 {
        self.fill(link, load.per_link.get(&link).copied())
    }

    /// How full a link looks to traffic of priority `q`: a full link serves higher priorities first, so only
    /// traffic of the same or a higher priority is in its way (with nobody prioritised: everyone's).
    pub fn utilisation_for(&self, load: &Load, link: u32, q: Qos) -> f32 {
        let ahead = Qos::ALL.iter().filter(|c| **c <= q).filter_map(|c| load.per_class.get(&(link, *c))).fold((0.0, 0.0), |s, x| (s.0 + x.0, s.1 + x.1));
        self.fill(link, Some(ahead))
    }

    fn fill(&self, link: u32, load: Option<(f32, f32)>) -> f32 {
        let (Some(l), Some((a, b))) = (self.links.iter().find(|l| l.id == link), load) else { return 0.0 };
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

    /// Extra delay (ms) from full queues along a path ("bufferbloat"), for the QoS priority of the device it starts at.
    pub fn queue_ms(&self, load: &Load, path: &[u32]) -> f32 {
        let q = path.first().and_then(|id| self.dev(*id)).map_or(Qos::Normal, |d| d.qos);
        path.windows(2)
            .filter_map(|w| self.link_between(w[0], w[1]))
            .map(|l| {
                let u = self.utilisation_for(load, l.id, q).min(0.98);
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

/// Power a switch port reserves for a device that draws `draw` W: its PoE class maximum at the port
/// (af 15.4 W, at 30 W, bt 60 or 90 W), which also covers what the cable loses on the way (~16%, ~20% on bt).
/// None: more than PoE can carry.
// shortcut: switches reserve by class; ones that negotiate the real need (LLDP) can fit a few more devices.
pub fn poe_reserve(draw: f32) -> Option<f32> {
    [(12.95, 15.4), (25.5, 30.0), (51.0, 60.0), (71.3, 90.0)].into_iter().find(|(pd, _)| draw <= *pd).map(|x| x.1)
}

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
            let need = poe_reserve(d.poe_draw);
            if port_w == 0.0 {
                v.push((d.id, crate::i18n::trf("{} needs about {} W over the cable, but {} has no PoE. Use a PoE switch or an injector.", &[&d.name, &format!("{:.0}", d.poe_draw), &src.name]), Some(src.id)));
            } else if need.is_none_or(|w| w > port_w) {
                let class = need.map_or(crate::i18n::tr("more than PoE can carry").to_string(), |w| format!("a {w:.1} W port"));
                v.push((d.id, crate::i18n::trf("{} draws {} W, so it needs {}, but {}'s ports give only {} W ({}). It won't start or keeps rebooting.", &[&d.name, &format!("{:.0}", d.poe_draw), &class, &src.name, &format!("{:.1}", port_w), &(POE[src.poe_class as usize].0)]), Some(src.id)));
            } else {
                *used.entry(src.id).or_default() += need.unwrap_or(0.0);
            }
        }
        for (sw, w) in used {
            let s = self.dev(sw).unwrap();
            if w > s.poe_budget {
                v.push((sw, crate::i18n::trf("{} has a PoE budget of {} W but reserves {} W for its devices (each one's class maximum). The last ones to start get no power.", &[&s.name, &format!("{:.0}", s.poe_budget), &format!("{:.0}", w)]), None));
            }
        }
        v
    }
}

// ---------- firewall rules ----------

fn private(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

/// Does a rule's port ("443", "any") or range ("50000-50100") cover `port`?
fn port_in(spec: &str, port: u16) -> bool {
    match spec.split_once('-') {
        Some((a, b)) => a.parse::<u16>().ok().zip(b.parse::<u16>().ok()).is_some_and(|(a, b)| (a..=b).contains(&port)),
        None => spec == "any" || spec.parse() == Ok(port),
    }
}

/// How rules name a simulated packet's transport protocol (ping has none).
fn l4(proto: &str) -> &'static str {
    match proto {
        "TCP" | "HTTP" | "HTTPS" => "tcp",
        "UDP" | "DNS" | "DHCP" | "QUIC" => "udp",
        _ => "",
    }
}

/// Is `hour` inside a time window like "22:00-06:30" (it may run past midnight)? None: not a time window.
fn in_window(spec: &str, hour: f32) -> Option<bool> {
    let t = |s: &str| -> Option<f32> {
        let (h, m) = s.split_once(':')?;
        Some(h.parse::<f32>().ok()? + m.parse::<f32>().ok()? / 60.0)
    };
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (t(a)?, t(b)?);
    Some(if a <= b { (a..b).contains(&hour) } else { hour >= a || hour < b })
}

/// The ports a device's games ask the router to open over UPnP: Xbox Live 3074, PlayStation Network
/// 3478–3480, and Steam (PCs) 27015–27030. Devices that don't play games online ask for nothing.
pub fn upnp_ports(d: &Device) -> &'static [(u16, u16)] {
    let says = |w: &str| d.model.to_lowercase().contains(w) || d.name.to_lowercase().contains(w);
    if d.off || !(d.kind == Kind::Console || d.uses.contains(&Use::Gaming)) {
        &[]
    } else if says("xbox") {
        &[(3074, 3074)]
    } else if says("playstation") || says("ps5") || says("ps4") {
        &[(3478, 3480)]
    } else if d.kind == Kind::Console {
        // A console of unknown make asks for both networks' ports.
        &[(3074, 3074), (3478, 3480)]
    } else {
        &[(27015, 27030)]
    }
}

impl Network {
    /// Does `what` ("any", "lan", "guest", "internet", an address, a network/prefix or a device's name) cover `ip`?
    fn matches(&self, what: &str, ip: Ipv4Addr) -> bool {
        match what {
            "any" | "*" => true,
            "lan" => private(ip) && !self.is_guest(ip),
            "guest" => self.is_guest(ip),
            "internet" | "wan" => !private(ip),
            w => match w.split_once('/') {
                Some((n, p)) => parse_ip(n).zip(p.parse().ok()).is_some_and(|(n, p)| same_net(n, ip, p)),
                None => parse_ip(w).map_or_else(|| self.devices.iter().any(|d| (d.hostname() == w || d.name.eq_ignore_ascii_case(w)) && d.has_ip(ip)), |a| a == ip),
            },
        }
    }

    fn is_guest(&self, ip: Ipv4Addr) -> bool {
        self.devices.iter().any(|d| d.ifaces.iter().any(|f| f.vlan == GUEST_VLAN && f.addr().is_some_and(|(a, p)| same_net(a, ip, p))))
    }

    /// First matching allow/deny line on a router. None = no rule matched (allowed).
    /// A line: allow|deny <from> <to> [port or range] [tcp|udp] [time window like 22:00-06:30, from the plan's hour].
    pub fn rule_verdict(&self, d: &Device, src: Ipv4Addr, dst: Ipv4Addr, port: u16, proto: &str) -> Option<(bool, String)> {
        for line in d.rules.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let w: Vec<&str> = line.split_whitespace().collect();
            let allow = match w.first() {
                Some(&"allow") => true,
                Some(&"deny") | Some(&"block") => false,
                _ => continue,
            };
            let (from, to) = (w.get(1).copied().unwrap_or("any"), w.get(2).copied().unwrap_or("any"));
            let extra_ok = w.iter().skip(3).all(|t| match *t {
                "tcp" | "udp" => *t == l4(proto),
                t => in_window(t, self.hour).unwrap_or_else(|| port_in(t, port)),
            });
            if self.matches(from, src) && self.matches(to, dst) && extra_ok {
                return Some((allow, line.to_string()));
            }
        }
        None
    }

    /// Port forwarding on a home router: "forward 443 192.168.1.10", "forward 50000-50100 192.168.1.20 udp";
    /// with UPnP on, games open their usual ports themselves.
    pub fn forward_of(&self, d: &Device, port: u16, proto: &str) -> Option<Ipv4Addr> {
        let rule = d.rules.lines().find_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let proto_ok = w.get(3).is_none_or(|p| *p == l4(proto));
            (w.first() == Some(&"forward") && w.get(1).is_some_and(|p| port_in(p, port)) && proto_ok).then(|| w.get(2).and_then(|i| parse_ip(i))).flatten()
        });
        // UPnP: each device maps the ports its own games ask for; a port someone already holds goes to the first.
        let mapped = || self.devices.iter().find(|g| upnp_ports(g).iter().any(|r| (r.0..=r.1).contains(&port))).and_then(|g| g.first_ip());
        rule.or_else(|| d.upnp.then(mapped).flatten())
    }
}

// ---------- remote access ----------

impl Network {
    /// Whether the home router's WireGuard VPN can be reached from outside, and why. None = the VPN is off.
    pub fn remote_access(&self, r: &Device) -> Option<(bool, String)> {
        if r.kind != Kind::HomeRouter || !r.vpn {
            return None;
        }
        let name = if r.ddns.trim().is_empty() { crate::i18n::tr("the router's address").to_string() } else { r.ddns.trim().to_string() };
        let v6 = self.v6_prefix(r).is_some();
        let wan = r.ifaces.get(1).and_then(|f| f.addr()).map(|a| a.0);
        Some(match (self.cgnat(r), wan) {
            (Some(cg), _) if v6 => (true, crate::i18n::trf("Works over IPv6 only. On the mobile line the router shares the IPv4 address {} with other customers (carrier-grade NAT), but {} also points at its public IPv6 address. From a network without IPv6 it fails.", &[&cg, &name])),
            (Some(cg), _) => (false, crate::i18n::trf("Doesn't work. On the mobile line the router only has the shared address {} (carrier-grade NAT), so nothing from outside reaches it. Turn on IPv6, or use a relay service.", &[&cg])),
            (None, Some(ip)) if private(ip) => (false, crate::i18n::trf("Doesn't work yet: {} sits behind another router ({} is a private address). Forward UDP port 51820 on the router in front, or run the VPN there.", &[&r.name, &ip])),
            (None, Some(ip)) => (
                true,
                crate::i18n::trf("Works: {} leads to the public address {}, WireGuard listens on UDP port 51820, and phones and laptops are inside the home network from anywhere.{}", &[&name, &ip, &(if r.ddns.trim().is_empty() { " Set a dynamic DNS name: the provider may change the address." } else { "" })]),
            ),
            (None, None) => (false, crate::i18n::trf("Doesn't work: {} has no internet address yet.", &[&r.name])),
        })
    }
}

// ---------- IPv6 ----------

impl Network {
    /// The /64 a home router hands to its network (documentation range 2001:db8::/32). The provider delegates a
    /// /56 (256 networks) to the router on the line (DHCPv6-PD), which uses the first /64; every router behind
    /// it, however many levels deep, asks the one in front for a /64 out of that /56 the same way. A router in
    /// the chain without IPv6 cuts everything behind it off.
    pub fn v6_prefix(&self, router: &Device) -> Option<String> {
        if router.kind != Kind::HomeRouter || !router.ipv6 {
            return None;
        }
        let segs = self.segments();
        let upstream = |r: &Device| {
            let wan = segs.get(&(r.id, 1));
            self.devices.iter().find(|u| u.id != r.id && u.kind == Kind::HomeRouter && wan.is_some() && segs.get(&(u.id, 0)) == wan)
        };
        let mut top = router;
        for _ in 0..8 {
            match upstream(top) {
                Some(u) if !u.ipv6 => return None,
                Some(u) => top = u,
                None => break,
            }
        }
        let sub = if top.id == router.id { 0 } else { 1 + (router.id % 255) as u16 };
        Some(format!("2001:db8:{:x}:{:x}", (self.seed & 0xffff) as u16, self.v6_site(top) << 8 | sub))
    }

    /// Which /56 the provider delegates to a router (the high byte of its networks' fourth group).
    fn v6_site(&self, top: &Device) -> u16 {
        (crate::sim::hash(self.seed, top.id as u64, 0x56) & 0xff) as u16
    }

    /// ipconfig lines on a home router: where its IPv6 networks come from.
    pub fn v6_delegation(&self, r: &Device) -> Vec<String> {
        let Some(p) = self.v6_prefix(r) else { return vec![] };
        let site = format!("2001:db8:{:x}:{:x}00::/56", (self.seed & 0xffff) as u16, p.rsplit(':').next().and_then(|g| u16::from_str_radix(g, 16).ok()).unwrap_or(0) >> 8);
        vec![
            format!("IPv6 networks: {site} delegated by the provider (DHCPv6-PD); this router uses {p}::/64"),
            "Local-only IPv6 (ULA, fd00::/8) also works without the provider, like 192.168.x.x does for IPv4.".into(),
        ]
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

    /// The first router on a path (after the sender) that passes only IPv4.
    pub fn v4_only_router(&self, path: &[u32]) -> Option<&Device> {
        path.iter().skip(1).filter_map(|x| self.dev(*x)).find(|d| matches!(d.kind, Kind::Router | Kind::HomeRouter) && !d.ipv6)
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
                if from_internet {
                    let Some(r) = self.devices.iter().find(|r| r.kind == Kind::HomeRouter && r.ipv6 && self.v6_of(t).is_some_and(|a| a.starts_with(&self.v6_prefix(r).unwrap_or_default()))) else {
                        return;
                    };
                    let allowed = self.rule_verdict(r, Ipv4Addr::new(198, 51, 100, 99), t.first_ip().unwrap_or(Ipv4Addr::UNSPECIFIED), 0, "ICMP").is_some_and(|v| v.0);
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
        // IPv6 runs over the same cables, but each router on the way must route IPv6 itself, and nothing is translated.
        let mut t = self.send(path_from, src, path_to, "ICMPv6", 0, None);
        t.log.retain(|l| l.proto != "NAT");
        if let Some(r) = self.v4_only_router(&t.path) {
            t.ok = false;
            t.error = format!("{} routes only IPv4, so IPv6 stops there even though IPv4 gets through. Turn IPv6 on for it.", r.name);
        }
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
    /// Where the picture came from (shown, and read by older saves that don't carry it).
    pub path: String,
    /// The picture itself (JPEG, base64), so a plan sent to someone else keeps its floor plan.
    pub data: String,
    /// Top-left corner on the map (px) and real width (m).
    pub pos: [f32; 2],
    pub width_m: f32,
    pub opacity: f32,
}

impl FloorPlan {
    /// A picture file made small enough to keep in the plan: at most 2400 px wide, JPEG, base64.
    pub fn embed(bytes: &[u8]) -> Result<String, String> {
        use base64::Engine;
        let mut img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        if img.width() > 2400 {
            img = img.resize(2400, u32::MAX, image::imageops::FilterType::Triangle);
        }
        let mut out = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img.to_rgb8()).map_err(|e| e.to_string())?;
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    /// The picture's file contents: kept in the plan, or read from where it was picked.
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        use base64::Engine;
        if self.data.is_empty() {
            std::fs::read(&self.path).map_err(|e| e.to_string())
        } else {
            base64::engine::general_purpose::STANDARD.decode(&self.data).map_err(|e| e.to_string())
        }
    }

    /// Names the loaded picture in the texture cache.
    pub fn key(&self) -> String {
        format!("{}#{}#{}", self.floor, self.path, self.data.len())
    }
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
    fn a_floor_plan_picture_travels_inside_the_plan() {
        let mut png = vec![];
        image::RgbImage::from_pixel(3000, 1000, image::Rgb([200, 30, 30])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let p = FloorPlan { data: FloorPlan::embed(&png).unwrap(), path: "/gone/plan.png".into(), ..Default::default() };
        let img = image::load_from_memory(&p.bytes().unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (2400, 800), "made smaller, same shape");
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
        // Downloads fill the line, and no more.
        let u = n.utilisation(&busy, inet);
        assert!((0.95..1.05).contains(&u), "{u}");
        let pc = id(&n, "Office PC");
        let path = n.send(pc, n.dev(pc).unwrap().first_ip().unwrap(), crate::net::site_ip(0), "TCP", 443, None).path;
        assert!(n.queue_ms(&busy, &path) > n.queue_ms(&quiet, &path) + 20.0);
        assert!(n.check().iter().any(|i| i.text.contains("busy hour")), "{:?}", n.check());
    }

    #[test]
    fn high_priority_goes_first_on_a_full_line() {
        let mut n = template_seeded("home", 3);
        for d in n.devices.iter_mut().filter(|d| d.kind.is_host()) {
            d.uses.push(Use::Download);
        }
        let pc = id(&n, "Office PC");
        let path = n.send(pc, n.dev(pc).unwrap().first_ip().unwrap(), crate::net::site_ip(0), "TCP", 443, None).path;
        let normal = n.queue_ms(&n.loads(), &path);
        let speed = |n: &mut Network| n.exec(pc, "speedtest").lines.iter().find_map(|l| l.trim().strip_prefix("Download ≈ ").and_then(|r| r.split(' ').next()?.parse::<f32>().ok())).unwrap();
        let before = speed(&mut n);
        n.dev_mut(pc).unwrap().qos = Qos::High;
        assert!(n.queue_ms(&n.loads(), &path) < normal / 3.0, "{normal}");
        assert!(speed(&mut n) > before * 1.5, "{before}");
        assert!(n.exec(pc, "speedtest").lines.join(" ").contains("priority"));
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
        // Two 18 W access points reserve 30 W each: 36 W of real draw doesn't fit a 50 W budget.
        let ap2 = n.add_device(Kind::AccessPoint, [120.0, 0.0]);
        n.connect(sw, ap2, Medium::Cat6).unwrap();
        n.dev_mut(sw).unwrap().poe_budget = 50.0;
        assert!(n.poe_problems().iter().any(|p| p.1.contains("budget")));
        assert_eq!((poe_reserve(6.5), poe_reserve(60.0), poe_reserve(80.0)), (Some(15.4), Some(90.0), None));
    }

    #[test]
    fn a_router_behind_a_router_gets_a_slice_of_the_providers_ipv6() {
        let mut n = template_seeded("home", 2);
        let (r1, sw) = (id(&n, "Home router"), id(&n, "Switch 1"));
        let r2 = n.add_device(Kind::HomeRouter, [700.0, 700.0]);
        let l = n.connect(sw, r2, Medium::Cat6).unwrap();
        n.set_wan(r2, l, true);
        let (p1, p2) = (n.v6_prefix(n.dev(r1).unwrap()).unwrap(), n.v6_prefix(n.dev(r2).unwrap()).unwrap());
        // Same /56 (all but the last byte of the fourth group), different /64.
        let site = |p: &str| p.rsplit_once(':').map(|(head, g)| (head.to_string(), u16::from_str_radix(g, 16).unwrap() >> 8)).unwrap();
        assert!(p1 != p2 && site(&p1) == site(&p2), "{p1} {p2}");
        assert!(n.exec(r1, "ipconfig").lines.join(" ").contains("/56"));
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
    fn port_ranges_protocols_schedules_upnp_and_ad_blocking() {
        let mut n = template_seeded("home", 5);
        let (r, pc, nas) = (id(&n, "Home router"), id(&n, "Office PC"), id(&n, "NAS 1"));
        let nas_ip = n.dev(nas).unwrap().first_ip().unwrap();
        let wan = n.dev(r).unwrap().ifaces[1].addr().unwrap().0;
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        let from = n.dev(inet).unwrap().first_ip().unwrap();
        let reach = |n: &Network, port: u16, proto: &'static str| n.send(inet, from, wan, proto, port, None).dst_dev;
        n.dev_mut(r).unwrap().rules = format!("forward 50000-50100 {nas_ip} udp");
        assert_eq!(reach(&n, 50050, "UDP"), Some(nas));
        assert_eq!(reach(&n, 50050, "TCP"), None);
        assert_eq!(reach(&n, 50101, "UDP"), None);
        // UPnP: the game opens its own port.
        n.dev_mut(r).unwrap().rules.clear();
        n.dev_mut(pc).unwrap().uses.push(Use::Gaming);
        assert_eq!(reach(&n, 27015, "UDP"), None);
        n.dev_mut(r).unwrap().upnp = true;
        // A PC plays through Steam, so it asks for Steam's ports and not the Xbox one.
        assert_eq!(reach(&n, 27015, "UDP"), Some(pc));
        assert_eq!(reach(&n, 3074, "UDP"), None);
        assert!(n.check().iter().any(|i| i.text.contains("UPnP")));
        // Bedtime for the office PC: no internet from 22:00 to 06:30.
        n.dev_mut(r).unwrap().rules = "deny office-pc internet 22:00-06:30".into();
        n.hour = 23.0;
        assert!(n.exec(pc, "ping example.com").lines.join(" ").contains("firewall"));
        n.hour = 12.0;
        assert!(n.exec(pc, "ping example.com").lines.join(" ").contains(", 4 received"));
        // The ad blocker answers 0.0.0.0 for tracker names, and only for those.
        n.dev_mut(r).unwrap().adblock = true;
        assert!(n.exec(pc, "nslookup ads.doubleclick.net").lines.join(" ").contains("0.0.0.0"));
        assert!(!n.exec(pc, "nslookup example.com").lines.join(" ").contains("0.0.0.0"));
    }

    #[test]
    fn remote_access_needs_a_reachable_address() {
        let mut n = template_seeded("home", 1);
        let r = id(&n, "Home router");
        assert!(n.remote_access(n.dev(r).unwrap()).is_none());
        n.dev_mut(r).unwrap().vpn = true;
        assert!(n.remote_access(n.dev(r).unwrap()).unwrap().0);
        // On the mobile line: carrier-grade NAT; only IPv6 gets in.
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        n.links.iter_mut().find(|l| [l.a, l.b].contains(&inet) && [l.a, l.b].contains(&r)).unwrap().medium = Medium::Cellular;
        assert!(n.remote_access(n.dev(r).unwrap()).unwrap().1.contains("IPv6 only"));
        n.dev_mut(r).unwrap().ipv6 = false;
        assert!(!n.remote_access(n.dev(r).unwrap()).unwrap().0);
        assert!(n.check().iter().any(|i| i.text.contains("Remote access")));
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

    #[test]
    fn upnp_opens_the_ports_each_game_asks_for() {
        let mut n = crate::net::template("home");
        let r = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        n.dev_mut(r).unwrap().upnp = true;
        let xbox = n.add_device(Kind::Console, [300.0, 300.0]);
        n.dev_mut(xbox).unwrap().name = "Xbox".into();
        let pc = n.add_device(Kind::Desktop, [320.0, 300.0]);
        n.dev_mut(pc).unwrap().uses = vec![Use::Gaming];
        n.run_dhcp();
        let ip = |n: &Network, id| n.dev(id).unwrap().first_ip();
        let rd = n.dev(r).unwrap().clone();
        assert_eq!(n.forward_of(&rd, 3074, "UDP"), ip(&n, xbox));
        assert_eq!(n.forward_of(&rd, 27020, "UDP"), ip(&n, pc));
        assert_eq!(n.forward_of(&rd, 3479, "UDP"), None, "nobody here plays on PlayStation");
        assert_eq!(n.forward_of(&rd, 445, "TCP"), None);
    }

    #[test]
    fn ipv6_needs_every_router_on_the_way_and_isnt_translated() {
        let mut n = crate::net::template("classroom");
        let r = n.devices.iter().find(|d| d.kind == Kind::Router).unwrap().id;
        let pc = n.devices.iter().find(|d| d.kind == Kind::Desktop).unwrap().id;
        let srv = n.devices.iter().find(|d| d.kind == Kind::Server).unwrap().id;
        let path = n.send(pc, n.dev(pc).unwrap().first_ip().unwrap(), n.dev(srv).unwrap().first_ip().unwrap(), "ICMP", 0, None).path;
        assert!(path.contains(&r), "the classroom path crosses the router");
        n.dev_mut(r).unwrap().ipv6 = false;
        assert_eq!(n.v4_only_router(&path).map(|d| d.id), Some(r), "IPv4 gets through, IPv6 stops at the plain router");
        n.dev_mut(r).unwrap().ipv6 = true;
        assert!(n.v4_only_router(&path).is_none());
        // At home with IPv6 on: ping6 reaches the internet and nothing is translated on the way.
        let mut h = crate::net::template("home");
        let hr = h.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        h.dev_mut(hr).unwrap().ipv6 = true;
        let office = h.devices.iter().find(|d| d.name == "Office PC").unwrap().id;
        let o = h.exec(office, "ping6 example.com");
        assert!(o.lines.join(" ").contains("4 received") && !o.traces.iter().flat_map(|t| &t.log).any(|l| l.proto == "NAT"), "{:?}", o.lines);
    }
}