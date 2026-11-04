//! Network model, address planning, problem checks and the packet simulation.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::net::Ipv4Addr;

/// Canvas pixels per metre, used for cable lengths and Wi-Fi range.
pub const PX_PER_M: f32 = 20.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Kind {
    Internet,
    HomeRouter,
    Router,
    Switch,
    AccessPoint,
    Desktop,
    Laptop,
    Phone,
    Server,
    Nas,
    Printer,
    Tv,
    SmartHome,
    Extender,
    Speaker,
    Neighbor,
    Camera,
    Console,
    Interference,
}

impl Kind {
    pub const SHELF: [Kind; 19] = [
        Kind::Internet,
        Kind::HomeRouter,
        Kind::Router,
        Kind::Switch,
        Kind::AccessPoint,
        Kind::Desktop,
        Kind::Laptop,
        Kind::Phone,
        Kind::Server,
        Kind::Nas,
        Kind::Printer,
        Kind::Tv,
        Kind::SmartHome,
        Kind::Extender,
        Kind::Speaker,
        Kind::Camera,
        Kind::Console,
        Kind::Neighbor,
        Kind::Interference,
    ];
    pub fn label(self) -> &'static str {
        use Kind::*;
        match self {
            Internet => "Internet",
            HomeRouter => "Home router",
            Router => "Router",
            Switch => "Switch",
            AccessPoint => "Access point",
            Desktop => "Desktop PC",
            Laptop => "Laptop",
            Phone => "Phone",
            Server => "Server",
            Nas => "NAS",
            Printer => "Printer",
            Tv => "Smart TV",
            SmartHome => "Smart home",
            Extender => "Range extender",
            Speaker => "Mesh speaker",
            Neighbor => "Neighbour Wi-Fi",
            Camera => "Camera",
            Console => "Game console",
            Interference => "Interference",
        }
    }
    pub fn blurb(self) -> &'static str {
        use Kind::*;
        match self {
            Internet => "Your internet provider (fibre, cable, DSL or 5G). Hands your router a public address and resolves names.",
            HomeRouter => "All-in-one box: router + switch + Wi-Fi + DHCP + DNS + firewall/NAT. The heart of most homes.",
            Router => "Connects separate networks and forwards packets between them using its routing table.",
            Switch => "Connects wired devices in one network. Learns which device sits on which port (hubs are obsolete).",
            AccessPoint => "Adds Wi-Fi coverage. Best connected to the router with a cable (wired backhaul) or as a mesh node.",
            Desktop => "A wired computer. Can run a web server, file sharing and more.",
            Laptop => "A portable computer, usually on Wi-Fi.",
            Phone => "Phone or tablet on Wi-Fi.",
            Server => "Always-on machine for websites, DNS, DHCP or other services.",
            Nas => "Network storage for backups, photos and media (file sharing over SMB 3).",
            Printer => "Network printer (IPP / AirPrint / Mopria driverless printing).",
            Tv => "Streaming device. 4K streams need roughly 15–25 Mbit/s.",
            SmartHome => "Lights, plugs, sensors (often Matter over Wi-Fi or Thread). Consider a separate guest/IoT network.",
            Extender => "Repeats Wi-Fi it receives over the air. Every packet crosses the air twice on the same channel, so speed is at best halved.",
            Speaker => "Multi-room speaker that builds its own wireless mesh and runs spanning tree (like Sonos with SonosNet). Wire one and the others mesh to it.",
            Neighbor => "A network you don't own (the flat next door). You can't connect to it, but it shares the air: same or overlapping channels slow you down.",
            Camera => "Security camera. Uploads a constant 2–8 Mbit/s stream; often powered over the cable (PoE).",
            Console => "Game console. Wants low, stable latency more than raw speed — a cable beats Wi-Fi.",
            Interference => "Something that isn't Wi-Fi but uses 2.4 GHz: a microwave oven, an analog baby monitor or video sender, Bluetooth, a Zigbee/Thread hub. It doesn't wait its turn, so nearby 2.4 GHz Wi-Fi suffers.",
        }
    }
    pub fn bridges(self) -> bool {
        matches!(self, Kind::Switch | Kind::AccessPoint | Kind::Extender | Kind::Speaker)
    }
    pub fn routes(self) -> bool {
        matches!(self, Kind::Router | Kind::HomeRouter | Kind::Internet)
    }
    pub fn is_host(self) -> bool {
        !self.bridges() && !self.routes() && !self.foreign()
    }
    /// Provides Wi-Fi to others.
    pub fn radio(self) -> bool {
        matches!(self, Kind::HomeRouter | Kind::AccessPoint | Kind::Extender)
    }
    /// Sends Wi-Fi into the air (own or someone else's).
    pub fn transmits(self) -> bool {
        self.radio() || self.foreign() || self == Kind::Speaker
    }
    /// In the air but not part of your network: the neighbours' Wi-Fi and household disturbers.
    pub fn foreign(self) -> bool {
        matches!(self, Kind::Neighbor | Kind::Interference)
    }
    pub fn wifi_client(self) -> bool {
        use Kind::*;
        matches!(self, Desktop | Laptop | Phone | Printer | Tv | SmartHome | AccessPoint | Extender | Camera | Console | Speaker)
    }
    /// Has a web browser people use.
    pub fn has_browser(self) -> bool {
        matches!(self, Kind::Desktop | Kind::Laptop | Kind::Phone | Kind::Server | Kind::Internet)
    }
    /// Has a command line / network tools (the Internet cloud stands for "a computer out there").
    pub fn has_shell(self) -> bool {
        matches!(self, Kind::Desktop | Kind::Laptop | Kind::Phone | Kind::Server | Kind::Nas | Kind::Router | Kind::HomeRouter | Kind::Internet)
    }
    /// Network gear with ports worth inspecting one by one.
    pub fn has_ports_view(self) -> bool {
        (self.bridges() || self.routes()) && self != Kind::Internet
    }
    /// Which network services this kind of device can realistically run: "dhcp", "dns", "web", "files", "print".
    pub fn offers(self, svc: &str) -> bool {
        use Kind::*;
        match svc {
            "dhcp" => matches!(self, Server | Router | HomeRouter | Internet | Desktop),
            "dns" => matches!(self, Server | Router | HomeRouter | Internet | Desktop | Nas),
            "web" => matches!(self, Desktop | Laptop | Server | Nas | Printer),
            "files" => matches!(self, Desktop | Laptop | Server | Nas),
            "print" => self == Printer,
            _ => false,
        }
    }
    pub fn serves(self) -> bool {
        ["dhcp", "dns", "web", "files", "print"].iter().any(|s| self.offers(s))
    }
    /// Usually powered over its Ethernet cable.
    pub fn poe_powered(self) -> bool {
        matches!(self, Kind::AccessPoint | Kind::Camera)
    }
    /// Busy-hour activities that make sense on this kind of device.
    pub fn uses(self) -> &'static [crate::extras::Use] {
        use crate::extras::Use::*;
        match self {
            Kind::Desktop | Kind::Laptop => &[Browsing, VideoCall, StreamHd, Stream4k, Gaming, Backup, Download, Music],
            Kind::Phone => &[Browsing, VideoCall, StreamHd, Music, Gaming, Download, Backup],
            Kind::Tv => &[StreamHd, Stream4k, Music, Download],
            Kind::Console => &[Gaming, Download, StreamHd, Stream4k],
            Kind::Camera => &[Camera],
            Kind::Speaker => &[Music],
            _ => &[],
        }
    }
    /// Runs spanning tree by default.
    pub fn stp_default(self) -> bool {
        matches!(self, Kind::Switch | Kind::Speaker)
    }
    /// Rough 2026 street prices; editable per device.
    pub fn price(self) -> f32 {
        use Kind::*;
        match self {
            Internet => 0.0,
            HomeRouter => 180.0,
            Router => 250.0,
            Switch => 60.0,
            AccessPoint => 130.0,
            Desktop => 800.0,
            Laptop => 900.0,
            Phone => 500.0,
            Server => 1200.0,
            Nas => 450.0,
            Printer => 200.0,
            Tv => 600.0,
            SmartHome => 35.0,
            Extender => 60.0,
            Speaker => 450.0,
            Neighbor => 0.0,
            Camera => 120.0,
            Console => 500.0,
            Interference => 0.0,
        }
    }
    fn base_name(self) -> &'static str {
        use Kind::*;
        match self {
            Internet => "Internet",
            HomeRouter => "Home router",
            Router => "Router",
            Switch => "Switch",
            AccessPoint => "Access point",
            Desktop => "PC",
            Laptop => "Laptop",
            Phone => "Phone",
            Server => "Server",
            Nas => "NAS",
            Printer => "Printer",
            Tv => "TV",
            SmartHome => "Smart plug",
            Extender => "Extender",
            Speaker => "Speaker",
            Neighbor => "Neighbour",
            Camera => "Camera",
            Console => "Console",
            Interference => "Microwave",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Medium {
    Cat5e,
    Cat6,
    Cat6a,
    Fiber,
    WiFi,
    Cat5,
    Powerline,
    Mesh,
    Cellular,
}

impl Medium {
    pub const ALL: [Medium; 9] = [Medium::Cat6, Medium::Cat6a, Medium::Cat5e, Medium::Cat5, Medium::Fiber, Medium::Powerline, Medium::WiFi, Medium::Mesh, Medium::Cellular];
    pub fn label(self) -> &'static str {
        match self {
            Medium::Cat5e => "Cat 5e cable",
            Medium::Cat6 => "Cat 6 cable",
            Medium::Cat6a => "Cat 6A cable",
            Medium::Fiber => "Fibre",
            Medium::WiFi => "Wi-Fi",
            Medium::Cat5 => "Old Cat 5 cable",
            Medium::Powerline => "Powerline adapters",
            Medium::Mesh => "Speaker mesh (wireless)",
            Medium::Cellular => "Mobile 5G backup line",
        }
    }
    pub fn speed(self) -> &'static str {
        match self {
            Medium::Cat5e => "1–2.5 Gbit/s",
            Medium::Cat6 => "up to 10 Gbit/s (≤ 55 m)",
            Medium::Cat6a => "10 Gbit/s (≤ 100 m)",
            Medium::Fiber => "10+ Gbit/s, long distances",
            Medium::WiFi => "depends on signal, walls and neighbours",
            Medium::Cat5 => "1 Gbit/s at most — nothing faster (2.5G needs Cat 5e or better)",
            Medium::Powerline => "speed depends on the house wiring, distance and running appliances",
            Medium::Mesh => "2.4 GHz speaker radio link: speed from its signal, shared with every speaker",
            Medium::Cellular => "≈ 300 Mbit/s down, 50 up, higher delay, no port forwarding (shared address); only used when the main internet line fails",
        }
    }
    pub fn price_per_m(self) -> f32 {
        match self {
            Medium::Cat5e | Medium::Cat5 => 0.5,
            Medium::Cat6 => 0.8,
            Medium::Cat6a => 1.5,
            Medium::Fiber => 2.0,
            Medium::WiFi | Medium::Mesh => 0.0,
            Medium::Powerline | Medium::Cellular => 0.0,
        }
    }
    /// Hardware needed per link regardless of length (a powerline adapter pair).
    pub fn per_link_price(self) -> f32 {
        match self {
            Medium::Powerline => 70.0,
            Medium::Cellular => 150.0,
            _ => 2.0,
        }
    }
    /// A real Ethernet cable that takes a port at both ends.
    pub fn cable(self) -> bool {
        !self.wireless() && !matches!(self, Medium::Powerline | Medium::Cellular)
    }
    pub fn wireless(self) -> bool {
        matches!(self, Medium::WiFi | Medium::Mesh)
    }
    pub fn latency_ms(self) -> f32 {
        match self {
            Medium::WiFi => 2.4,
            Medium::Mesh => 6.0,
            Medium::Powerline => 3.0,
            Medium::Cellular => 28.0,
            _ => 0.15,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum WifiStd {
    Wifi5,
    Wifi6,
    Wifi6E,
    Wifi7,
}

impl WifiStd {
    pub const ALL: [WifiStd; 4] = [WifiStd::Wifi7, WifiStd::Wifi6E, WifiStd::Wifi6, WifiStd::Wifi5];
    pub fn label(self) -> &'static str {
        match self {
            WifiStd::Wifi5 => "Wi-Fi 5 (802.11ac, 5 GHz)",
            WifiStd::Wifi6 => "Wi-Fi 6 (802.11ax, 2.4/5 GHz)",
            WifiStd::Wifi6E => "Wi-Fi 6E (adds 6 GHz)",
            WifiStd::Wifi7 => "Wi-Fi 7 (802.11be, MLO, 320 MHz)",
        }
    }
    /// Fastest rate of one antenna stream on a 20 MHz channel (top modulation), Mbit/s.
    pub fn stream20(self, band: usize) -> f32 {
        match (self, band) {
            (WifiStd::Wifi5, 0) => 72.2,
            (WifiStd::Wifi5, _) => 86.7,
            (WifiStd::Wifi6 | WifiStd::Wifi6E, _) => 143.4,
            (WifiStd::Wifi7, _) => 172.1,
        }
    }
    /// Widest channel this standard allows on a band (0 = band not supported).
    pub fn max_width(self, band: usize) -> u16 {
        match (self, band) {
            (_, 0) => 40,
            (WifiStd::Wifi5 | WifiStd::Wifi6 | WifiStd::Wifi6E | WifiStd::Wifi7, 1) => 160,
            (WifiStd::Wifi6E, 2) => 160,
            (WifiStd::Wifi7, 2) => 320,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Security {
    Open,
    Wep,
    Wpa2,
    Wpa3,
    Wpa3Ent,
}

impl Security {
    pub const ALL: [Security; 5] = [Security::Wpa3, Security::Wpa3Ent, Security::Wpa2, Security::Wep, Security::Open];
    pub fn label(self) -> &'static str {
        match self {
            Security::Open => "Open (no password)",
            Security::Wep => "WEP (broken)",
            Security::Wpa2 => "WPA2",
            Security::Wpa3 => "WPA3 (recommended)",
            Security::Wpa3Ent => "WPA3-Enterprise (802.1X: own login per person)",
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Services {
    pub web: bool,
    pub dns: bool,
    pub dhcp: bool,
    pub files: bool,
    pub print: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Iface {
    pub name: String,
    pub ip: String,
    pub prefix: u8,
    pub mac: String,
    /// VLAN this address lives in (1 = the default, untagged network).
    pub vlan: u16,
    /// For a VLAN sub-interface: the physical port it rides on.
    pub parent: Option<usize>,
}

impl Default for Iface {
    fn default() -> Self {
        Iface { name: "eth0".into(), ip: String::new(), prefix: 24, mac: String::new(), vlan: 1, parent: None }
    }
}

impl Iface {
    pub fn addr(&self) -> Option<(Ipv4Addr, u8)> {
        Some((parse_ip(&self.ip)?, self.prefix.min(32)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Device {
    pub id: u32,
    pub kind: Kind,
    pub name: String,
    pub pos: [f32; 2],
    pub ifaces: Vec<Iface>,
    pub gateway: String,
    pub dns: String,
    pub dhcp_client: bool,
    pub services: Services,
    pub dhcp_from: String,
    pub dhcp_to: String,
    pub routes: String,
    pub dns_records: String,
    pub web_page: String,
    /// Extra pages by file name; `web_page` is index.html.
    pub web_files: Vec<(String, String)>,
    pub ssid: String,
    pub wifi: WifiStd,
    pub security: Security,
    pub price: f32,
    pub notes: String,
    /// Spanning tree on/off and bridge priority (lower wins the root election).
    pub stp: bool,
    pub priority: u16,
    /// Radio, per band [2.4 GHz, 5 GHz, 6 GHz]: on/off, channel, width (MHz), transmit power (dBm).
    pub bands: [bool; 3],
    pub ch: [u16; 3],
    pub width: [u16; 3],
    pub tx: [f32; 3],
    /// Antenna streams (MIMO 1–4).
    pub streams: u8,
    /// Steer clients to the faster band instead of the strongest one.
    pub band_steer: bool,
    /// A separate radio for the link back to the router (tri-band mesh, cross-band extender).
    pub backhaul: bool,
    /// Hardware preset name, for the plan.
    pub model: String,
    /// 802.11k/v/r: helps phones roam quickly and to the right access point.
    pub fast_roam: bool,
    /// Kick clients weaker than this (dBm, 0 = off).
    pub min_rssi: i16,
    /// Neighbour networks: share of airtime they use (0–1).
    pub busy: f32,
    /// Internet: the plan's download speed in Mbit/s.
    pub plan_mbps: f32,
    /// Ethernet, fibre and internet-line sockets, as groups of identical ports.
    #[serde(rename = "port_groups")]
    pub ports: Vec<crate::ports::PortGroup>,
    /// Plans saved before port groups had a count and one speed; sanitize turns them into groups.
    #[serde(rename = "ports", skip_serializing)]
    pub(crate) old_ports: u8,
    #[serde(rename = "port_mbps", skip_serializing)]
    pub(crate) old_mbps: u32,
    /// Storey: 0 = ground floor, 1 = first floor, -1 = basement.
    pub floor: i8,
    /// Wi-Fi clients: signal (dBm) at which this device starts looking for a better access point.
    pub roam_at: i16,
    /// Spanning tree: uses the old short path-cost scale (802.1D-1998: 1 Gbit/s = 4) instead of the
    /// long one (802.1D-2004: 1 Gbit/s = 20000). Mixing both makes paths through the short-scale device look cheap.
    pub stp_short: bool,
    /// What people do on this device at the busiest hour of the day.
    pub uses: Vec<crate::extras::Use>,
    /// Power over Ethernet: per-port standard it supplies (index into extras::POE), total budget (W),
    /// and what this device draws over its cable (W, 0 = own power supply).
    pub poe_class: u8,
    pub poe_budget: f32,
    pub poe_draw: f32,
    /// Calibration from real measurements (dB added to this radio's signal).
    pub cal_db: f32,
    /// Router firewall: lines like "deny guest lan", "allow any 192.168.1.10 443", "forward 443 192.168.1.10".
    pub rules: String,
    /// Home router: hands out IPv6 to its network.
    pub ipv6: bool,
    /// Home router: smart queue management (SQM) on the internet link — keeps delay low when it's full.
    pub sqm: bool,
    /// Guest Wi-Fi name (the guest network itself is a router interface in VLAN 4000).
    pub guest_ssid: String,
    /// Extra Wi-Fi names this radio broadcasts, each leading into its own VLAN.
    pub ssids: Vec<(String, u16)>,
    /// Wi-Fi clients: the network name they joined (empty = the access point's main name).
    pub join: String,
    /// Switched off (to try "what if this device fails?").
    pub off: bool,
    /// Electrical power (W): while in use for TVs, computers and consoles (they sleep the rest of the day,
    /// see `more::duty`), around the clock for everything else.
    pub watts: f32,
    /// Antenna: beam width in degrees (0 = all around) and where it points (degrees, 0 = right, 90 = down).
    pub beam: f32,
    pub aim: f32,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            id: 0,
            kind: Kind::Desktop,
            name: String::new(),
            pos: [0.0, 0.0],
            ifaces: vec![],
            gateway: String::new(),
            dns: String::new(),
            dhcp_client: false,
            services: Services::default(),
            dhcp_from: String::new(),
            dhcp_to: String::new(),
            routes: String::new(),
            dns_records: String::new(),
            web_page: String::new(),
            web_files: vec![],
            ssid: String::new(),
            wifi: WifiStd::Wifi6,
            security: Security::Wpa3,
            price: 0.0,
            notes: String::new(),
            stp: false,
            priority: 32768,
            bands: [true, true, false],
            ch: [6, 36, 37],
            width: [20, 80, 160],
            tx: [20.0, 23.0, 23.0],
            streams: 2,
            band_steer: true,
            backhaul: false,
            model: String::new(),
            fast_roam: false,
            min_rssi: 0,
            busy: 0.3,
            plan_mbps: 500.0,
            ports: vec![crate::ports::PortGroup::rj45(1, 1000)],
            old_ports: 0,
            old_mbps: 0,
            floor: 0,
            roam_at: -75,
            stp_short: false,
            uses: vec![],
            poe_class: 0,
            poe_budget: 0.0,
            poe_draw: 0.0,
            cal_db: 0.0,
            rules: String::new(),
            ipv6: true,
            sqm: false,
            guest_ssid: String::new(),
            ssids: vec![],
            join: String::new(),
            off: false,
            watts: 0.0,
            beam: 0.0,
            aim: 0.0,
        }
    }
}

impl Device {
    /// The interface DHCP configures: WAN on a home router, the only one elsewhere.
    pub fn dhcp_iface(&self) -> usize {
        if self.kind == Kind::HomeRouter { 1 } else { 0 }
    }
    pub fn hostname(&self) -> String {
        let s: String = self
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        s.trim_matches('-').to_string()
    }
    pub fn first_ip(&self) -> Option<Ipv4Addr> {
        self.ifaces.iter().find_map(|f| f.addr().map(|a| a.0))
    }
    pub fn has_ip(&self, ip: Ipv4Addr) -> bool {
        self.ifaces.iter().any(|f| f.addr().map(|a| a.0) == Some(ip))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub id: u32,
    pub a: u32,
    pub a_if: usize,
    pub b: u32,
    pub b_if: usize,
    pub medium: Medium,
    /// None = measured from the drawing.
    #[serde(default)]
    pub length_m: Option<f32>,
    /// Pulled out (to try "what if this cable breaks?").
    #[serde(default)]
    pub unplugged: bool,
    /// Port number (0-based) the cable is plugged into at each end; None = wireless or no port free.
    #[serde(default)]
    pub a_port: Option<u16>,
    #[serde(default)]
    pub b_port: Option<u16>,
    /// Powerline links: the adapter kit (catalog::powerline), empty = a typical one.
    #[serde(default)]
    pub kit: String,
}

impl Link {
    pub fn other(&self, id: u32) -> (u32, usize) {
        if self.a == id { (self.b, self.b_if) } else { (self.a, self.a_if) }
    }
    pub fn end_if(&self, id: u32) -> usize {
        if self.a == id { self.a_if } else { self.b_if }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Level {
    Error,
    Warning,
    Tip,
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub level: Level,
    pub dev: Option<u32>,
    pub link: Option<u32>,
    pub text: String,
    pub fix: Option<crate::issues::Fix>,
}

#[derive(Clone, Debug)]
pub struct Log {
    pub src: String,
    pub dst: String,
    pub proto: &'static str,
    pub layer: &'static str,
    pub info: String,
    pub dev: u32,
}

/// One packet's journey, for the animation.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    pub path: Vec<u32>,
    pub l3: Vec<(u32, Ipv4Addr)>,
    pub ok: bool,
    pub error: String,
    pub log: Vec<Log>,
    pub dst_dev: Option<u32>,
    pub site: Option<usize>,
    /// Reached a real website out on the internet.
    pub real: bool,
    pub ms: f32,
    /// Source address as the destination sees it (after NAT).
    pub seen_src: Option<Ipv4Addr>,
    pub nat: Option<(u32, Ipv4Addr)>,
    /// (device, address it now knows the MAC of) from the ARP questions on the way.
    pub arp: Vec<(u32, Ipv4Addr)>,
}

#[derive(Default)]
pub struct Outcome {
    pub lines: Vec<String>,
    pub traces: Vec<Trace>,
    pub log: Vec<Log>,
    /// A real web page to fetch: address and the round trip it takes over the simulated network (ms).
    pub fetch: Option<(String, f32)>,
}

/// Websites that live "on the internet" in the simulation (documentation address range).
/// (name, address, page text, one-way delay beyond your provider in ms — distance and the internet in between)
pub const SITES: [(&str, [u8; 4], &str, f32); 4] = [
    ("example.com", [203, 0, 113, 10], "Example Domain — this page is for use in examples. Served over HTTPS (TLS 1.3).", 5.0),
    ("learn.example.org", [203, 0, 113, 20], "Network lessons — a sample learning site. It answers over HTTP/3 (QUIC) and HTTP/2.", 12.0),
    ("video.example.net", [203, 0, 113, 30], "Streaming test — a 4K stream needs about 15–25 Mbit/s, HD about 5 Mbit/s. Served from a cache inside your provider.", 2.0),
    ("far.example.net", [203, 0, 113, 40], "A server on another continent: light in fibre needs ~70 ms each way, so every round trip takes ~150 ms.", 75.0),
];

/// Real websites: their names resolve to an address in 198.18.0.0/15 (reserved for network testing),
/// the packets travel to the internet device, and the browser then fetches the real page.
pub fn real_ip(name: &str) -> Ipv4Addr {
    let h = name.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    Ipv4Addr::new(198, 18 + (h & 1) as u8, (h >> 8) as u8, 1 + (h >> 16) as u8 % 254)
}

pub fn is_real_ip(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 198 && (o[1] == 18 || o[1] == 19)
}

/// One-way delay from your provider to a typical real website (they mostly sit in nearby data centres).
pub const REAL_MS: f32 = 10.0;

/// A name that belongs on the public internet (not a home, test or example name).
pub fn is_public_name(name: &str) -> bool {
    let local = [".test", ".home.arpa", ".local", ".lan", ".internal", ".localhost", ".invalid", ".example"];
    name.contains('.') && parse_ip(name).is_none() && !local.iter().any(|l| name.ends_with(l)) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

pub fn site_ip(i: usize) -> Ipv4Addr {
    let o = SITES[i].1;
    Ipv4Addr::new(o[0], o[1], o[2], o[3])
}

pub fn parse_ip(s: &str) -> Option<Ipv4Addr> {
    s.trim().parse().ok()
}

fn mask(p: u8) -> u32 {
    if p == 0 { 0 } else { u32::MAX << (32 - p.min(32) as u32) }
}

pub fn same_net(a: Ipv4Addr, b: Ipv4Addr, p: u8) -> bool {
    u32::from(a) & mask(p) == u32::from(b) & mask(p)
}

pub fn net_of(a: Ipv4Addr, p: u8) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(a) & mask(p))
}

/// The .netplan format this build writes; `sanitize` brings older files up to it.
pub const FORMAT: u32 = 1;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    /// Save format version (0 = a file from before versions were written).
    #[serde(default)]
    pub version: u32,
    pub title: String,
    pub currency: String,
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
    pub next_id: u32,
    pub walls: Vec<crate::wifi::Wall>,
    /// What this scenario is about, shown in the overview, and its key in scenarios::SCENARIOS ("" for your own plans).
    pub story: String,
    pub scenario: String,
    /// Makes this plan's "random" hardware (MAC addresses, neighbours' habits, house wiring) reproducible.
    pub seed: u64,
    /// What the floors between storeys are made of.
    pub slab: crate::wifi::WallKind,
    /// Pictures of each storey's floor plan, real Wi-Fi readings, and the device models used in this plan.
    pub plans: Vec<crate::extras::FloorPlan>,
    pub survey: Vec<crate::extras::Sample>,
    pub models: Vec<crate::extras::Model>,
    /// Measuring points from the companion app, and which measured access point is which device.
    pub points: Vec<crate::companion::MeasurePoint>,
    pub measured: Vec<(String, u32)>,
    /// Room names written on the map, and names people gave their storeys ("Loft").
    pub labels: Vec<RoomLabel>,
    pub storey_names: Vec<(i8, String)>,
    /// Garden, terrace or yard: places outside the walls where Wi-Fi should reach too.
    pub outdoor: Vec<crate::plan::Outdoor>,
    /// Electricity price per kWh, for the yearly running cost.
    pub kwh_price: f32,
    /// Time of day (hours) the traffic is looked at; 20.5 = the evening busy hour.
    pub hour: f32,
    /// Simulation time (s) and whether live variation (fading, traffic) is on; not saved.
    #[serde(skip)]
    pub clock: f64,
    #[serde(skip)]
    pub live: bool,
    /// Addresses each device learned a MAC address for from traffic (its ARP cache); not saved.
    #[serde(skip)]
    pub arp_seen: HashMap<u32, Vec<Ipv4Addr>>,
}

impl Default for Network {
    fn default() -> Self {
        Network {
            version: FORMAT,
            title: String::new(),
            currency: String::new(),
            devices: vec![],
            links: vec![],
            next_id: 0,
            walls: vec![],
            story: String::new(),
            scenario: String::new(),
            seed: 1,
            slab: crate::wifi::WallKind::Concrete,
            plans: vec![],
            survey: vec![],
            models: vec![],
            points: vec![],
            measured: vec![],
            labels: vec![],
            storey_names: vec![],
            outdoor: vec![],
            kwh_price: 0.30,
            hour: 20.5,
            clock: 0.0,
            live: false,
            arp_seen: HashMap::new(),
        }
    }
}

/// Port speed and port count of typical hardware of each kind.
fn ports_of(kind: Kind) -> Vec<crate::ports::PortGroup> {
    use Kind::*;
    let g = crate::ports::PortGroup::rj45;
    match kind {
        Internet => vec![g(64, 10000)],
        HomeRouter => vec![g(1, 1000).wan(), g(4, 1000)],
        Router | Switch => vec![g(8, 1000)],
        AccessPoint | Nas | Server => vec![g(1, 2500)],
        Extender | Speaker | Tv | Printer | Camera | SmartHome => vec![g(1, 100)],
        Neighbor | Interference => vec![],
        _ => vec![g(1, 1000)],
    }
}

/// Names of storeys.
/// A room name on the map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RoomLabel {
    pub id: u32,
    pub pos: [f32; 2],
    pub floor: i8,
    pub text: String,
}

impl Network {
    /// A storey's own name if someone gave it one, else "Ground floor", "1st floor"…
    pub fn storey(&self, f: i8) -> String {
        self.storey_names.iter().find(|s| s.0 == f && !s.1.trim().is_empty()).map(|s| s.1.clone()).unwrap_or_else(|| floor_name(f))
    }

    pub fn add_label(&mut self, pos: [f32; 2], floor: i8, text: &str) -> u32 {
        self.next_id += 1;
        self.labels.push(RoomLabel { id: self.next_id, pos, floor, text: text.into() });
        self.next_id
    }
}

pub fn floor_name(f: i8) -> String {
    match f {
        0 => "Ground floor".into(),
        1 => "1st floor".into(),
        2 => "2nd floor".into(),
        3 => "3rd floor".into(),
        f if f < 0 => if f == -1 { "Basement".into() } else { format!("Basement {}", -f) },
        f => format!("{f}th floor"),
    }
}

impl Network {
    /// Repairs a plan read from a file so nothing points at devices or ports that don't exist.
    pub fn sanitize(&mut self) {
        for d in self.devices.iter_mut() {
            crate::wifi::normalize_radio(d);
            d.floor = d.floor.clamp(-3, 20);
        }
        // Storeys out of range or repeated ids in a damaged file would trip up the map.
        for w in self.walls.iter_mut() {
            w.floor = w.floor.clamp(-3, 20);
        }
        for p in self.points.iter_mut() {
            p.floor = p.floor.clamp(-3, 20);
        }
        self.outdoor.retain(|o| o.a.iter().chain(&o.b).all(|v| v.is_finite()));
        for o in self.outdoor.iter_mut() {
            o.floor = o.floor.clamp(-3, 20);
        }
        // Ids near the top of u32 would make next_id overflow; a real plan never gets there.
        const MAX_ID: u32 = 1 << 30;
        self.links.retain(|l| l.id < MAX_ID);
        self.walls.retain(|w| w.id < MAX_ID);
        self.points.retain(|p| p.id < MAX_ID);
        self.labels.retain(|l| l.id < MAX_ID);
        let mut seen = std::collections::HashSet::new();
        self.devices.retain(|d| d.id < MAX_ID && seen.insert(d.id));
        self.next_id = self.next_id.max(self.devices.iter().map(|d| d.id).max().unwrap_or(0)).max(self.walls.iter().map(|w| w.id).max().unwrap_or(0))
            .max(self.points.iter().map(|p| p.id).max().unwrap_or(0))
            .max(self.labels.iter().map(|l| l.id).max().unwrap_or(0));
        let ids: std::collections::HashSet<u32> = self.devices.iter().map(|d| d.id).collect();
        self.links.retain(|l| ids.contains(&l.a) && ids.contains(&l.b) && l.a != l.b);
        for d in self.devices.iter_mut() {
            if d.old_ports > 0 {
                let mbps = if d.old_mbps > 0 { d.old_mbps } else { 1000 };
                // The old count left a home router's WAN port out.
                d.ports = if d.kind == Kind::HomeRouter { vec![crate::ports::PortGroup::rj45(1, mbps).wan()] } else { vec![] };
                d.ports.push(crate::ports::PortGroup::rj45(d.old_ports, mbps));
                (d.old_ports, d.old_mbps) = (0, 0);
            }
            d.ports.retain(|g| g.n > 0);
            d.poe_class = d.poe_class.min(4);
            d.streams = d.streams.clamp(1, 4);
            for f in d.ifaces.iter_mut() {
                f.prefix = f.prefix.min(32);
                f.vlan = f.vlan.clamp(1, 4094);
            }
            let n = d.ifaces.len();
            for f in d.ifaces.iter_mut() {
                if f.parent.is_some_and(|p| p >= n) {
                    f.parent = None;
                }
            }
            if d.kind == Kind::HomeRouter && d.ifaces.len() < 2 {
                d.ifaces.resize(2, Iface::default());
            }
            if (d.kind.is_host() || d.kind.bridges() || d.kind == Kind::Internet) && d.ifaces.is_empty() {
                d.ifaces.push(Iface::default());
            }
        }
        let ifs: HashMap<u32, usize> = self.devices.iter().map(|d| (d.id, d.ifaces.len())).collect();
        self.links.retain(|l| l.a_if < ifs[&l.a] && l.b_if < ifs[&l.b]);
        self.next_id = self.next_id.min(MAX_ID).max(self.devices.iter().map(|d| d.id).chain(self.links.iter().map(|l| l.id)).chain(self.walls.iter().map(|w| w.id)).max().unwrap_or(0));
        // A repeated id would make one delete remove several things: give the copies new ids.
        let mut fresh = |ids: &mut dyn Iterator<Item = &mut u32>| {
            let mut seen = std::collections::HashSet::new();
            for id in ids {
                if !seen.insert(*id) {
                    self.next_id += 1;
                    *id = self.next_id;
                }
            }
        };
        fresh(&mut self.links.iter_mut().map(|l| &mut l.id));
        fresh(&mut self.walls.iter_mut().map(|w| &mut w.id));
        fresh(&mut self.points.iter_mut().map(|p| &mut p.id));
        fresh(&mut self.labels.iter_mut().map(|l| &mut l.id));
        fresh(&mut self.outdoor.iter_mut().map(|o| &mut o.id));
        // Migrations above are keyed on what's in the file; from here on it is the current format (fields a newer one added are gone).
        self.version = FORMAT;
        self.assign_ports();
    }

    /// Re-rolls everything that differs between two physical copies of the same plan:
    /// MAC addresses, how sticky each phone is, how busy the neighbours are.
    pub fn shuffle_hardware(&mut self, seed: u64) {
        self.seed = seed;
        for i in 0..self.devices.len() {
            let (id, kind) = (self.devices[i].id, self.devices[i].kind);
            self.randomise(i, id, kind);
        }
    }

    fn randomise(&mut self, i: usize, id: u32, kind: Kind) {
        let seed = self.seed;
        let d = &mut self.devices[i];
        for (p, f) in d.ifaces.iter_mut().enumerate() {
            f.mac = crate::sim::mac(seed, kind, id, p as u32);
        }
        // Clients differ: some hang on to a weak access point much longer than others.
        d.roam_at = -70 - (crate::sim::unit(seed, id as u64, 0x40) * 10.0) as i16;
        if kind == Kind::Neighbor {
            d.busy = 0.05 + 0.6 * crate::sim::unit(seed, id as u64, 0x41).powf(1.5);
        }
    }

    pub fn dev(&self, id: u32) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
    pub fn dev_mut(&mut self, id: u32) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }
    pub fn name(&self, id: u32) -> String {
        self.dev(id).map(|d| d.name.clone()).unwrap_or_default()
    }

    pub fn add_device(&mut self, kind: Kind, pos: [f32; 2]) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        let n = self.devices.iter().filter(|d| d.kind == kind).count() + 1;
        let name = if kind == Kind::Internet || n == 1 && kind == Kind::HomeRouter {
            kind.base_name().to_string()
        } else {
            format!("{} {}", kind.base_name(), n)
        };
        let iface = |n: &str, ip: &str, p: u8, _i: u32| Iface { name: n.into(), ip: ip.into(), prefix: p, ..Default::default() };
        let mut d = Device {
            id,
            kind,
            name,
            pos,
            price: kind.price(),
            stp: kind.stp_default(),
            stp_short: kind == Kind::Speaker,
            ports: ports_of(kind),
            uses: crate::extras::default_uses(kind),
            poe_draw: crate::extras::default_poe_draw(kind),
            watts: crate::more::default_watts(kind),
            ..Default::default()
        };
        match kind {
            Kind::Internet => {
                d.ifaces = vec![iface("isp", "198.51.100.1", 24, 0)];
                d.services = Services { dns: true, dhcp: true, web: true, ..Default::default() };
                d.dhcp_from = "198.51.100.10".into();
                d.dhcp_to = "198.51.100.250".into();
            }
            Kind::HomeRouter => {
                d.ifaces = vec![iface("lan", "192.168.1.1", 24, 0), iface("wan", "", 24, 1)];
                d.dhcp_client = true;
                d.services = Services { dns: true, dhcp: true, ..Default::default() };
                d.dhcp_from = "192.168.1.100".into();
                d.dhcp_to = "192.168.1.199".into();
                d.ssid = "HomeNet".into();
            }
            Kind::Router => {}
            Kind::Switch | Kind::AccessPoint | Kind::Extender | Kind::Speaker => {
                d.ifaces = vec![iface("ports", "", 24, 0)];
                d.ssid = if kind == Kind::Speaker { "(speaker mesh)".into() } else { "HomeNet".into() };
                if kind == Kind::Speaker {
                    // All speakers of one system share the channel the system picked at setup.
                    d.ch[0] = crate::sim::pick(self.seed, 0x5EA, 0, &["1", "6", "11"]).parse().unwrap();
                }
                if kind == Kind::AccessPoint {
                    d.ch = [1, 149, 69];
                }
            }
            Kind::Interference => crate::wifi::apply_disturber(&mut d, 0),
            Kind::Neighbor => {
                // Neighbours: a typical mix of names and factory channels (most routers pick 1/6/11 and 36).
                let (s, k) = (self.seed, id as u64);
                let hex = format!("{:04X}", crate::sim::hash(s, k, 1) & 0xffff);
                d.ssid = crate::sim::pick(s, k, 2, &["WLAN-", "Home_", "Flat-", "Guest-", "DIRECT-Printer-", "NET-"]).to_string() + &hex;
                let c24 = crate::sim::pick(s, k, 3, &["1", "1", "6", "6", "6", "11", "11", "3", "9"]);
                let c5 = crate::sim::pick(s, k, 4, &["36", "36", "36", "44", "52", "100", "116"]);
                d.ch = [c24.parse().unwrap(), c5.parse().unwrap(), 37];
                d.wifi = WifiStd::Wifi6;
            }
            _ => {
                let wifi = matches!(kind, Kind::Laptop | Kind::Phone | Kind::SmartHome);
                d.ifaces = vec![iface(if wifi { "wlan0" } else { "eth0" }, "", 24, 0)];
                // Everything starts on automatic addresses; "Plan addresses" pins servers, NAS and printers.
                d.dhcp_client = true;
                match kind {
                    Kind::Server => {
                        d.services.web = true;
                        d.web_page = format!("Welcome to {}!", d.name);
                    }
                    Kind::Nas => {
                        d.services.files = true;
                        d.services.web = true;
                        d.web_page = "Storage admin page".into();
                    }
                    Kind::Printer => {
                        d.services.print = true;
                        d.services.web = true;
                        d.web_page = "Printer status: ready".into();
                    }
                    _ => {}
                }
            }
        }
        if let Some(p) = crate::wifi::default_preset(kind) {
            p.apply(&mut d);
        }
        // A second home router gets its own network instead of a clash on 192.168.1.1.
        if kind == Kind::HomeRouter {
            let taken: Vec<u8> = self.devices.iter().filter(|o| o.kind == Kind::HomeRouter).filter_map(|o| o.ifaces.first()?.addr()).map(|(ip, _)| ip.octets()[2]).collect();
            if let Some(n) = (1..=254u8).find(|n| !taken.contains(n)) {
                d.ifaces[0].ip = format!("192.168.{n}.1");
                (d.dhcp_from, d.dhcp_to) = (format!("192.168.{n}.100"), format!("192.168.{n}.199"));
            }
        }
        // New access points start on the channels your other radios use least.
        if matches!(kind, Kind::AccessPoint | Kind::HomeRouter) {
            let others: Vec<[u16; 3]> = self.devices.iter().filter(|o| o.kind.radio()).map(|o| o.ch).collect();
            let least = |opts: &[u16], i: usize| *opts.iter().min_by_key(|c| others.iter().filter(|o| o[i] == **c).count()).unwrap();
            d.ch[0] = least(&crate::wifi::CH24[..3], 0);
            d.ch[1] = least(&crate::wifi::Band::B5.channels(d.width[1].max(20)), 1);
            d.ch[2] = least(&crate::wifi::Band::B6.channels(d.width[2].max(20)), 2);
        }
        crate::wifi::normalize_radio(&mut d);
        self.devices.push(d);
        let i = self.devices.len() - 1;
        self.randomise(i, id, kind);
        id
    }

    pub fn connect(&mut self, a: u32, b: u32, medium: Medium) -> Result<u32, String> {
        let id = self.connect_unplugged(a, b, medium)?;
        self.assign_ports();
        Ok(id)
    }

    /// `connect` without choosing ports yet, for adding many cables at once; call `assign_ports` afterwards.
    pub(crate) fn connect_unplugged(&mut self, a: u32, b: u32, medium: Medium) -> Result<u32, String> {
        if a == b {
            return Err("A device can't be connected to itself.".into());
        }
        // A mobile backup line may run next to the main internet line; anything else only once.
        if self.links.iter().any(|l| ((l.a == a && l.b == b) || (l.a == b && l.b == a)) && (medium != Medium::Cellular || l.medium == Medium::Cellular)) {
            return Err("These two are already connected.".into());
        }
        let (ka, kb) = (self.dev(a).ok_or("gone")?.kind, self.dev(b).ok_or("gone")?.kind);
        if ka == Kind::Neighbor || kb == Kind::Neighbor {
            return Err("That's the neighbours' network — it only shares the air with yours. Place it to see its interference.".into());
        }
        if ka == Kind::Interference || kb == Kind::Interference {
            return Err("Interference sources aren't part of the network — place them where they are to see what they disturb.".into());
        }
        if medium == Medium::Mesh && (ka != Kind::Speaker || kb != Kind::Speaker) {
            return Err("Only mesh speakers talk over the speaker mesh. Use a cable or Wi-Fi.".into());
        }
        if medium != Medium::WiFi && [ka, kb].iter().any(|k| matches!(k, Kind::Phone | Kind::SmartHome)) {
            return Err("Phones and smart-home gadgets have no Ethernet socket — connect them by Wi-Fi.".into());
        }
        for (k, id) in [(ka, a), (kb, b)] {
            if k.is_host() && self.connected(id) {
                return Err(format!("This {} already has a connection. Put a switch or access point in between.", k.label().to_lowercase()));
            }
        }
        let a_if = self.bind_iface(a, kb);
        let b_if = self.bind_iface(b, ka);
        self.next_id += 1;
        let id = self.next_id;
        self.links.push(Link { id, a, a_if, b, b_if, medium, length_m: None, unplugged: false, a_port: None, b_port: None, kit: String::new() });
        Ok(id)
    }

    /// Picks (or creates) the interface a new link plugs into.
    fn bind_iface(&mut self, id: u32, other: Kind) -> usize {
        let kind = self.dev(id).unwrap().kind;
        match kind {
            Kind::HomeRouter => {
                if other == Kind::Internet { 1 } else { 0 }
            }
            Kind::Router => {
                // Reuse a free physical port before adding a new one.
                let used = |i: usize| self.links.iter().any(|l| (l.a == id && l.a_if == i) || (l.b == id && l.b_if == i));
                let d = self.dev(id).unwrap();
                if let Some(i) = (0..d.ifaces.len()).find(|i| d.ifaces[*i].parent.is_none() && !used(*i)) {
                    return i;
                }
                let seed = self.seed;
                let d = self.dev_mut(id).unwrap();
                let n = d.ifaces.iter().filter(|f| f.parent.is_none()).count();
                d.ifaces.push(Iface {
                    name: format!("eth{n}"),
                    mac: crate::sim::mac(seed, Kind::Router, id, d.ifaces.len() as u32),
                    ..Default::default()
                });
                d.ifaces.len() - 1
            }
            _ => 0,
        }
    }

    /// Router ports stay (with their addresses) when a cable is unplugged, like real hardware.
    pub fn remove_link(&mut self, id: u32) {
        self.links.retain(|l| l.id != id);
    }

    pub fn remove_device(&mut self, id: u32) {
        let ids: Vec<u32> = self.links.iter().filter(|l| l.a == id || l.b == id).map(|l| l.id).collect();
        for l in ids {
            self.remove_link(l);
        }
        self.devices.retain(|d| d.id != id);
    }

    pub fn link_len(&self, l: &Link) -> f32 {
        if let Some(m) = l.length_m {
            return m;
        }
        match (self.dev(l.a), self.dev(l.b)) {
            (Some(a), Some(b)) => {
                let (dx, dy) = ((a.pos[0] - b.pos[0]).abs(), (a.pos[1] - b.pos[1]).abs());
                let storeys = (a.floor as f32 - b.floor as f32).abs() * 3.0;
                if l.medium.cable() || l.medium == Medium::Powerline {
                    // Cables run along walls and ceilings at right angles, up through floors, plus a metre at each end.
                    ((dx + dy) / PX_PER_M + storeys + 2.0).round().max(1.0)
                } else {
                    (((dx * dx + dy * dy).sqrt() / PX_PER_M).powi(2) + storeys * storeys).sqrt().round().max(1.0)
                }
            }
            _ => 0.0,
        }
    }

    /// Physical reach: (device, interface) -> cable-connected component. VLAN sub-interfaces share their port's.
    pub fn phys(&self) -> HashMap<(u32, usize), usize> {
        let keys: Vec<(u32, usize)> = self
            .devices
            .iter()
            .flat_map(|d| d.ifaces.iter().enumerate().filter(|(_, f)| f.parent.is_none()).map(move |(i, _)| (d.id, i)))
            .collect();
        let idx: HashMap<(u32, usize), usize> = keys.iter().enumerate().map(|(i, k)| (*k, i)).collect();
        let mut parent: Vec<usize> = (0..keys.len()).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            p[x] = r;
            r
        }
        for l in &self.links {
            if let (Some(&x), Some(&y)) = (idx.get(&(l.a, l.a_if)), idx.get(&(l.b, l.b_if))) {
                let (rx, ry) = (find(&mut parent, x), find(&mut parent, y));
                parent[rx] = ry;
            }
        }
        let mut out = HashMap::new();
        for (i, k) in keys.iter().enumerate() {
            out.insert(*k, find(&mut parent, i));
        }
        for d in &self.devices {
            for (i, f) in d.ifaces.iter().enumerate() {
                if let Some(c) = f.parent.and_then(|p| out.get(&(d.id, p)).copied()) {
                    out.insert((d.id, i), c);
                }
            }
        }
        out
    }

    /// Broadcast domains: (device, interface) -> segment number. Same cables + same VLAN = same segment.
    pub fn segments(&self) -> HashMap<(u32, usize), usize> {
        let phys = self.phys();
        let mut ids: HashMap<(usize, u16), usize> = HashMap::new();
        let mut out = HashMap::new();
        let mut keys: Vec<&(u32, usize)> = phys.keys().collect();
        keys.sort();
        for k in keys {
            let d = self.dev(k.0).unwrap();
            let vlan = if d.kind.bridges() { 0 } else { d.ifaces[k.1].vlan };
            let n = ids.len();
            out.insert(*k, *ids.entry((phys[k], vlan)).or_insert(n));
        }
        out
    }

    fn connected(&self, id: u32) -> bool {
        self.links.iter().any(|l| l.a == id || l.b == id)
    }

    /// Who in segment `seg` owns `ip`.
    fn owner_in(&self, segs: &HashMap<(u32, usize), usize>, seg: usize, ip: Ipv4Addr) -> Option<(u32, usize)> {
        self.devices.iter().find_map(|d| {
            d.ifaces.iter().enumerate().find_map(|(i, f)| {
                (segs.get(&(d.id, i)) == Some(&seg) && f.addr().map(|a| a.0) == Some(ip)).then_some((d.id, i))
            })
        })
    }

    /// Path through switches / Wi-Fi from one device to another over the same cables,
    /// skipping links spanning tree keeps on standby and Wi-Fi without signal.
    fn l2_path(&self, phys: &HashMap<(u32, usize), usize>, blocked: &HashSet<u32>, from: u32, to: u32, comp: usize) -> Option<(Vec<u32>, f32)> {
        let mut prev: HashMap<u32, (u32, f32)> = HashMap::new();
        let mut q = VecDeque::from([from]);
        let mut seen = HashSet::from([from]);
        while let Some(cur) = q.pop_front() {
            if cur == to {
                let mut path = vec![to];
                let mut ms = 0.0;
                let mut c = to;
                while let Some(&(p, m)) = prev.get(&c) {
                    path.push(p);
                    ms += m;
                    c = p;
                }
                path.reverse();
                return Some((path, ms));
            }
            let can_pass = cur == from || self.dev(cur).is_some_and(|d| d.kind.bridges() || matches!(d.kind, Kind::HomeRouter | Kind::Internet));
            if !can_pass {
                continue;
            }
            for l in &self.links {
                if (l.a != cur && l.b != cur) || blocked.contains(&l.id) || !self.link_ok(l) || phys.get(&(cur, l.end_if(cur))) != Some(&comp) {
                    continue;
                }
                // The backup line only carries traffic while the main line is down.
                if self.link_between(l.a, l.b).is_some_and(|m| m.id != l.id) {
                    continue;
                }
                let sig = self.link_signal(l);
                if sig.is_some_and(|s| !s.alive()) {
                    continue;
                }
                let (n, _) = l.other(cur);
                if seen.insert(n) {
                    let isp = [l.a, l.b].iter().any(|x| self.dev(*x).map(|d| d.kind) == Some(Kind::Internet));
                    let air = sig.map(|s| (-55.0 - s.rssi).max(0.0) * 0.25 + (s.contention - 1.0) * 3.0).unwrap_or(0.0);
                    prev.insert(n, (cur, l.medium.latency_ms() + air + if isp { 6.0 } else { 0.0 }));
                    q.push_back(n);
                }
            }
        }
        None
    }

    fn parse_routes(d: &Device) -> Vec<(Ipv4Addr, u8, Ipv4Addr)> {
        d.routes
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().filter(|w| *w != "via").collect();
                let (net, p) = parts.first()?.split_once('/')?;
                Some((parse_ip(net)?, p.parse().ok()?, parse_ip(parts.get(1)?)?))
            })
            .collect()
    }

    /// Routing decision: (outgoing interface, next-hop address).
    fn route(d: &Device, dst: Ipv4Addr) -> Option<(usize, Ipv4Addr)> {
        let direct = |ip: Ipv4Addr| {
            d.ifaces.iter().enumerate().find_map(|(i, f)| {
                let (a, p) = f.addr()?;
                same_net(a, ip, p).then_some(i)
            })
        };
        if let Some(i) = direct(dst) {
            return Some((i, dst));
        }
        if d.kind.routes() {
            let mut best: Option<(u8, Ipv4Addr)> = None;
            for (net, p, gw) in Self::parse_routes(d) {
                if same_net(net, dst, p) && best.is_none_or(|b| p > b.0) {
                    best = Some((p, gw));
                }
            }
            if let Some((_, gw)) = best {
                return direct(gw).map(|i| (i, gw));
            }
        }
        let gw = parse_ip(&d.gateway)?;
        direct(gw).map(|i| (i, gw))
    }

    /// Simulates one packet from `from` towards `dst`, hop by hop.
    pub fn send(&self, from: u32, src: Ipv4Addr, dst: Ipv4Addr, proto: &'static str, port: u16, nat_back: Option<(u32, Ipv4Addr)>) -> Trace {
        let segs = self.segments();
        let phys = self.phys();
        let stp = self.stp();
        let mut t = Trace { path: vec![from], ..Default::default() };
        // Replies from a far-away site travel the same distance back.
        if let Some(i) = (0..SITES.len()).find(|i| site_ip(*i) == src) {
            t.ms += SITES[i].3;
        }
        if is_real_ip(src) {
            t.ms += REAL_MS;
        }
        let mut cur = from;
        let mut src = src;
        let mut dst = dst;
        let mut came_in: Option<usize> = None;
        // A reply to something sent from inside: NAT lets it in.
        let reply = nat_back.is_some();
        let mut nat_back = nat_back;
        for ttl in (1..=32u8).rev() {
            let d = self.dev(cur).unwrap();
            if d.off {
                t.error = format!("{} is switched off.", d.name);
                return t;
            }
            if let Some((r, inner)) = nat_back {
                if r == cur && d.has_ip(dst) {
                    t.log.push(Log { src: src.to_string(), dst: inner.to_string(), proto: "NAT", layer: "Network", info: format!("{} translates the reply back to {inner}", d.name), dev: cur });
                    dst = inner;
                    nat_back = None;
                }
            }
            // Unsolicited traffic from outside: dropped unless a port forwarding rule passes it on to a device inside.
            if d.kind == Kind::HomeRouter && came_in == Some(1) && !reply && d.ifaces.get(1).and_then(|f| f.addr()).map(|a| a.0) == Some(dst) {
                let shared = self.cgnat(d);
                match (self.forward_of(d, port), shared) {
                    (_, Some(cg)) => {
                        t.error = format!("{} is on the mobile line, behind the provider's carrier-grade NAT: it only has the shared address {cg}, so nothing from the internet reaches it and port forwarding can't work. IPv6 or a VPN/tunnel service gets around it.", d.name);
                        t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("Dropped at the provider's carrier-grade NAT: {} only has the shared address {cg}", d.name), dev: cur });
                        return t;
                    }
                    (Some(inner), None) => {
                        t.log.push(Log { src: src.to_string(), dst: inner.to_string(), proto: "NAT", layer: "Network", info: format!("{} forwards port {port} to {inner}", d.name), dev: cur });
                        dst = inner;
                    }
                    (None, None) => {
                        t.error = format!("{} drops it: nobody inside started this conversation and no port forwarding rule passes port {port} on.", d.name);
                        t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("{} drops unsolicited traffic from the internet (no forwarding rule for port {port})", d.name), dev: cur });
                        return t;
                    }
                }
            }
            if d.has_ip(dst) {
                t.ok = true;
                t.dst_dev = Some(cur);
                t.seen_src = Some(src);
                return t;
            }
            if d.kind == Kind::Internet {
                if let Some(i) = (0..SITES.len()).find(|i| site_ip(*i) == dst) {
                    if let Some(ci) = came_in {
                        t.l3.push((cur, d.ifaces[ci].addr().map(|a| a.0).unwrap_or(dst)));
                    }
                    t.ok = true;
                    t.site = Some(i);
                    t.dst_dev = Some(cur);
                    t.seen_src = Some(src);
                    t.ms += SITES[i].3;
                    return t;
                }
                if is_real_ip(dst) {
                    if let Some(ci) = came_in {
                        t.l3.push((cur, d.ifaces[ci].addr().map(|a| a.0).unwrap_or(dst)));
                    }
                    t.ok = true;
                    t.real = true;
                    t.dst_dev = Some(cur);
                    t.seen_src = Some(src);
                    t.ms += REAL_MS;
                    return t;
                }
            }
            if cur != from && !d.kind.routes() {
                t.error = format!("{} received a packet for {dst} but doesn't forward packets.", d.name);
                return t;
            }
            let Some((out, hop)) = Self::route(d, dst) else {
                t.error = if d.kind == Kind::Internet && dst.is_private() {
                    format!("The internet drops packets to {dst}: private addresses (10.x, 172.16–31.x, 192.168.x) only exist inside a home or office.")
                } else if cur == from && d.kind.is_host() {
                    format!("{} has no route to {dst}: it's in another network and no default gateway is set.", d.name)
                } else {
                    format!("{} has no route to {dst} (no matching network, route or gateway).", d.name)
                };
                return t;
            };
            let Some(&seg) = segs.get(&(cur, out)) else {
                t.error = format!("{} isn't plugged in.", d.name);
                return t;
            };
            t.log.push(Log { src: d.name.clone(), dst: "broadcast".into(), proto: "ARP", layer: "Link", info: format!("Who has {hop}? Tell {}", d.ifaces[out].ip), dev: cur });
            let Some((next, next_if)) = self.owner_in(&segs, seg, hop) else {
                t.error = format!("Nobody answers ARP for {hop}: no device with that address is in {}'s network.", d.name);
                return t;
            };
            // Both ends of an ARP exchange remember each other.
            t.arp.push((cur, hop));
            if let Some((me, _)) = d.ifaces[out].addr() {
                t.arp.push((next, me));
            }
            if d.kind.routes() && cur != from {
                let guest = |i: usize| d.ifaces.get(i).is_some_and(|f| f.vlan == crate::extras::GUEST_VLAN);
                if d.kind == Kind::HomeRouter && ((came_in.is_some_and(guest) && out != 1) || (guest(out) && came_in != Some(1))) {
                    t.error = format!("{}: the guest network is isolated from the home network — guests only get the internet.", d.name);
                    return t;
                }
                if let Some((false, rule)) = self.rule_verdict(d, src, dst, port) {
                    t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("Dropped by {}'s firewall rule '{rule}'", d.name), dev: cur });
                    t.error = format!("Blocked by {}'s firewall rule '{rule}'.", d.name);
                    return t;
                }
            }
            let comp = phys[&(cur, out)];
            if let Some(l) = stp.storm.iter().filter_map(|id| self.links.iter().find(|l| l.id == *id)).find(|l| phys.get(&(l.a, l.a_if)) == Some(&comp)) {
                t.error = format!("Broadcast storm: {} and {} form a loop that no spanning tree breaks. Broadcasts circle forever and the network is flooded.", self.name(l.a), self.name(l.b));
                return t;
            }
            let Some((path, ms)) = self.l2_path(&phys, &stp.blocked, cur, next, comp) else {
                t.error = match self.links.iter().find(|l| (l.a == cur || l.b == cur || l.a == next || l.b == next) && l.medium == Medium::WiFi).and_then(|l| self.link_signal(l).filter(|s| !s.alive()).map(|s| (l, s))) {
                    Some((l, s)) => format!("No Wi-Fi: {} and {} can't hear each other ({:.0} dBm through {} wall(s)).", self.name(l.a), self.name(l.b), s.rssi, s.walls),
                    None => format!("No working path from {} to {}.", d.name, self.name(next)),
                };
                return t;
            };
            if d.ifaces[out].vlan != 1 {
                t.log.push(Log { src: d.name.clone(), dst: self.name(next), proto: "802.1Q", layer: "Link", info: format!("Frame tagged with VLAN {} on the trunk", d.ifaces[out].vlan), dev: cur });
            }
            t.ms += ms;
            t.path.extend(path.into_iter().skip(1));
            if d.kind == Kind::HomeRouter && out == 1 && came_in.is_some_and(|i| i != 1) {
                let wan = parse_ip(&d.ifaces[1].ip).unwrap_or(src);
                t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("{} replaces sender {src} with its public address {wan}", d.name), dev: cur });
                if let Some(cg) = self.cgnat(d) {
                    t.log.push(Log { src: cg.clone(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("Mobile line: the provider translates {cg} once more to an address shared by many customers (carrier-grade NAT)"), dev: cur });
                }
                t.nat = Some((cur, src));
                src = wan;
            }
            if cur != from {
                t.l3.push((cur, d.ifaces[came_in.unwrap_or(out)].addr().map(|a| a.0).unwrap_or(hop)));
            }
            t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("{} sends to {} (next hop {hop}, TTL {ttl})", d.name, self.name(next)), dev: cur });
            cur = next;
            came_in = Some(next_if);
        }
        t.error = "TTL expired: the packet went in circles (routing loop).".into();
        t
    }

    /// The shared address (100.64.0.0/10) a router gets while its internet runs over the mobile line
    /// (carrier-grade NAT), or None on a normal line.
    // shortcut: inside the simulation the router keeps its usual WAN address; the 100.64 one is shown and explained.
    pub fn cgnat(&self, r: &Device) -> Option<String> {
        let inet = self.devices.iter().find(|d| d.kind == Kind::Internet)?;
        let h = crate::sim::hash(self.seed, r.id as u64, 0xC6);
        (self.link_between(r.id, inet.id)?.medium == Medium::Cellular).then(|| format!("100.{}.{}.{}", 64 + h % 64, (h >> 8) % 256, 1 + (h >> 16) % 254))
    }

    fn src_ip(&self, id: u32) -> Option<Ipv4Addr> {
        self.dev(id)?.first_ip()
    }

    /// Resolves a host name through the configured DNS servers.
    pub fn resolve(&self, from: u32, name: &str, out: &mut Outcome) -> Option<Ipv4Addr> {
        if let Some(ip) = parse_ip(name) {
            return Some(ip);
        }
        let d = self.dev(from)?;
        let name = name.trim_end_matches('.').to_lowercase();
        // The internet device is the outside world: it knows public names itself.
        if d.kind == Kind::Internet {
            if let Some(ip) = self.answer(d, &name) {
                return Some(ip);
            }
        }
        let Some(dns) = parse_ip(&d.dns) else {
            out.lines.push(format!("Can't look up '{name}': no DNS server is set on {}.", d.name));
            return None;
        };
        let Some(src) = self.src_ip(from) else {
            out.lines.push("No IP address yet.".into());
            return None;
        };
        let t = self.send(from, src, dns, "DNS", 53, None);
        out.log.push(Log { src: src.to_string(), dst: dns.to_string(), proto: "DNS", layer: "Application", info: format!("Query: A record for {name}?"), dev: from });
        out.log.extend(t.log.clone());
        let ok = t.ok;
        let err = t.error.clone();
        let server = t.dst_dev;
        out.traces.push(t);
        if !ok {
            out.lines.push(format!("DNS server {dns} unreachable: {err}"));
            return None;
        }
        let mut server = server?;
        for _ in 0..4 {
            let s = self.dev(server)?;
            if let Some(site) = self.blocked_by(s, &name) {
                out.lines.push(format!("Blocked: {name} is on {}'s filter list ({site}).", s.name));
                out.log.push(Log { src: s.name.clone(), dst: src.to_string(), proto: "DNS", layer: "Application", info: format!("{name} blocked by the content filter"), dev: server });
                return None;
            }
            if let Some(ip) = self.answer(s, &name) {
                out.log.push(Log { src: dns.to_string(), dst: src.to_string(), proto: "DNS", layer: "Application", info: format!("Answer from {}: {name} is {ip}", s.name), dev: from });
                return Some(ip);
            }
            // Forward to the upstream resolver.
            let Some(up) = parse_ip(&s.dns).or_else(|| parse_ip(&s.gateway)) else { break };
            let Some(next) = self.devices.iter().find(|x| x.has_ip(up)) else { break };
            if next.id == server {
                break;
            }
            out.log.push(Log { src: s.name.clone(), dst: up.to_string(), proto: "DNS", layer: "Application", info: format!("{} doesn't know {name}, asks {}", s.name, next.name), dev: server });
            server = next.id;
        }
        out.lines.push(format!("** server can't find {name}: NXDOMAIN (no such name)"));
        None
    }

    fn answer(&self, s: &Device, name: &str) -> Option<Ipv4Addr> {
        if s.services.dns {
            for line in s.dns_records.lines() {
                let mut w = line.split_whitespace();
                if let (Some(n), Some(ip)) = (w.next(), w.next()) {
                    if n.eq_ignore_ascii_case(name) {
                        return parse_ip(ip);
                    }
                }
            }
        }
        if s.kind == Kind::Internet {
            return SITES.iter().position(|x| x.0 == name).map(site_ip).or_else(|| is_public_name(name).then(|| real_ip(name)));
        }
        if s.kind == Kind::HomeRouter && s.services.dns {
            // Home routers answer for local devices as name or name.home.arpa (RFC 8375).
            let segs = self.segments();
            let lan = segs.get(&(s.id, 0))?;
            let short = name.trim_end_matches(".home.arpa");
            return self.devices.iter().find_map(|d| {
                (d.hostname() == short && segs.get(&(d.id, 0)) == Some(lan)).then(|| d.first_ip()).flatten()
            });
        }
        None
    }

    /// Runs DHCP for every device that wants an automatic address.
    pub fn run_dhcp(&mut self) -> Vec<Log> {
        let mut log = vec![];
        let mut order: Vec<u32> = self.devices.iter().filter(|d| d.dhcp_client && d.kind.routes()).map(|d| d.id).collect();
        order.extend(self.devices.iter().filter(|d| d.dhcp_client && !d.kind.routes()).map(|d| d.id));
        for id in order {
            self.dhcp_one(id, &mut log);
        }
        log
    }

    pub fn dhcp_one(&mut self, id: u32, log: &mut Vec<Log>) {
        let segs = self.segments();
        let Some(d) = self.dev(id) else { return };
        let ifi = d.dhcp_iface();
        if ifi >= d.ifaces.len() {
            return;
        }
        let name = d.name.clone();
        // Leases belong to the network card's MAC address: the same device keeps (or gets back) the same address.
        let mac = crate::sim::hash(0, d.ifaces[ifi].mac.bytes().fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64)), 0xD4);
        let had = d.ifaces[ifi].addr().map(|a| u32::from(a.0));
        // Without an answer a device picks its own 169.254.1.0–169.254.254.255 address (from its MAC).
        let link_local = format!("169.254.{}.{}", 1 + mac % 254, (mac >> 8) % 256);
        let Some(&seg) = segs.get(&(id, ifi)).filter(|_| self.connected(id)) else {
            let d = self.dev_mut(id).unwrap();
            d.ifaces[ifi].ip.clear();
            return;
        };
        log.push(Log { src: "0.0.0.0".into(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("DISCOVER from {name}: is there a DHCP server?"), dev: id });
        // Find a server with an address in this segment (a router may serve many VLANs), reachable over the air/cables.
        let phys = self.phys();
        let blocked = self.stp().blocked;
        let servers: Vec<(&Device, usize)> = self
            .devices
            .iter()
            .filter(|s| s.id != id && s.services.dhcp)
            .filter_map(|s| {
                let j = (0..s.ifaces.len()).find(|j| (s.kind != Kind::HomeRouter || *j != 1) && segs.get(&(s.id, *j)) == Some(&seg) && s.ifaces[*j].addr().is_some())?;
                self.l2_path(&phys, &blocked, id, s.id, phys[&(id, ifi)]).map(|_| (s, j))
            })
            .collect();
        // With two DHCP servers in one network, whichever answers first wins — it differs per device.
        let server = (!servers.is_empty()).then(|| servers[(crate::sim::hash(self.seed, id as u64, 0xD7) % servers.len() as u64) as usize]);
        let Some((s, j)) = server else {
            let ip = link_local;
            log.push(Log { src: name.clone(), dst: "-".into(), proto: "DHCP", layer: "Application", info: format!("No answer — {name} gives itself the link-local address {ip}"), dev: id });
            let d = self.dev_mut(id).unwrap();
            d.ifaces[ifi].ip = ip;
            d.ifaces[ifi].prefix = 16;
            d.gateway.clear();
            d.dns.clear();
            return;
        };
        let (sip, spfx) = s.ifaces[j].addr().unwrap();
        let (sname, sid) = (s.name.clone(), s.id);
        let (from, to) = (parse_ip(&s.dhcp_from), parse_ip(&s.dhcp_to));
        let used: HashSet<Ipv4Addr> = self
            .devices
            .iter()
            .filter(|x| x.id != id)
            .flat_map(|x| x.ifaces.iter().enumerate().filter(|(i, _)| segs.get(&(x.id, *i)) == Some(&seg)).filter_map(|(_, f)| f.addr().map(|a| a.0)).collect::<Vec<_>>())
            .collect();
        let base = u32::from(net_of(sip, spfx));
        let range = match (from, to) {
            (Some(f), Some(t)) if same_net(f, sip, spfx) && same_net(t, sip, spfx) => Some((u32::from(f), u32::from(t))),
            // Other VLAN interfaces of a router get an automatic pool .100–.199.
            _ if j > 0 && spfx <= 24 => Some((base + 100, base + 199)),
            _ => None,
        };
        let free = |ip: u32| !used.contains(&Ipv4Addr::from(ip));
        let pick = range.filter(|(f, t)| t >= f).and_then(|(f, t)| {
            let n = t - f + 1;
            had.filter(|ip| (f..=t).contains(ip) && free(*ip)).or_else(|| (0..n).map(|k| f + ((mac % n as u64) as u32 + k) % n).find(|ip| free(*ip)))
        });
        let pick = pick.map(Ipv4Addr::from);
        let Some(ip) = pick else {
            // A full (or broken) pool means no answer: the device falls back to a link-local address.
            log.push(Log { src: sip.to_string(), dst: name.clone(), proto: "DHCP", layer: "Application", info: format!("{sname} has no free address in its pool (or the pool doesn't match its network)"), dev: sid });
            let d = self.dev_mut(id).unwrap();
            d.ifaces[ifi].ip = link_local;
            d.ifaces[ifi].prefix = 16;
            d.gateway.clear();
            d.dns.clear();
            return;
        };
        let gw = sip;
        let dns = match s.kind {
            Kind::Router => parse_ip(&s.dns).unwrap_or(sip),
            k if k.routes() => sip,
            _ => parse_ip(&s.dns).unwrap_or(sip),
        };
        let gw = if s.kind.routes() { gw } else { parse_ip(&s.gateway).unwrap_or(gw) };
        log.push(Log { src: sip.to_string(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("OFFER from {sname}: use {ip}/{spfx}, gateway {gw}, DNS {dns}"), dev: sid });
        log.push(Log { src: "0.0.0.0".into(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("REQUEST from {name}: I'll take {ip}"), dev: id });
        log.push(Log { src: sip.to_string(), dst: ip.to_string(), proto: "DHCP", layer: "Application", info: "ACK: the address is yours for 24 h".into(), dev: sid });
        let d = self.dev_mut(id).unwrap();
        d.ifaces[ifi].ip = ip.to_string();
        d.ifaces[ifi].prefix = spfx;
        d.gateway = gw.to_string();
        d.dns = dns.to_string();
    }

    /// Plans addresses for the whole drawing, then runs DHCP.
    pub fn auto_address(&mut self) -> Vec<Log> {
        let segs = self.segments();
        let mut groups: BTreeMap<usize, Vec<(u32, usize)>> = BTreeMap::new();
        for (&(d, i), &s) in &segs {
            let dev = self.dev(d).unwrap();
            let port = dev.ifaces[i].parent.unwrap_or(i);
            if !dev.kind.bridges() && dev.kind != Kind::Neighbor && self.links.iter().any(|l| (l.a == d && l.a_if == port) || (l.b == d && l.b_if == port)) {
                groups.entry(s).or_default().push((d, i));
            }
        }
        let mut subnet = 0u8;
        let mut lan_no = 1u8;
        for members in groups.values_mut() {
            members.sort();
            let kind = |n: &Network, id: u32| n.dev(id).unwrap().kind;
            if members.iter().any(|m| kind(self, m.0) == Kind::Internet) {
                for m in members.iter() {
                    if kind(self, m.0) == Kind::HomeRouter {
                        self.dev_mut(m.0).unwrap().dhcp_client = true;
                    }
                }
                continue;
            }
            // A home router LAN without an address gets one.
            for m in members.iter() {
                let d = self.dev_mut(m.0).unwrap();
                if d.kind == Kind::HomeRouter && m.1 == 0 {
                    if d.ifaces[0].addr().is_none() {
                        d.ifaces[0].ip = format!("192.168.{lan_no}.1");
                        d.ifaces[0].prefix = 24;
                    }
                    let (ip, _) = d.ifaces[0].addr().unwrap();
                    let o = ip.octets();
                    d.services.dhcp = true;
                    d.services.dns = true;
                    d.dhcp_from = format!("{}.{}.{}.100", o[0], o[1], o[2]);
                    d.dhcp_to = format!("{}.{}.{}.199", o[0], o[1], o[2]);
                    lan_no += 1;
                }
            }
            let server = members.iter().find_map(|m| {
                let d = self.dev(m.0).unwrap();
                ((m.1 == 0 || d.kind == Kind::Router) && d.services.dhcp).then(|| d.ifaces[m.1].addr()).flatten().map(|a| (m.0, a))
            });
            if let Some((sid, (sip, p))) = server {
                let o = sip.octets();
                // u16 and a stop at .254: a network with hundreds of fixed devices just runs out, it doesn't wrap.
                let mut n = 10u16;
                for m in members.iter() {
                    if m.0 == sid {
                        continue;
                    }
                    let d = self.dev_mut(m.0).unwrap();
                    if d.kind.is_host() && !matches!(d.kind, Kind::Server | Kind::Nas | Kind::Printer) {
                        d.dhcp_client = true;
                    } else if (d.kind.is_host() || d.kind == Kind::Router) && n <= 254 {
                        // Fixed address below the DHCP pool, so it never moves.
                        d.dhcp_client = false;
                        d.ifaces[m.1].ip = format!("{}.{}.{}.{n}", o[0], o[1], o[2]);
                        d.ifaces[m.1].prefix = p;
                        if d.kind.is_host() {
                            d.gateway = sip.to_string();
                            d.dns = sip.to_string();
                        }
                        n += 1;
                    }
                }
            } else {
                subnet += 1;
                let (mut r, mut h) = (1u16, 10u16);
                let mut gw = String::new();
                for m in members.iter() {
                    let d = self.dev_mut(m.0).unwrap();
                    if d.kind.routes() && r <= 254 {
                        d.ifaces[m.1].ip = format!("10.0.{subnet}.{r}");
                        d.ifaces[m.1].prefix = 24;
                        if gw.is_empty() {
                            gw = d.ifaces[m.1].ip.clone();
                        }
                        r += 1;
                    }
                }
                for m in members.iter() {
                    let d = self.dev_mut(m.0).unwrap();
                    // The interface on this network: a host with VLAN sub-interfaces is on several.
                    if d.kind.is_host() && h <= 254 {
                        d.dhcp_client = false;
                        d.ifaces[m.1].ip = format!("10.0.{subnet}.{h}");
                        d.ifaces[m.1].prefix = 24;
                        d.gateway = gw.clone();
                        h += 1;
                    }
                }
            }
        }
        self.plan_routes();
        // Static hosts without DNS use any DNS server in the drawing.
        let dns_server = self.devices.iter().find(|d| d.services.dns && d.kind.is_host()).and_then(|d| d.first_ip());
        for d in &mut self.devices {
            if d.kind.is_host() && !d.dhcp_client && d.dns.is_empty() {
                if let Some(ip) = dns_server {
                    d.dns = ip.to_string();
                }
            }
        }
        self.run_dhcp()
    }

    /// Fills each router's table with routes to networks it isn't directly on.
    fn plan_routes(&mut self) {
        let segs = self.segments();
        // Each routing device's (segment, own address, prefix) list.
        let routers: Vec<(u32, Vec<(usize, Ipv4Addr, u8)>)> = self
            .devices
            .iter()
            .filter(|d| matches!(d.kind, Kind::Router | Kind::HomeRouter))
            .map(|d| (d.id, d.ifaces.iter().enumerate().filter_map(|(i, f)| Some((*segs.get(&(d.id, i))?, f.addr()?.0, f.addr()?.1))).collect()))
            .collect();
        for (rid, mine) in &routers {
            let my_segs: HashSet<usize> = mine.iter().map(|m| m.0).collect();
            // BFS across routers: segment -> first-hop gateway address.
            let mut via: HashMap<usize, Ipv4Addr> = HashMap::new();
            let mut q: VecDeque<(u32, Option<Ipv4Addr>)> = VecDeque::from([(*rid, None)]);
            let mut seen = HashSet::from([*rid]);
            while let Some((r, first)) = q.pop_front() {
                let rs = &routers.iter().find(|x| x.0 == r).unwrap().1;
                for (seg, _, _) in rs {
                    if let Some(f) = first {
                        via.entry(*seg).or_insert(f);
                    }
                    for (oid, others) in &routers {
                        if let Some(o) = others.iter().find(|o| o.0 == *seg) {
                            if seen.insert(*oid) && self.dev(*oid).unwrap().kind == Kind::Router {
                                q.push_back((*oid, first.or(Some(o.1))));
                            }
                        }
                    }
                }
            }
            let mut lines = vec![];
            for (seg, gw) in via {
                if my_segs.contains(&seg) {
                    continue;
                }
                if let Some((_, ip, p)) = routers.iter().flat_map(|r| r.1.iter()).find(|x| x.0 == seg) {
                    lines.push(format!("{}/{p} via {gw}", net_of(*ip, *p)));
                }
            }
            lines.sort();
            lines.dedup();
            let d = self.dev_mut(*rid).unwrap();
            d.routes = lines.join("\n");
        }
        // Routers sharing a network with a home router send everything else there.
        let homes: Vec<(usize, Ipv4Addr)> = routers
            .iter()
            .filter(|r| self.dev(r.0).unwrap().kind == Kind::HomeRouter)
            .flat_map(|r| r.1.iter().filter(|x| segs.get(&(r.0, 0)) == Some(&x.0)).map(|x| (x.0, x.1)))
            .collect();
        for (rid, mine) in &routers {
            if self.dev(*rid).unwrap().kind != Kind::Router {
                continue;
            }
            if let Some((_, ip)) = homes.iter().find(|h| mine.iter().any(|m| m.0 == h.0)) {
                self.dev_mut(*rid).unwrap().gateway = ip.to_string();
            }
        }
    }

    /// Finds everything that will stop the network from working, or is outdated.
    pub fn check(&self) -> Vec<Issue> {
        let segs = self.segments();
        let mut v = vec![];
        let mut push = |level, dev: Option<u32>, link: Option<u32>, text: String| v.push(Issue { level, dev, link, text, fix: None });
        if self.devices.is_empty() {
            return v;
        }
        // Duplicate addresses.
        let mut seen: HashMap<(usize, Ipv4Addr), u32> = HashMap::new();
        for d in &self.devices {
            for (i, f) in d.ifaces.iter().enumerate() {
                if let (Some((ip, _)), Some(&s)) = (f.addr(), segs.get(&(d.id, i))) {
                    if let Some(o) = seen.insert((s, ip), d.id) {
                        push(Level::Error, Some(d.id), None, format!("{} and {} both use {ip}. Every address must be unique.", d.name, self.name(o)));
                    }
                }
            }
        }
        for d in &self.devices {
            let id = Some(d.id);
            if d.kind.foreign() {
                continue;
            }
            if !self.connected(d.id) {
                push(Level::Warning, id, None, format!("{} isn't connected to anything.", d.name));
                continue;
            }
            for f in &d.ifaces {
                if !f.ip.trim().is_empty() && f.addr().is_none() {
                    push(Level::Error, id, None, format!("{}: '{}' isn't a valid IPv4 address.", d.name, f.ip));
                }
                if let Some((ip, p)) = f.addr() {
                    if p == 0 || p > 30 && !(p == 32 && d.kind.is_host()) {
                        push(Level::Warning, id, None, format!("{}: prefix /{p} is unusual; home networks use /24 (255.255.255.0).", d.name));
                    }
                    if ip.octets()[0] == 169 && ip.octets()[1] == 254 {
                        push(Level::Error, id, None, format!("{} only has a link-local address ({ip}) — no DHCP server reached it.", d.name));
                    }
                }
            }
            if d.kind.is_host() {
                let Some((ip, p)) = d.ifaces[0].addr() else {
                    if d.dhcp_client {
                        push(Level::Tip, id, None, format!("{} will get its address by DHCP when the simulation starts.", d.name));
                    } else {
                        push(Level::Error, id, None, format!("{} has no IP address. Set one or turn on DHCP.", d.name));
                    }
                    continue;
                };
                match parse_ip(&d.gateway) {
                    Some(gw) if !same_net(gw, ip, p) => push(Level::Error, id, None, format!("{}'s gateway {gw} is outside its own network {}/{p}.", d.name, net_of(ip, p))),
                    Some(gw) => {
                        if let Some(&s) = segs.get(&(d.id, 0)) {
                            if self.owner_in(&segs, s, gw).is_none() {
                                push(Level::Warning, id, None, format!("{}'s gateway {gw} doesn't exist in its network.", d.name));
                            }
                        }
                    }
                    None if !d.gateway.trim().is_empty() => push(Level::Error, id, None, format!("{}: gateway '{}' isn't a valid address.", d.name, d.gateway)),
                    None if self.devices.iter().any(|x| x.kind.routes()) => push(Level::Tip, id, None, format!("{} has no default gateway, so it can only reach its own network.", d.name)),
                    None => {}
                }
                if !d.dhcp_client {
                    for s in &self.devices {
                        if let (true, Some(f), Some(t)) = (s.services.dhcp && segs.get(&(s.id, 0)) == segs.get(&(d.id, 0)), parse_ip(&s.dhcp_from), parse_ip(&s.dhcp_to)) {
                            if (u32::from(f)..=u32::from(t)).contains(&u32::from(ip)) {
                                push(Level::Warning, id, None, format!("{}'s fixed address {ip} is inside {}'s DHCP pool — it may be handed out twice.", d.name, s.name));
                            }
                        }
                    }
                }
            }
            if d.services.dhcp {
                if let (Some((ip, p)), Some(f), Some(t)) = (d.ifaces.first().and_then(|f| f.addr()), parse_ip(&d.dhcp_from), parse_ip(&d.dhcp_to)) {
                    if !same_net(f, ip, p) || !same_net(t, ip, p) || u32::from(f) > u32::from(t) {
                        push(Level::Error, Some(d.id), None, format!("{}'s DHCP pool {f}–{t} doesn't fit its network {}/{p}.", d.name, net_of(ip, p)));
                    }
                }
            }
            if d.kind.radio() && d.bands.iter().any(|b| *b) {
                match d.security {
                    Security::Open => push(Level::Error, id, None, format!("{}'s Wi-Fi has no password. Anyone nearby can join and read unencrypted traffic.", d.name)),
                    Security::Wep => push(Level::Error, id, None, format!("{} uses WEP, which can be cracked in minutes. Use WPA3 (or WPA2/WPA3 mixed).", d.name)),
                    Security::Wpa2 => push(Level::Tip, id, None, format!("{}: WPA3 is safer than WPA2 and required for 6 GHz Wi-Fi.", d.name)),
                    Security::Wpa3 | Security::Wpa3Ent => {}
                }
            }
        }
        // Two DHCP servers in one network.
        let mut dhcp: HashMap<usize, u32> = HashMap::new();
        for d in self.devices.iter().filter(|d| d.services.dhcp && self.connected(d.id)) {
            if let Some(&s) = segs.get(&(d.id, 0)) {
                if let Some(o) = dhcp.insert(s, d.id) {
                    push(Level::Warning, Some(d.id), None, format!("{} and {} both hand out addresses in the same network. Keep one DHCP server.", d.name, self.name(o)));
                }
            }
        }
        for l in &self.links {
            let (Some(a), Some(b)) = (self.dev(l.a), self.dev(l.b)) else { continue };
            let len = self.link_len(l);
            let lk = Some(l.id);
            if l.medium == Medium::WiFi {
                let (ap, cl) = if a.kind.radio() { (a, b) } else { (b, a) };
                if !ap.kind.radio() {
                    push(Level::Error, None, lk, format!("Wi-Fi between {} and {}: one side must be a router or access point.", a.name, b.name));
                } else if !cl.kind.wifi_client() && !cl.kind.radio() {
                    push(Level::Error, None, lk, format!("{} has no Wi-Fi. Use a cable.", cl.name));
                }
            } else {
                if l.medium.cable() && l.medium != Medium::Fiber && len > 100.0 {
                    push(Level::Error, None, lk, format!("The cable {}–{} is {len:.0} m; copper Ethernet is limited to 100 m.", a.name, b.name));
                }
            }
        }
        if !self.devices.iter().any(|d| d.kind == Kind::Internet) {
            push(Level::Tip, None, None, "No internet connection in this plan. Add the Internet cloud and connect it to a router.".into());
        }
        self.check_more(&mut v);
        v.sort_by_key(|i| i.level);
        v
    }

    fn lookup_dev(&self, word: &str) -> Option<&Device> {
        let w = word.to_lowercase();
        self.devices.iter().find(|d| d.hostname() == w || d.name.to_lowercase() == w)
    }

    /// Connects to a service (web, files, print, dns) and returns what it serves.
    /// For the web, `url` may carry a page path: "192.168.1.10/about.html".
    pub fn service(&self, id: u32, url: &str, svc: &str, o: &mut Outcome) -> Option<String> {
        let url = url.trim().trim_start_matches("https://").trim_start_matches("http://");
        let (target, path) = url.split_once('/').unwrap_or((url, ""));
        let (port, proto, svc_name) = match svc {
            "files" | "smb" => (445, "SMB", "file sharing"),
            "print" | "ipp" => (631, "IPP", "printing"),
            "dns" => (53, "DNS", "DNS"),
            _ => (443, "HTTPS", "web"),
        };
        let dst = self.resolve(id, target, o)?;
        let Some(src) = self.src_ip(id) else {
            o.lines.push("No IP address — is DHCP working?".into());
            return None;
        };
        let t = self.send(id, src, dst, "TCP", port, None);
        o.log.push(Log { src: format!("{src}:51514"), dst: format!("{dst}:{port}"), proto: "TCP", layer: "Transport", info: "SYN — open a connection".into(), dev: id });
        o.log.extend(t.log.clone());
        if !t.ok {
            o.lines.push(format!("Can't connect to {target}: {}", t.error));
            o.traces.push(t);
            return None;
        }
        let (dev_id, site, seen, nat, real, there) = (t.dst_dev.unwrap(), t.site, t.seen_src.unwrap(), t.nat, t.real, t.ms);
        o.traces.push(t);
        let target_dev = self.dev(dev_id).unwrap();
        let mut status = "200 OK";
        let content = if let Some(s) = site {
            (svc_name == "web").then(|| SITES[s].2.to_string())
        } else if real {
            (svc_name == "web").then(|| format!("<p>{target} is a real website. The browser fetches it from the real internet, at this network's speed.</p>"))
        } else {
            let s = &target_dev.services;
            match svc_name {
                "web" if s.web => Some(match path.trim_end_matches('/') {
                    "" | "index.html" | "index.htm" if target_dev.web_page.is_empty() => format!("{} web page", target_dev.name),
                    "" | "index.html" | "index.htm" => target_dev.web_page.clone(),
                    p => match target_dev.web_files.iter().find(|f| f.0.eq_ignore_ascii_case(p)) {
                        Some(f) => f.1.clone(),
                        None => {
                            status = "404 Not Found";
                            format!("<h1>404 Not Found</h1><p>{} has no page called {p}.</p>", target_dev.name)
                        }
                    },
                }),
                "file sharing" if s.files => Some(format!("\\\\{}\\shared — connected with SMB 3.1.1 (encrypted)", target_dev.hostname())),
                "printing" if s.print => Some(format!("{} is ready (IPP Everywhere, no driver needed)", target_dev.name)),
                "DNS" if s.dns || target_dev.kind.routes() => Some(format!("{} answers DNS on port 53", target_dev.name)),
                _ => None,
            }
        };
        let Some(text) = content else {
            o.lines.push(format!("Connection refused: {} doesn't offer {svc_name} (port {port}).", target_dev.name));
            o.log.push(Log { src: format!("{dst}:{port}"), dst: src.to_string(), proto: "TCP", layer: "Transport", info: "RST — nothing listens on this port".into(), dev: dev_id });
            return None;
        };
        let back = self.send(dev_id, dst, seen, "TCP", port, nat);
        o.log.extend(back.log.clone());
        if real {
            o.fetch = Some((format!("https://{target}/{path}"), there + back.ms));
        }
        o.traces.push(back);
        o.log.push(Log { src: format!("{dst}:{port}"), dst: format!("{src}:51514"), proto: "TCP", layer: "Transport", info: "SYN-ACK, then ACK — connection open".into(), dev: id });
        if proto == "HTTPS" {
            o.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "TLS", layer: "Application", info: "TLS 1.3 handshake — traffic is now encrypted".into(), dev: id });
            o.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "HTTP", layer: "Application", info: format!("GET /{path} HTTP/2  Host: {target}"), dev: id });
            o.log.push(Log { src: dst.to_string(), dst: src.to_string(), proto: "HTTP", layer: "Application", info: format!("HTTP/2 {status} — {} bytes", text.len()), dev: dev_id });
            o.lines.push(format!("HTTP/2 {status}  ({target}, secured with TLS 1.3)"));
        } else {
            o.lines.push(format!("{proto} port {port} open on {target}"));
        }
        Some(text)
    }

    /// Runs one command typed into a device's terminal.
    pub fn exec(&mut self, id: u32, line: &str) -> Outcome {
        let mut o = self.run_cmd(id, line);
        for &(d, ip) in o.traces.iter().flat_map(|t| t.arp.iter()) {
            let known = self.arp_seen.entry(d).or_default();
            if !known.contains(&ip) {
                known.push(ip);
            }
        }
        match line.split_whitespace().next().unwrap_or("").to_lowercase().as_str() {
            "speedtest" | "iperf" => {
                if let (Some(i), Some(t)) = (o.lines.iter().position(|l| l.starts_with("  Download")), o.traces.last()) {
                    // Uploads ride on the devices' own (weaker) radios; TCP never quite fills the pipe.
                    let up = self.upload_rate(&t.path) * 0.93;
                    o.lines.insert(i + 1, format!("  Upload   ≈ {up:.0} Mbit/s"));
                }
            }
            "stp" | "spanning-tree" => o.lines.extend(self.stp_convergence(&self.stp())),
            _ => {}
        }
        o
    }

    fn run_cmd(&mut self, id: u32, line: &str) -> Outcome {
        let mut o = Outcome::default();
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = words.first() else { return o };
        let arg = words.get(1).copied().unwrap_or("");
        let Some(d) = self.dev(id).cloned() else { return o };
        match cmd.to_lowercase().as_str() {
            "help" | "?" => {
                o.lines.extend(
                    [
                        "ipconfig            show my addresses (also: ip a)",
                        "ping <name|ip>      is it reachable? (ICMP echo)",
                        "traceroute <name>   list the routers on the way (also: tracert)",
                        "nslookup <name>     ask DNS for an address",
                        "open <name>         load a web page over HTTPS (also: curl)",
                        "test <name> <svc>   check a service: web, files, print, dns",
                        "arp                 neighbours I know by MAC address",
                        "route               my routing table",
                        "speedtest [name]    real speed and the bottleneck on the way",
                        "ping6 <name>        IPv6 ping (no NAT; inbound needs a firewall rule)",
                        "wifi                my signal and every network in the air",
                        "stp                 spanning tree: root bridge, standby links",
                        "vlan                my VLANs",
                        "dhcp                ask for a new address",
                        "clear               clear the screen",
                    ]
                    .map(String::from),
                );
            }
            "ipconfig" | "ifconfig" | "ip" => {
                for f in &d.ifaces {
                    o.lines.push(format!("{}: {}  mac {}", f.name, if f.ip.is_empty() { "no address".into() } else { format!("{}/{}", f.ip, f.prefix) }, f.mac));
                }
                o.lines.push(format!("gateway: {}   dns: {}   {}", or_dash(&d.gateway), or_dash(&d.dns), if d.dhcp_client { "(from DHCP)" } else { "(fixed)" }));
                match self.v6_of(&d) {
                    Some(v6) => o.lines.push(format!("IPv6: {v6}/64 (global, from the router via SLAAC) + fe80:: link-local")),
                    None => o.lines.push("IPv6: fe80:: link-local only (no IPv6 from the router)".into()),
                }
                o.lines.extend(self.v6_delegation(&d));
                if let Some(cg) = self.cgnat(&d) {
                    o.lines.push(format!("Mobile line: the provider only gives this router the shared address {cg} (carrier-grade NAT) — no port forwarding."));
                }
            }
            "ping6" => self.ping6(id, arg, &mut o),
            "ping" if arg == "-6" => self.ping6(id, words.get(2).copied().unwrap_or(""), &mut o),
            "ping" => {
                let Some(dst) = self.resolve(id, arg, &mut o) else {
                    if arg.is_empty() {
                        o.lines.push("usage: ping <name or address>".into());
                    }
                    return o;
                };
                let Some(src) = self.src_ip(id) else {
                    o.lines.push("No IP address — is DHCP working?".into());
                    return o;
                };
                o.lines.push(format!("PING {arg} ({dst}): 56 data bytes"));
                let t = self.send(id, src, dst, "ICMP", 0, None);
                o.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "ICMP", layer: "Network", info: "Echo request".into(), dev: id });
                o.log.extend(t.log.clone());
                if !t.ok {
                    o.lines.push(format!("Request timeout — {}", t.error));
                    o.lines.push("4 packets transmitted, 0 received, 100% packet loss".into());
                    o.traces.push(t);
                    return o;
                }
                let (from, seen_src, nat, ms, hops) = (t.dst_dev.unwrap(), t.seen_src.unwrap(), t.nat, t.ms, t.l3.len());
                // Radio hops on the way decide loss and jitter.
                let (p_loss, jitter, worst) = self.path_air(&t.path);
                // Everyone's apps at the busy hour fill queues on the way.
                let queue = self.queue_ms(&self.loads(), &t.path);
                o.traces.push(t);
                let back = self.send(from, dst, seen_src, "ICMP", 0, nat);
                o.log.extend(back.log.clone());
                if !back.ok {
                    o.lines.push(format!("Request timeout — the reply got lost: {}", back.error));
                    o.traces.push(back);
                    return o;
                }
                let rtt = ms + back.ms + queue;
                o.traces.push(back);
                o.log.push(Log { src: dst.to_string(), dst: src.to_string(), proto: "ICMP", layer: "Network", info: "Echo reply".into(), dev: id });
                // Live: each packet takes its own chance (reproducible from the plan's seed and the clock).
                // Planning: the expected outcome.
                let tick = (self.clock * 1000.0) as u64;
                let expected_lost = (4.0 * p_loss).round() as usize;
                let mut lost = 0;
                for s in 1..=4usize {
                    let gone = if self.live { crate::sim::unit(self.seed, tick, s as u64) < p_loss } else { s <= expected_lost };
                    if gone {
                        lost += 1;
                        o.lines.push(format!("Request timeout for icmp_seq={s}{}", worst.map(|w| format!(" (Wi-Fi at {w:.0} dBm)")).unwrap_or_default()));
                    } else {
                        // Waiting for airtime adds random delay (exponential back-off).
                        let wait = if self.live { -(crate::sim::unit(self.seed, tick, 100 + s as u64).max(1e-4)).ln() * jitter } else { jitter };
                        o.lines.push(format!("64 bytes from {dst}: icmp_seq={s} ttl={} time={:.1} ms", 64 - hops.min(63), rtt + wait));
                    }
                }
                o.lines.push(format!("4 packets transmitted, {} received, {}% packet loss", 4 - lost, lost * 25));
            }
            "traceroute" | "tracert" | "tracepath" => {
                let Some(dst) = self.resolve(id, arg, &mut o) else { return o };
                let Some(src) = self.src_ip(id) else { return o };
                o.lines.push(format!("traceroute to {arg} ({dst}), 30 hops max"));
                let t = self.send(id, src, dst, "ICMP", 0, None);
                o.log.extend(t.log.clone());
                for (n, (r, ip)) in t.l3.iter().enumerate() {
                    o.lines.push(format!("{:>2}  {} ({ip})  {:.1} ms", n + 1, self.name(*r), 1.0 + n as f32 * 4.0));
                }
                if t.ok {
                    o.lines.push(format!("{:>2}  {dst}  {:.1} ms   reached", t.l3.len() + 1, t.ms * 2.0));
                } else {
                    o.lines.push(format!("{:>2}  * * *   {}", t.l3.len() + 1, t.error));
                }
                o.traces.push(t);
            }
            "nslookup" | "dig" | "host" => {
                if arg.is_empty() {
                    o.lines.push("usage: nslookup <name>".into());
                } else if let Some(ip) = self.resolve(id, arg, &mut o) {
                    o.lines.push(format!("Server:  {}", d.dns));
                    o.lines.push(format!("Name:    {arg}\nAddress: {ip}"));
                }
            }
            "open" | "curl" | "browse" | "http" | "https" | "test" => {
                let svc = if cmd == "test" { words.get(2).copied().unwrap_or("web") } else { "web" };
                if arg.is_empty() {
                    o.lines.push(format!("usage: {cmd} <name or address>{}", if cmd == "test" { " [web|files|print|dns]" } else { "" }));
                    return o;
                }
                if let Some(text) = self.service(id, arg, svc, &mut o) {
                    o.lines.push(if svc == "web" { crate::html::plain(&text) } else { text });
                }
            }
            "arp" => {
                // Only neighbours it has talked to: ARP asks for a MAC address the first time it's needed.
                let segs = self.segments();
                let known = self.arp_seen.get(&id).cloned().unwrap_or_default();
                let mut any = false;
                for (i, _) in d.ifaces.iter().enumerate() {
                    let Some(&s) = segs.get(&(id, i)) else { continue };
                    for x in &self.devices {
                        for (j, f) in x.ifaces.iter().enumerate() {
                            if x.id != id && segs.get(&(x.id, j)) == Some(&s) && f.addr().is_some_and(|a| known.contains(&a.0)) {
                                o.lines.push(format!("{:<16} {}   {}", f.ip, f.mac, x.name));
                                any = true;
                            }
                        }
                    }
                }
                if !any {
                    o.lines.push("No entries yet: a device learns a neighbour's MAC address (ARP) the first time it talks to it — try a ping first.".into());
                }
            }
            "route" | "netstat" => {
                o.lines.push("Destination        Gateway          Interface".into());
                for f in &d.ifaces {
                    if let Some((ip, p)) = f.addr() {
                        o.lines.push(format!("{:<18} {:<16} {}", format!("{}/{p}", net_of(ip, p)), "direct", f.name));
                    }
                }
                for (n, p, gw) in Self::parse_routes(&d) {
                    o.lines.push(format!("{:<18} {:<16}", format!("{n}/{p}"), gw));
                }
                if !d.gateway.is_empty() {
                    o.lines.push(format!("{:<18} {:<16}", "default (0.0.0.0/0)", d.gateway));
                }
            }
            "dhcp" | "dhclient" | "renew" => {
                if !d.dhcp_client {
                    o.lines.push(format!("{} uses a fixed address. Turn on DHCP in Design mode.", d.name));
                } else {
                    let mut log = vec![];
                    self.dhcp_one(id, &mut log);
                    o.log.extend(log);
                    let d = self.dev(id).unwrap();
                    let f = &d.ifaces[d.dhcp_iface()];
                    o.lines.push(format!("New address: {}/{}  gateway {}  dns {}", or_dash(&f.ip), f.prefix, or_dash(&d.gateway), or_dash(&d.dns)));
                }
            }
            _ => {
                if self.exec_more(id, cmd, arg, &mut o) {
                } else if let Some(x) = self.lookup_dev(cmd) {
                    o.lines.push(format!("Did you mean: ping {}", x.hostname()));
                } else {
                    o.lines.push(format!("Unknown command '{cmd}'. Type help."));
                }
            }
        }
        o
    }
}

fn or_dash(s: &str) -> &str {
    if s.is_empty() { "—" } else { s }
}

/// Ready-made plans.
pub fn template(name: &str) -> Network {
    template_seeded(name, crate::sim::new_seed())
}

/// A ready-made plan whose random hardware comes from `seed` (reproducible).
pub fn template_seeded(name: &str, seed: u64) -> Network {
    let mut n = Network { currency: "€".into(), seed, ..Default::default() };
    match name {
        "home" => {
            n.title = "Our home network".into();
            let inet = n.add_device(Kind::Internet, [120.0, 300.0]);
            let r = n.add_device(Kind::HomeRouter, [340.0, 300.0]);
            let sw = n.add_device(Kind::Switch, [560.0, 160.0]);
            let pc = n.add_device(Kind::Desktop, [780.0, 70.0]);
            let nas = n.add_device(Kind::Nas, [800.0, 190.0]);
            let tv = n.add_device(Kind::Tv, [700.0, 300.0]);
            let lap = n.add_device(Kind::Laptop, [300.0, 500.0]);
            let ph = n.add_device(Kind::Phone, [470.0, 470.0]);
            let plug = n.add_device(Kind::SmartHome, [180.0, 470.0]);
            let ap = n.add_device(Kind::AccessPoint, [620.0, 480.0]);
            let lap2 = n.add_device(Kind::Laptop, [800.0, 520.0]);
            let pr = n.add_device(Kind::Printer, [820.0, 400.0]);
            for (k, nm) in [(pc, "Office PC"), (lap, "Anna's laptop"), (ph, "Phone"), (tv, "Living room TV"), (lap2, "Study laptop"), (plug, "Smart plug"), (ap, "Upstairs Wi-Fi")] {
                n.dev_mut(k).unwrap().name = nm.into();
            }
            let _ = n.connect(inet, r, Medium::Fiber);
            let _ = n.connect(r, sw, Medium::Cat6);
            for x in [pc, nas, tv] {
                let _ = n.connect(sw, x, Medium::Cat6);
            }
            let _ = n.connect(sw, ap, Medium::Cat6a);
            for x in [lap, ph, plug] {
                let _ = n.connect(r, x, Medium::WiFi);
            }
            for x in [lap2, pr] {
                let _ = n.connect(ap, x, Medium::WiFi);
            }
            crate::scenarios::home_walls(&mut n);
            // The switch powers the access point over its cable.
            let swd = n.dev_mut(sw).unwrap();
            (swd.poe_class, swd.poe_budget, swd.model) = (2, 60.0, "8-port PoE+ switch".into());
            // Upstairs: the access point, the study and the printer, with their own partition walls.
            for id in [ap, lap2, pr] {
                n.dev_mut(id).unwrap().floor = 1;
            }
            for (a, b, k) in [([220.0, 20.0], [900.0, 20.0], crate::wifi::WallKind::Brick), ([900.0, 20.0], [900.0, 590.0], crate::wifi::WallKind::Brick), ([900.0, 590.0], [220.0, 590.0], crate::wifi::WallKind::Brick), ([220.0, 590.0], [220.0, 20.0], crate::wifi::WallKind::Brick), ([700.0, 300.0], [900.0, 300.0], crate::wifi::WallKind::Drywall), ([700.0, 300.0], [700.0, 590.0], crate::wifi::WallKind::Drywall)] {
                crate::scenarios::wall(&mut n, a, b, k);
                n.walls.last_mut().unwrap().floor = 1;
            }
            n.story = "A two-storey house: the router sits in the hall, a switch feeds the office and an access point upstairs. Brick outer walls, a concrete stairwell and coated windows shape the Wi-Fi — turn on the Wi-Fi map to see it.".into();
            n.auto_address();
        }
        "flat" | "roaming" | "extender" | "apartment" | "speakers" | "office" | "nightmare" | "campus" => crate::scenarios::build(name, &mut n),
        "classroom" => {
            n.title = "Two networks and a router".into();
            let s1 = n.add_device(Kind::Switch, [260.0, 260.0]);
            let s2 = n.add_device(Kind::Switch, [700.0, 260.0]);
            let r = n.add_device(Kind::Router, [480.0, 260.0]);
            let _ = n.connect(s1, r, Medium::Cat6);
            let _ = n.connect(r, s2, Medium::Cat6);
            for (i, sw) in [(0.0, s1), (1.0, s2)] {
                for j in 0..3 {
                    let pc = n.add_device(Kind::Desktop, [120.0 + i * 700.0 + (j as f32) * 90.0 - 90.0 + i * -40.0, 100.0 + (j as f32) * 160.0]);
                    let _ = n.connect(sw, pc, Medium::Cat6);
                }
            }
            let srv = n.add_device(Kind::Server, [700.0, 450.0]);
            let _ = n.connect(s2, srv, Medium::Cat6);
            {
                let d = n.dev_mut(srv).unwrap();
                d.name = "Web server".into();
                d.services.dns = true;
            }
            n.auto_address();
            let ip = n.dev(srv).unwrap().first_ip().unwrap();
            n.dev_mut(srv).unwrap().dns_records = format!("www.school.test {ip}\nweb-server {ip}");
        }
        _ => {
            n.title = "New plan".into();
        }
    }
    // The example houses were first drawn at a scale where a home came out 30 m wide: shrink them to real sizes.
    let f = match name {
        "home" | "roaming" | "extender" | "nightmare" | "speakers" => 0.45,
        "apartment" => 0.5,
        _ => 1.0,
    };
    if f != 1.0 {
        for d in &mut n.devices {
            d.pos = [d.pos[0] * f, d.pos[1] * f];
        }
        for w in &mut n.walls {
            w.a = [w.a[0] * f, w.a[1] * f];
            w.b = [w.b[0] * f, w.b[1] * f];
        }
    }
    n.scenario = name.into();
    n.assign_ports();
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(n: &mut Network, name: &str, cmd: &str) -> String {
        let id = n.lookup_dev(name).unwrap().id;
        n.exec(id, cmd).lines.join("\n")
    }

    #[test]
    fn leases_follow_the_mac_and_arp_learns_from_traffic() {
        let mut n = template("home");
        let pc = n.lookup_dev("office-pc").unwrap().id;
        let ip = n.dev(pc).unwrap().ifaces[0].ip.clone();
        n.run_dhcp();
        assert_eq!(n.dev(pc).unwrap().ifaces[0].ip, ip);
        n.dev_mut(pc).unwrap().ifaces[0].ip.clear();
        n.run_dhcp();
        assert_eq!(n.dev(pc).unwrap().ifaces[0].ip, ip);
        assert!(run(&mut n, "office-pc", "arp").contains("No entries"));
        run(&mut n, "office-pc", "ping example.com");
        assert!(run(&mut n, "office-pc", "arp").contains("Home router"));
        // Speed tests report both directions; nothing from the internet gets in without a forwarding rule.
        assert!(run(&mut n, "office-pc", "speedtest").contains("Upload"));
        let wan = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().ifaces[1].ip.clone();
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        assert!(n.exec(inet, &format!("open {wan}")).lines.join(" ").contains("forwarding"));
    }

    #[test]
    fn home_template_works_end_to_end() {
        let mut n = template("home");
        assert!(n.check().iter().all(|i| i.level != Level::Error), "{:?}", n.check());
        let out = run(&mut n, "office-pc", "ping example.com");
        assert!(out.contains("0% packet loss") && !out.contains("100%"), "{out}");
        let out = run(&mut n, "study-laptop", "ping nas-1");
        assert!(out.contains("0% packet loss") && !out.contains("100%"), "{out}");
        let out = run(&mut n, "phone", "open example.com");
        assert!(out.contains("200 OK"), "{out}");
        let out = run(&mut n, "phone", "test nas-1 files");
        assert!(out.contains("open"), "{out}");
    }

    #[test]
    fn classroom_routes_between_networks() {
        let mut n = template("classroom");
        assert!(n.check().iter().all(|i| i.level != Level::Error), "{:?}", n.check());
        let out = run(&mut n, "pc-1", "open www.school.test");
        assert!(out.contains("200 OK"), "{out}");
        let out = run(&mut n, "pc-1", "traceroute web-server");
        assert!(out.contains("Router") && out.contains("reached"), "{out}");
    }

    #[test]
    fn damaged_plan_files_are_repaired_not_fatal() {
        let mut n = template("home");
        n.links.push(Link { id: 900, a: 9999, a_if: 0, b: 1, b_if: 0, medium: Medium::Cat6, length_m: None, unplugged: false, a_port: None, b_port: None, kit: String::new() });
        let r = n.devices.iter().position(|d| d.kind == Kind::HomeRouter).unwrap();
        n.devices[r].ifaces.truncate(1);
        n.devices[r].poe_class = 77;
        let pc = n.devices.iter().position(|d| d.kind == Kind::Desktop).unwrap();
        n.devices[pc].ifaces.clear();
        let json = serde_json::to_string(&n).unwrap();
        let mut back: Network = serde_json::from_str(&json).unwrap();
        back.sanitize();
        assert!(back.links.iter().all(|l| l.a != 9999));
        let _ = back.check();
        let _ = back.stp();
        let _ = back.loads();
    }

    #[test]
    fn far_servers_are_slow_to_answer_and_limit_one_download() {
        let mut n = template("home");
        let rtt = |out: &str| out.split("time=").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<f32>().ok()).unwrap();
        let near = rtt(&run(&mut n, "office-pc", "ping example.com"));
        let far = rtt(&run(&mut n, "office-pc", "ping far.example.net"));
        assert!(near < 60.0 && far > 140.0, "near {near}, far {far}");
        let out = run(&mut n, "office-pc", "speedtest far.example.net");
        assert!(out.contains("One single download"), "{out}");
    }

    #[test]
    fn real_websites_go_through_the_simulated_network() {
        let mut n = template("home");
        assert!(is_public_name("wikipedia.org") && !is_public_name("nas-1.home.arpa") && !is_public_name("www.school.test") && !is_public_name("nas-1"));
        let pc = n.lookup_dev("office-pc").unwrap().id;
        let mut o = Outcome::default();
        assert!(n.service(pc, "wikipedia.org/wiki/Router", "web", &mut o).is_some(), "{:?}", o.lines);
        let (url, rtt) = o.fetch.clone().unwrap();
        assert_eq!(url, "https://wikipedia.org/wiki/Router");
        assert!(rtt > 2.0 * REAL_MS, "{rtt}");
        // Unplug the internet: the real site is out of reach like any other.
        let up = n.links.iter().position(|l| n.dev(l.a).unwrap().kind == Kind::Internet || n.dev(l.b).unwrap().kind == Kind::Internet).unwrap();
        n.links[up].unplugged = true;
        let mut o = Outcome::default();
        assert!(n.service(pc, "wikipedia.org", "web", &mut o).is_none() && o.fetch.is_none());
        // The internet device itself can open them too.
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        n.links[up].unplugged = false;
        let mut o = Outcome::default();
        assert!(n.service(inet, "wikipedia.org", "web", &mut o).is_some() && o.fetch.is_some(), "{:?}", o.lines);
        // Home names still never leave the house.
        let mut o = Outcome::default();
        n.links[up].unplugged = false;
        assert!(n.service(pc, "nas-1", "web", &mut o).is_none() || o.fetch.is_none());
    }

    #[test]
    fn internet_drops_private_addresses() {
        let mut n = template("home");
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        let out = n.exec(inet, "ping 192.168.1.10").lines.join("\n");
        assert!(out.contains("100%"), "{out}");
    }

    #[test]
    fn a_damaged_save_with_repeated_or_huge_ids_is_repaired() {
        let mut n = template("flat");
        let w = n.walls[0].clone();
        n.walls.push(w);
        let mut l = n.links[0].clone();
        n.links.push(l.clone());
        l.id = u32::MAX;
        n.links.push(l);
        n.points.push(crate::companion::MeasurePoint { id: 7, floor: -128, ..Default::default() });
        n.sanitize();
        let walls: std::collections::HashSet<u32> = n.walls.iter().map(|w| w.id).collect();
        let links: std::collections::HashSet<u32> = n.links.iter().map(|l| l.id).collect();
        assert_eq!((walls.len(), links.len()), (n.walls.len(), n.links.len()), "every id is unique again");
        assert!(n.links.iter().all(|l| l.id < u32::MAX) && n.points.iter().all(|p| p.floor >= -3));
        n.add_device(Kind::Desktop, [0.0, 0.0]);
    }
}