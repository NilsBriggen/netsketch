//! Network model, address planning, problem checks, the packet simulation (layer headers, TCP, RIP, MAC learning) and the terminal commands.
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
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
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
        crate::i18n::tr(self.blurb_en())
    }
    fn blurb_en(self) -> &'static str {
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
    /// Which network services this kind of device can realistically run: "dhcp", "dns", "web", "files", "print", "mail".
    pub fn offers(self, svc: &str) -> bool {
        use Kind::*;
        match svc {
            "dhcp" => matches!(self, Server | Router | HomeRouter | Internet | Desktop),
            "dns" => matches!(self, Server | Router | HomeRouter | Internet | Desktop | Nas),
            "web" => matches!(self, Desktop | Laptop | Server | Nas | Printer),
            "files" => matches!(self, Desktop | Laptop | Server | Nas),
            "print" => self == Printer,
            "mail" => matches!(self, Server | Desktop | Nas),
            _ => false,
        }
    }
    pub fn serves(self) -> bool {
        ["dhcp", "dns", "web", "files", "print", "mail"].iter().any(|s| self.offers(s))
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
            // Its own backup to the cloud, and downloads straight onto it.
            Kind::Nas | Kind::Server => &[Backup, Download],
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
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
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
        crate::i18n::tr(self.speed_en())
    }
    fn speed_en(self) -> &'static str {
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
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
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
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
        match self {
            Security::Open => "Open (no password)",
            Security::Wep => "WEP (broken)",
            Security::Wpa2 => "WPA2",
            Security::Wpa3 => "WPA3 (recommended)",
            Security::Wpa3Ent => "WPA3-Enterprise (802.1X: own login per person)",
        }
    }
}

/// How a smart-home gadget talks: Wi-Fi like a laptop, or a low-power 802.15.4 mesh on 2.4 GHz through a hub.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SmartLink {
    #[default]
    WiFi,
    /// IPv6 mesh (Matter over Thread) through a Thread border router.
    Thread,
    /// Its own non-IP network through a Zigbee hub.
    Zigbee,
}

impl SmartLink {
    pub const ALL: [SmartLink; 3] = [SmartLink::WiFi, SmartLink::Thread, SmartLink::Zigbee];
    pub fn label(self) -> &'static str {
        match self {
            SmartLink::WiFi => "Wi-Fi",
            SmartLink::Thread => "Thread",
            SmartLink::Zigbee => "Zigbee",
        }
    }
}

/// Which traffic a full link serves first (Ord: high < normal < low).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Qos {
    High,
    #[default]
    Normal,
    Low,
}

impl Qos {
    pub const ALL: [Qos; 3] = [Qos::High, Qos::Normal, Qos::Low];
    pub fn label(self) -> &'static str {
        match self {
            Qos::High => "High",
            Qos::Normal => "Normal",
            Qos::Low => "Low",
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
    /// Email: SMTP (port 587) to send in, IMAP (993) to read the inbox.
    pub mail: bool,
    /// Routers: share routes with neighbour routers by RIP.
    pub rip: bool,
    /// A host's own firewall (its `rules`), when the Firewall software is installed.
    pub firewall: bool,
    /// When RIP was switched on (simulation clock, s); None = it has been running long enough to settle.
    #[serde(skip)]
    pub rip_since: Option<f64>,
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
    /// Older files kept the web server's pages here; `sanitize` moves them into `files` (webserver/…).
    #[serde(skip_serializing)]
    pub web_page: String,
    #[serde(skip_serializing)]
    pub web_files: Vec<(String, String)>,
    /// The software installed (desktop apps and servers).
    pub installed: Vec<crate::soft::Soft>,
    /// Its files, by path ("webserver/index.html", "shared/notes.txt").
    pub files: Vec<(String, String)>,
    /// A mail server: the user names it takes mail for (none = anyone), and the mail delivered to it.
    pub mailboxes: Vec<String>,
    pub inbox: Vec<crate::soft::Mail>,
    /// The mail program's account.
    pub mail_account: Option<crate::soft::MailAccount>,
    pub ssid: String,
    pub wifi: WifiStd,
    pub security: Security,
    pub price: f32,
    pub notes: String,
    /// Already in the home, so not on the shopping list; None = by kind (see `Device::owned`).
    pub owned: Option<bool>,
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
    /// Picks its own channel (automatic channel selection), as almost every router does out of the box.
    pub auto_ch: bool,
    /// A network this device sends by itself, besides being a client: a printer's Wi-Fi Direct, a setup hotspot.
    pub own_net: Option<crate::wifi::OwnNet>,
    /// Kick clients weaker than this (dBm, 0 = off).
    pub min_rssi: i16,
    /// Neighbour networks: share of airtime they use (0–1).
    pub busy: f32,
    /// Internet: the plan's download and upload speed in Mbit/s (upload 0 = as fast as the download).
    pub plan_mbps: f32,
    pub plan_up_mbps: f32,
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
    /// Custom traffic: the apps it runs in your own test, all at once and at full rate ("what if everyone streams?").
    pub test_uses: Vec<crate::extras::Use>,
    /// Custom traffic: an own load on top of those apps, Mbit/s (down, up) to and from the internet.
    pub test_mbps: [f32; 2],
    /// Where that own load goes: a device in the house (its id), or 0 for the internet.
    pub test_to: u32,
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
    /// Height (m) of the antenna above its storey's floor: desks and hands about 1 m, ceiling access points 2.5 m.
    pub mount: f32,
    /// Wi-Fi 7 Multi-Link Operation: sends on two bands at once.
    pub mlo: bool,
    /// Smart-home gadgets: Wi-Fi, Thread or Zigbee.
    pub smart: SmartLink,
    /// Thread border router / Zigbee hub (home routers, speakers and smart-home hubs), on 802.15.4 channel `zb_ch` (11–26).
    pub hub: bool,
    pub zb_ch: u8,
    /// Home router: WireGuard VPN server for remote access, reached at the dynamic DNS name `ddns`.
    pub vpn: bool,
    pub ddns: String,
    /// Home router: UPnP lets consoles and games open ports by themselves.
    pub upnp: bool,
    /// Home router: its DNS answers 0.0.0.0 for ad and tracker names (the starter list plus `blocklist`).
    pub adblock: bool,
    /// Names imported from a real blocklist file, one per line (see `more::parse_blocklist`).
    pub blocklist: String,
    /// Hosts: which traffic goes first when a link is full.
    pub qos: Qos,
    /// Battery (Wh) of the UPS this device is plugged into; 0 = none.
    pub ups_wh: f32,
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
            installed: vec![],
            files: vec![],
            mailboxes: vec![],
            inbox: vec![],
            mail_account: None,
            ssid: String::new(),
            wifi: WifiStd::Wifi6,
            security: Security::Wpa3,
            price: 0.0,
            notes: String::new(),
            owned: None,
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
            auto_ch: true,
            own_net: None,
            min_rssi: 0,
            busy: 0.3,
            plan_mbps: 500.0,
            plan_up_mbps: 0.0,
            ports: vec![crate::ports::PortGroup::rj45(1, 1000)],
            old_ports: 0,
            old_mbps: 0,
            floor: 0,
            roam_at: -75,
            stp_short: false,
            uses: vec![],
            test_uses: vec![],
            test_mbps: [0.0; 2],
            test_to: 0,
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
            mount: 1.0,
            mlo: false,
            smart: SmartLink::WiFi,
            hub: false,
            zb_ch: 15,
            vpn: false,
            ddns: String::new(),
            upnp: false,
            adblock: false,
            blocklist: String::new(),
            qos: Qos::Normal,
            ups_wh: 0.0,
        }
    }
}

impl Device {
    /// Already in the home (left off the shopping list). Unless set, phones, computers, TVs and
    /// the like are; the network gear being planned is not.
    pub fn owned(&self) -> bool {
        use Kind::*;
        self.owned.unwrap_or(matches!(self.kind, Desktop | Laptop | Phone | Server | Printer | Tv | SmartHome | Speaker | Console))
    }

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
    /// A Thread or Zigbee gadget: no Wi-Fi, no DHCP; it talks to a hub over 802.15.4.
    pub fn on_802154(&self) -> bool {
        self.kind == Kind::SmartHome && self.smart != SmartLink::WiFi
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
    /// The way the cable runs (map points between its ends); empty = straight along the walls.
    #[serde(default)]
    pub path: Vec<[f32; 2]>,
    /// Link aggregation (LACP) group: cables between the same two switches with the same non-zero number are one bundle.
    #[serde(default)]
    pub lag: u16,
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

#[derive(Clone, Debug, Default)]
pub struct Log {
    pub src: String,
    pub dst: String,
    pub proto: &'static str,
    pub layer: &'static str,
    pub info: String,
    pub dev: u32,
    /// The packet's headers, layer by layer, where this line knows them (for the layer view).
    pub hdr: Hdr,
}

/// What a frame carries on each layer. Empty / zero = not known for this line.
#[derive(Clone, Debug, Default)]
pub struct Hdr {
    /// Ethernet: sender and receiver MAC on this hop, and the VLAN tag (0 = untagged).
    pub mac: Option<(String, String)>,
    pub vlan: u16,
    /// IPv4: sender, receiver, time to live.
    pub ip: Option<(Ipv4Addr, Ipv4Addr, u8)>,
    /// "TCP", "UDP" or "ICMP", with ports, TCP flags, sequence and acknowledgement numbers, payload bytes.
    pub l4: &'static str,
    pub ports: (u16, u16),
    pub flags: &'static str,
    pub seq: u32,
    pub ack: u32,
    pub len: u32,
    /// The application's message in a few words.
    pub app: String,
}

impl Hdr {
    /// Takes every field this header doesn't know yet from `o`.
    fn fill(&mut self, o: &Hdr) {
        if self.mac.is_none() {
            (self.mac, self.vlan) = (o.mac.clone(), o.vlan);
        }
        self.ip = self.ip.or(o.ip);
        if self.l4.is_empty() {
            self.l4 = o.l4;
        }
        if self.ports == (0, 0) {
            (self.ports, self.flags, self.seq, self.ack, self.len) = (o.ports, o.flags, o.seq, o.ack, o.len);
        }
        if self.app.is_empty() {
            self.app = o.app.clone();
        }
    }
}

impl Log {
    /// The frame layer by layer, outermost first: (layer, what it says).
    pub fn layers(&self) -> Vec<(&'static str, String)> {
        let h = &self.hdr;
        let mut v = vec![];
        if let Some((s, d)) = &h.mac {
            let kind = if self.proto == "ARP" { "0x0806 ARP" } else { "0x0800 IPv4" };
            let tag = if h.vlan > 1 { format!(" · 802.1Q tag VLAN {}", h.vlan) } else { String::new() };
            v.push(("Ethernet", format!("{s} → {d} · EtherType {kind}{tag}")));
        }
        if let Some((s, d, ttl)) = h.ip {
            v.push(("IP", format!("{s} → {d} · TTL {ttl} · protocol {}", if h.l4.is_empty() { "?" } else { h.l4 })));
        }
        if h.ports != (0, 0) {
            let tcp = if h.l4 == "TCP" { format!(" · flags {} · seq {} · ack {}", h.flags, h.seq, h.ack) } else { String::new() };
            v.push(("Transport", format!("{} port {} → {}{tcp} · {} bytes of data", h.l4, h.ports.0, h.ports.1, h.len)));
        }
        if !h.app.is_empty() {
            v.push(("Application", h.app.clone()));
        }
        v
    }
}

/// Copies a packet's own fields (transport, application) into the hop lines of its trace,
/// and the first hop's frame (Ethernet, IP) into the packet's headline.
fn carry(head: &mut Log, hops: &mut [Log]) {
    if let Some(first) = hops.iter().find(|l| l.hdr.ip.is_some()) {
        head.hdr.fill(&first.hdr.clone());
    }
    for l in hops.iter_mut().filter(|l| l.hdr.ip.is_some()) {
        l.hdr.fill(&head.hdr);
    }
}

/// One TCP connection: both ends and their next sequence numbers. Each segment moves its sender's
/// number on by the bytes it carries (SYN and FIN count as one).
pub struct Tcp {
    pub client: (Ipv4Addr, u16, u32),
    pub server: (Ipv4Addr, u16, u32),
    /// Next sequence number of the client and of the server.
    pub cs: u32,
    pub ss: u32,
    /// Ethernet/IP of the first hop each way, for the lines without a trace of their own.
    fwd: Hdr,
    back: Hdr,
    /// Where it arrived (an internet site, a real website), and the time there and back so far (ms).
    pub site: Option<usize>,
    pub real: bool,
    pub ms: f32,
    /// The client's address as the server sees it, and the NAT the reply must undo.
    seen: Ipv4Addr,
    nat: Option<(u32, Ipv4Addr)>,
}

impl Tcp {
    fn seg(&mut self, o: &mut Outcome, from_client: bool, flags: &'static str, len: u32, proto: &'static str, info: String) {
        let ((sip, sp, sdev), (dip, dp, _)) = if from_client { (self.client, self.server) } else { (self.server, self.client) };
        let (seq, ack) = if from_client { (self.cs, self.ss) } else { (self.ss, self.cs) };
        let step = len + flags.contains("SYN") as u32 + flags.contains("FIN") as u32;
        if from_client {
            self.cs = self.cs.wrapping_add(step);
        } else {
            self.ss = self.ss.wrapping_add(step);
        }
        let ack = if flags.contains("ACK") { ack } else { 0 };
        let app = if proto == "TCP" { String::new() } else { info.clone() };
        let mut hdr = Hdr { l4: "TCP", ports: (sp, dp), flags, seq, ack, len, app, ..Default::default() };
        hdr.fill(if from_client { &self.fwd } else { &self.back });
        let layer = if proto == "TCP" { "Transport" } else { "Application" };
        o.log.push(Log { src: format!("{sip}:{sp}"), dst: format!("{dip}:{dp}"), proto, layer, info, dev: sdev, hdr });
    }

    /// FIN, ACK both ways and the last ACK: the connection closes politely.
    fn close(&mut self, o: &mut Outcome) {
        self.seg(o, true, "FIN, ACK", 0, "TCP", "FIN — I'm done sending".into());
        self.seg(o, false, "FIN, ACK", 0, "TCP", "FIN — me too".into());
        self.seg(o, true, "ACK", 0, "TCP", "ACK — connection closed".into());
    }
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
    /// (switch, MAC address, cable it arrived on) for every frame a switch passed on.
    pub macs: Vec<(u32, String, u32)>,
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
    /// Wall kinds whose loss was learned from the measurements: a factor on the sourced value (1 = as published).
    pub wall_fit: Vec<(crate::wifi::WallKind, f32)>,
    /// What the phone watched over time, the problems it found and the devices it discovered.
    pub phone: crate::companion::PhoneReport,
    /// Room names written on the map, and names people gave their storeys ("Loft").
    pub labels: Vec<RoomLabel>,
    pub storey_names: Vec<(i8, String)>,
    /// Storey heights (m, floor to floor) where they differ from 2.8 m.
    pub storey_heights: Vec<(i8, f32)>,
    /// Garden, terrace or yard: places outside the walls where Wi-Fi should reach too.
    pub outdoor: Vec<crate::plan::Outdoor>,
    /// Electricity price per kWh, for the yearly running cost.
    pub kwh_price: f32,
    /// Extra cable on top of each estimated run (0.1 = 10 %): detours, a loop to spare, cutting waste.
    pub cable_slack: f32,
    /// Which country's Wi-Fi rules apply (channels, power).
    pub region: crate::wifi::Region,
    /// Time of day (hours) the traffic is looked at; 20.5 = the evening busy hour.
    pub hour: f32,
    /// How busy the household is: every app's demand times this (1 = a typical day, 2 = everyone home at once).
    pub traffic: f32,
    /// Custom traffic instead of the day's usual: each device runs its `test_uses` at full rate, whatever the hour.
    pub custom_traffic: bool,
    /// Custom traffic comes and goes like real apps in Simulate (page loads, video chunks) instead of staying steady.
    pub custom_bursty: bool,
    /// The traffic model's memory between frames (not saved).
    #[serde(skip)]
    pub memory: std::cell::RefCell<crate::extras::TrafficMemory>,
    /// Where signals bend around walls, kept until the walls change (not saved).
    #[serde(skip)]
    pub edge_cache: std::cell::RefCell<crate::wifi::EdgeCache>,
    /// Path losses already worked out (pure geometry; not saved).
    #[serde(skip)]
    pub path_cache: std::cell::RefCell<crate::wifi::PathCache>,
    /// Results kept for one UI frame (see `wifi::Memo`); not saved.
    #[serde(skip)]
    pub memo: std::cell::RefCell<crate::wifi::Memo>,
    /// Live Wi-Fi links that lost their access point: (signal gone since, joining again until), clock s; not saved.
    #[serde(skip)]
    pub link_down: std::cell::RefCell<HashMap<u32, (f64, f64)>>,
    /// Signal (dBm) that counts as good Wi-Fi in this home: −72 browsing, −67 video calls, −60 4K and gaming.
    /// The coverage score, the Wi-Fi map colours and its legend all follow it.
    pub target: f32,
    /// Simulation time (s) and whether live variation (fading, traffic) is on; not saved.
    #[serde(skip)]
    pub clock: f64,
    #[serde(skip)]
    pub live: bool,
    /// Addresses each device learned a MAC address for from traffic (its ARP cache); not saved.
    #[serde(skip)]
    pub arp_seen: HashMap<u32, Vec<Ipv4Addr>>,
    /// MAC addresses each switch learned from traffic, with the cable each came in on; not saved.
    #[serde(skip)]
    pub mac_seen: HashMap<u32, Vec<(String, u32)>>,
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
            wall_fit: vec![],
            plans: vec![],
            survey: vec![],
            models: vec![],
            points: vec![],
            measured: vec![],
            phone: Default::default(),
            labels: vec![],
            storey_names: vec![],
            storey_heights: vec![],
            outdoor: vec![],
            kwh_price: 0.30,
            cable_slack: 0.1,
            region: crate::wifi::Region::EuCh,
            hour: 20.5,
            traffic: 1.0,
            custom_traffic: false,
            custom_bursty: false,
            memory: Default::default(),
            edge_cache: Default::default(),
            path_cache: Default::default(),
            memo: Default::default(),
            link_down: Default::default(),
            target: -67.0,
            clock: 0.0,
            live: false,
            arp_seen: HashMap::new(),
            mac_seen: HashMap::new(),
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
    /// Storeys with anything on them: devices, walls, labels, pins, a floor-plan picture or an outdoor area.
    pub fn used_floors(&self) -> std::collections::BTreeSet<i8> {
        let n = self;
        n.devices.iter().map(|d| d.floor).chain(n.walls.iter().map(|w| w.floor)).chain(n.labels.iter().map(|l| l.floor)).chain(n.points.iter().map(|p| p.floor)).chain(n.plans.iter().map(|p| p.floor)).chain(n.outdoor.iter().map(|o| o.floor)).collect()
    }

    /// A storey's own name if someone gave it one, else "Ground floor", "1st floor"…
    pub fn storey(&self, f: i8) -> String {
        self.storey_names.iter().find(|s| s.0 == f && !s.1.trim().is_empty()).map(|s| s.1.clone()).unwrap_or_else(|| floor_name(f))
    }

    /// A storey's height (m, floor to floor).
    pub fn storey_h(&self, f: i8) -> f32 {
        self.storey_heights.iter().find(|s| s.0 == f).map_or(2.8, |s| s.1)
    }

    /// Height (m) of a storey's floor above the ground floor's.
    pub fn floor_z(&self, f: i8) -> f32 {
        if f >= 0 { (0..f).map(|s| self.storey_h(s)).sum() } else { -(f..0).map(|s| self.storey_h(s)).sum::<f32>() }
    }

    pub fn add_label(&mut self, pos: [f32; 2], floor: i8, text: &str) -> u32 {
        self.next_id += 1;
        self.labels.push(RoomLabel { id: self.next_id, pos, floor, text: text.into() });
        self.next_id
    }
}

pub fn floor_name(f: i8) -> String {
    if crate::i18n::german() {
        return match f {
            0 => "Erdgeschoss".into(),
            -1 => "Untergeschoss".into(),
            f if f < 0 => format!("{}. Untergeschoss", -f),
            f => format!("{f}. Obergeschoss"),
        };
    }
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
    /// Reads a saved plan. Only a file that lists devices counts: any other JSON (a phone survey, a
    /// settings file) would otherwise open as an empty plan, and saving would write over it.
    pub fn from_json(text: &str) -> Result<Network, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if v.get("devices").is_none_or(|d| !d.is_array()) {
            return Err(crate::i18n::tr("this isn't a NetSketch plan").into());
        }
        serde_json::from_value(v).map_err(|e| e.to_string())
    }

    /// Repairs a plan read from a file so nothing points at devices or ports that don't exist.
    /// Everything a device runs (or came with) is installed: plans from before software, and templates that switch
    /// servers on directly.
    pub fn fit_software(&mut self) {
        for d in &mut self.devices {
            if d.installed.is_empty() {
                d.installed = crate::soft::Soft::defaults(d.kind);
            }
            for s in crate::soft::Soft::ALL {
                let on = match s.service() {
                    Some("web") => d.services.web,
                    Some("dns") => d.services.dns,
                    Some("dhcp") => d.services.dhcp,
                    Some("mail") => d.services.mail,
                    Some("files") => d.services.files,
                    Some("print") => d.services.print,
                    _ => false,
                };
                if on && !d.has(s) {
                    d.installed.push(s);
                }
            }
            d.installed.sort();
            d.installed.dedup();
        }
    }

    pub fn sanitize(&mut self) {
        self.fit_software();
        // Pages of older plans move into the web server's folder.
        for d in &mut self.devices {
            if !d.web_page.is_empty() && d.file("webserver/index.html").is_none() {
                let page = std::mem::take(&mut d.web_page);
                d.set_file("webserver/index.html", &page);
            }
            for (name, body) in std::mem::take(&mut d.web_files) {
                if d.file(&format!("webserver/{name}")).is_none() {
                    d.set_file(&format!("webserver/{name}"), &body);
                }
            }
            d.web_page.clear();
        }
        if !(0.1..=5.0).contains(&self.traffic) {
            self.traffic = 1.0;
        }
        if !(-80.0..=-50.0).contains(&self.target) {
            self.target = -67.0;
        }
        // Positions that aren't real numbers (1e39 reads as infinity) would blank the whole map.
        let ok = |p: &[f32; 2]| p.iter().all(|v| v.is_finite());
        self.devices.retain(|d| ok(&d.pos));
        self.walls.retain(|w| ok(&w.a) && ok(&w.b));
        self.labels.retain(|l| ok(&l.pos));
        self.points.retain(|p| ok(&p.pos));
        self.wall_fit.retain(|f| f.1.is_finite());
        for f in &mut self.wall_fit {
            f.1 = f.1.clamp(0.5, 2.0);
        }
        self.survey.retain(|p| ok(&p.pos) && p.rssi.is_finite());
        self.plans.retain(|p| ok(&p.pos) && p.width_m.is_finite() && p.width_m > 0.0);
        for l in self.labels.iter_mut() {
            l.floor = l.floor.clamp(-3, 20);
        }
        for p in self.plans.iter_mut() {
            p.floor = p.floor.clamp(-3, 20);
        }
        for p in self.survey.iter_mut() {
            p.floor = p.floor.clamp(-3, 20);
        }
        for s in self.storey_names.iter_mut() {
            s.0 = s.0.clamp(-3, 20);
        }
        for s in self.storey_heights.iter_mut() {
            s.0 = s.0.clamp(-3, 20);
        }
        for d in self.devices.iter_mut() {
            crate::wifi::normalize_radio(d);
            d.floor = d.floor.clamp(-3, 20);
            d.mount = if d.mount.is_finite() { d.mount.clamp(0.0, 10.0) } else { 1.0 };
            d.ups_wh = if d.ups_wh.is_finite() { d.ups_wh.clamp(0.0, 100_000.0) } else { 0.0 };
            d.zb_ch = d.zb_ch.clamp(11, 26);
        }
        self.storey_heights.retain(|s| s.1.is_finite());
        for s in self.storey_heights.iter_mut() {
            s.1 = s.1.clamp(2.0, 10.0);
        }
        for l in self.links.iter_mut() {
            l.path.retain(|p| p.iter().all(|v| v.is_finite()));
            l.path.truncate(200);
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
        self.outdoor.retain(|o| o.id < MAX_ID);
        let mut seen = std::collections::HashSet::new();
        self.devices.retain(|d| d.id < MAX_ID && seen.insert(d.id));
        self.next_id = self.next_id.max(self.devices.iter().map(|d| d.id).max().unwrap_or(0)).max(self.walls.iter().map(|w| w.id).max().unwrap_or(0))
            .max(self.points.iter().map(|p| p.id).max().unwrap_or(0))
            .max(self.labels.iter().map(|l| l.id).max().unwrap_or(0))
            .max(self.outdoor.iter().map(|o| o.id).max().unwrap_or(0));
        let ids: std::collections::HashSet<u32> = self.devices.iter().map(|d| d.id).collect();
        self.links.retain(|l| ids.contains(&l.a) && ids.contains(&l.b) && l.a != l.b);
        self.measured.retain(|m| ids.contains(&m.1));
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
            d.plan_up_mbps = if d.plan_up_mbps.is_finite() { d.plan_up_mbps.clamp(0.0, 25000.0) } else { 0.0 };
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
                        let page = format!("Welcome to {}!", d.name);
                        d.set_file("webserver/index.html", &page);
                    }
                    Kind::Nas => {
                        d.services.files = true;
                        d.services.web = true;
                        d.set_file("webserver/index.html", "Storage admin page");
                        d.set_file("shared/readme.txt", "Files in this folder are shared with the whole network.");
                    }
                    Kind::Printer => {
                        d.services.print = true;
                        d.services.web = true;
                        d.set_file("webserver/index.html", "Printer status: ready");
                    }
                    _ => {}
                }
            }
        }
        if let Some(p) = crate::wifi::default_preset(kind) {
            p.apply(&mut d);
        }
        d.installed = crate::soft::Soft::defaults(kind);
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
        let (ka, kb) = (self.dev(a).ok_or("gone")?.kind, self.dev(b).ok_or("gone")?.kind);
        // A mobile backup line may run next to the main internet line, and switches may take several cables
        // (a backup, or a bundle); anything else only once.
        let spare = ka == Kind::Switch && kb == Kind::Switch && medium.cable();
        if self.links.iter().any(|l| ((l.a == a && l.b == b) || (l.a == b && l.b == a)) && (medium != Medium::Cellular || l.medium == Medium::Cellular) && !(spare && l.medium.cable())) {
            return Err("These two are already connected.".into());
        }
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
        self.links.push(Link { id, a, a_if, b, b_if, medium, length_m: None, unplugged: false, a_port: None, b_port: None, kit: String::new(), path: vec![], lag: 0 });
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
        // Measured networks matched to it are free again (adding it back re-matches them).
        self.measured.retain(|m| m.1 != id);
    }

    pub fn link_len(&self, l: &Link) -> f32 {
        if let Some(m) = l.length_m {
            return m;
        }
        match (self.dev(l.a), self.dev(l.b)) {
            (Some(a), Some(b)) => {
                let (dx, dy) = ((a.pos[0] - b.pos[0]).abs(), (a.pos[1] - b.pos[1]).abs());
                let storeys = (self.floor_z(a.floor) - self.floor_z(b.floor)).abs();
                // At each end the cable comes up (or down) to the device: at least a metre, more to an access
                // point high on a wall or the ceiling; then the plan's slack on top.
                let ends = a.mount.max(1.0) + b.mount.max(1.0);
                let slack = 1.0 + self.cable_slack.clamp(0.0, 1.0);
                if l.medium.cable() && !l.path.is_empty() {
                    // A drawn route: along its points, up through floors.
                    let pts: Vec<[f32; 2]> = std::iter::once(a.pos).chain(l.path.iter().copied()).chain(std::iter::once(b.pos)).collect();
                    let run: f32 = pts.windows(2).map(|w| ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt()).sum();
                    ((run / PX_PER_M + storeys + ends) * slack).round().max(1.0)
                } else if l.medium.cable() || l.medium == Medium::Powerline {
                    // Cables run along walls and ceilings at right angles, up through floors.
                    let f = if l.medium == Medium::Powerline { 1.0 } else { slack };
                    (((dx + dy) / PX_PER_M + storeys + ends) * f).round().max(1.0)
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
    fn route(d: &Device, dst: Ipv4Addr, rip: &[(Ipv4Addr, u8, Ipv4Addr, u8)]) -> Option<(usize, Ipv4Addr)> {
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
            // Typed routes win over learned ones (as a real router trusts static routes more than RIP).
            if best.is_none() {
                for (net, p, gw, _) in rip {
                    if same_net(*net, dst, *p) && best.is_none_or(|b| *p > b.0) {
                        best = Some((*p, *gw));
                    }
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
        let rip = self.rip_routes().0;
        let l4 = match proto {
            "ICMP" => "ICMP",
            "DNS" | "DHCP" | "UDP" => "UDP",
            _ => "TCP",
        };
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
        for ttl in (1..=64u8).rev() {
            let d = self.dev(cur).unwrap();
            if d.off {
                t.error = format!("{} is switched off.", d.name);
                return t;
            }
            if let Some((r, inner)) = nat_back {
                if r == cur && d.has_ip(dst) {
                    if let Some(cg) = self.cgnat(d) {
                        t.log.push(Log { src: src.to_string(), dst: cg.clone(), proto: "NAT", layer: "Network", info: format!("The provider's carrier-grade NAT maps the reply for {dst} back to {cg}"), dev: cur, ..Default::default() });
                    }
                    t.log.push(Log { src: src.to_string(), dst: inner.to_string(), proto: "NAT", layer: "Network", info: format!("{} translates the reply back to {inner}", d.name), dev: cur, ..Default::default() });
                    dst = inner;
                    nat_back = None;
                }
            }
            // Unsolicited traffic from outside: dropped unless a port forwarding rule passes it on to a device inside.
            if d.kind == Kind::HomeRouter && came_in == Some(1) && !reply && d.ifaces.get(1).and_then(|f| f.addr()).map(|a| a.0) == Some(dst) {
                let shared = self.cgnat(d);
                match (self.forward_of(d, port, proto), shared) {
                    (_, Some(cg)) => {
                        t.error = format!("{} is on the mobile line, behind the provider's carrier-grade NAT: it only has the shared address {cg}, so nothing from the internet reaches it and port forwarding can't work. IPv6 or a VPN/tunnel service gets around it.", d.name);
                        t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("Dropped at the provider's carrier-grade NAT: {} only has the shared address {cg}", d.name), dev: cur, ..Default::default() });
                        return t;
                    }
                    (Some(inner), None) => {
                        t.log.push(Log { src: src.to_string(), dst: inner.to_string(), proto: "NAT", layer: "Network", info: format!("{} forwards port {port} to {inner}", d.name), dev: cur, ..Default::default() });
                        dst = inner;
                    }
                    (None, None) => {
                        t.error = format!("{} drops it: nobody inside started this conversation and no port forwarding rule passes port {port} on.", d.name);
                        t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("{} drops unsolicited traffic from the internet (no forwarding rule for port {port})", d.name), dev: cur, ..Default::default() });
                        return t;
                    }
                }
            }
            // A computer's own firewall: its rules decide on what it sends and on what arrives for it (replies to
            // what it started always get back in).
            if d.running(crate::soft::Soft::Firewall) && !reply && (cur == from) != d.has_ip(dst) {
                if let Some((false, rule)) = self.rule_verdict(d, src, dst, port, proto) {
                    let way = if cur == from { "outgoing" } else { "incoming" };
                    t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("Dropped by {}'s firewall ({way}): '{rule}'", d.name), dev: cur, ..Default::default() });
                    t.error = format!("Blocked by {}'s own firewall rule '{rule}'.", d.name);
                    return t;
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
            let Some((out, hop)) = Self::route(d, dst, rip.get(&cur).map_or(&[][..], |v| v)) else {
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
            let my_mac = mac_of(d, out);
            let arp_hdr = Hdr { mac: Some((my_mac.clone(), "ff:ff:ff:ff:ff:ff".into())), vlan: d.ifaces[out].vlan, app: format!("ARP request: who has {hop}?"), ..Default::default() };
            t.log.push(Log { src: d.name.clone(), dst: "broadcast".into(), proto: "ARP", layer: "Link", info: format!("Who has {hop}? Tell {}", d.ifaces[out].ip), dev: cur, hdr: arp_hdr });
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
                if let Some((false, rule)) = self.rule_verdict(d, src, dst, port, proto) {
                    t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("Dropped by {}'s firewall rule '{rule}'", d.name), dev: cur, ..Default::default() });
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
                t.log.push(Log { src: d.name.clone(), dst: self.name(next), proto: "802.1Q", layer: "Link", info: format!("Frame tagged with VLAN {} on the trunk", d.ifaces[out].vlan), dev: cur, ..Default::default() });
            }
            t.ms += ms;
            // Every switch on the way learns where both ends are (the frame one way, the ARP reply the other).
            let next_mac = self.dev(next).map(|x| mac_of(x, next_if)).unwrap_or_default();
            for w in path.windows(3) {
                if self.dev(w[1]).is_some_and(|x| x.kind.bridges() || x.kind == Kind::HomeRouter) {
                    for (m, nb) in [(&my_mac, w[0]), (&next_mac, w[2])] {
                        if let Some(l) = self.link_between(w[1], nb) {
                            t.macs.push((w[1], m.clone(), l.id));
                        }
                    }
                }
            }
            t.path.extend(path.into_iter().skip(1));
            if d.kind == Kind::HomeRouter && out == 1 && came_in.is_some_and(|i| i != 1) {
                let wan = parse_ip(&d.ifaces[1].ip).unwrap_or(src);
                match self.cgnat(d) {
                    // Mobile line: two translations, the router's own and then the provider's.
                    Some(cg) => {
                        t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("{} replaces sender {src} with its mobile-line address {cg}", d.name), dev: cur, ..Default::default() });
                        t.log.push(Log { src: cg.clone(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("The provider's carrier-grade NAT replaces {cg} with {wan}, a public address many customers share"), dev: cur, ..Default::default() });
                    }
                    None => t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "NAT", layer: "Network", info: format!("{} replaces sender {src} with its public address {wan}", d.name), dev: cur, ..Default::default() }),
                }
                t.nat = Some((cur, src));
                src = wan;
            }
            if cur != from {
                t.l3.push((cur, d.ifaces[came_in.unwrap_or(out)].addr().map(|a| a.0).unwrap_or(hop)));
            }
            let hdr = Hdr { mac: Some((my_mac, next_mac)), vlan: d.ifaces[out].vlan, ip: Some((src, dst, ttl)), l4, ..Default::default() };
            t.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto, layer: "Network", info: format!("{} sends to {} (next hop {hop}, TTL {ttl})", d.name, self.name(next)), dev: cur, hdr });
            cur = next;
            came_in = Some(next_if);
        }
        t.error = "TTL expired: the packet went in circles (routing loop).".into();
        t
    }

    /// The router's own address while its internet runs over the mobile line, from the provider's shared
    /// range 100.64.0.0/10 (carrier-grade NAT), or None on a normal line. Its WAN entry then stands for the
    /// public address the provider's NAT shares among many customers: the router sends from the 100.64
    /// address, the provider maps that to the shared one on the way out and back again for replies, and
    /// unsolicited traffic to the shared address stops at the provider (see `send`).
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
        let mut t = self.send(from, src, dns, "DNS", 53, None);
        // UDP: no connection set up first, just one question and one answer.
        let sport = 49152 + (crate::sim::hash(self.seed, from as u64, (self.clock * 1000.0) as u64) % 16384) as u16;
        let q = format!("Query: A record for {name}?");
        let mut head = Log { src: format!("{src}:{sport}"), dst: format!("{dns}:53"), proto: "DNS", layer: "Application", info: format!("UDP, no handshake — {q}"), dev: from, hdr: Hdr { l4: "UDP", ports: (sport, 53), len: name.len() as u32 + 17, app: q, ..Default::default() } };
        carry(&mut head, &mut t.log);
        out.log.push(head);
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
                out.lines.push(format!("Blocked: {name} is on {}'s filter list ({site}); it answers 0.0.0.0, an address that goes nowhere.", s.name));
                out.log.push(Log { src: s.name.clone(), dst: src.to_string(), proto: "DNS", layer: "Application", info: format!("{name} blocked by the content filter: answer 0.0.0.0"), dev: server, ..Default::default() });
                return None;
            }
            if let Some(ip) = self.answer(s, &name) {
                let a = format!("Answer from {}: {name} is {ip}", s.name);
                out.log.push(Log { src: format!("{dns}:53"), dst: format!("{src}:{sport}"), proto: "DNS", layer: "Application", info: format!("UDP — {a}"), dev: from, hdr: Hdr { l4: "UDP", ports: (53, sport), len: name.len() as u32 + 33, app: a, ..Default::default() } });
                return Some(ip);
            }
            // Forward to the upstream resolver.
            let Some(up) = parse_ip(&s.dns).or_else(|| parse_ip(&s.gateway)) else { break };
            let Some(next) = self.devices.iter().find(|x| x.has_ip(up)) else { break };
            if next.id == server {
                break;
            }
            out.log.push(Log { src: s.name.clone(), dst: up.to_string(), proto: "DNS", layer: "Application", info: format!("{} doesn't know {name}, asks {}", s.name, next.name), dev: server, ..Default::default() });
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
                    // "school.test MX mail.school.test" names a mail server, not an address.
                    if n.eq_ignore_ascii_case(name) && !ip.eq_ignore_ascii_case("MX") {
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

    /// The devices that took an address from a DHCP server: automatic-address devices in one of its networks,
    /// holding an address from its pool (any address in the network when no pool is set).
    pub fn leases(&self, server: u32) -> Vec<(u32, String)> {
        let Some(s) = self.dev(server) else { return vec![] };
        let segs = self.segments();
        let (from, to) = (parse_ip(&s.dhcp_from).map(u32::from), parse_ip(&s.dhcp_to).map(u32::from));
        let mine: Vec<_> = (0..s.ifaces.len()).filter(|j| (s.kind != Kind::HomeRouter || *j != 1) && s.ifaces[*j].addr().is_some()).filter_map(|j| segs.get(&(server, j))).collect();
        self.devices
            .iter()
            .filter(|d| d.id != server && d.dhcp_client)
            .filter_map(|d| {
                let i = d.dhcp_iface();
                let (ip, _) = d.ifaces.get(i)?.addr()?;
                let n = u32::from(ip);
                let in_pool = match (from, to) {
                    (Some(a), Some(b)) => (a..=b).contains(&n),
                    _ => true,
                };
                (in_pool && !ip.is_link_local() && mine.contains(&segs.get(&(d.id, i))?)).then(|| (d.id, ip.to_string()))
            })
            .collect()
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
        log.push(Log { src: "0.0.0.0".into(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("DISCOVER from {name}: is there a DHCP server?"), dev: id, ..Default::default() });
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
            log.push(Log { src: name.clone(), dst: "-".into(), proto: "DHCP", layer: "Application", info: format!("No answer — {name} gives itself the link-local address {ip}"), dev: id, ..Default::default() });
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
            log.push(Log { src: sip.to_string(), dst: name.clone(), proto: "DHCP", layer: "Application", info: format!("{sname} has no free address in its pool (or the pool doesn't match its network)"), dev: sid, ..Default::default() });
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
        log.push(Log { src: sip.to_string(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("OFFER from {sname}: use {ip}/{spfx}, gateway {gw}, DNS {dns}"), dev: sid, ..Default::default() });
        log.push(Log { src: "0.0.0.0".into(), dst: "255.255.255.255".into(), proto: "DHCP", layer: "Application", info: format!("REQUEST from {name}: I'll take {ip}"), dev: id, ..Default::default() });
        log.push(Log { src: sip.to_string(), dst: ip.to_string(), proto: "DHCP", layer: "Application", info: "ACK: the address is yours for 24 h".into(), dev: sid, ..Default::default() });
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

    /// RIP: routers with it on tell their neighbour routers (on a shared network) every network they know,
    /// one hop further each time. Each update round takes 30 s, counted from when RIP was last switched on
    /// somewhere, so a new route spreads one router further per update. Per router (network, prefix, next hop,
    /// metric in hops), the update rounds done, and the seconds until the next one while it hasn't settled.
    /// 16 hops counts as unreachable.
    pub fn rip_state(&self) -> (HashMap<u32, Vec<(Ipv4Addr, u8, Ipv4Addr, u8)>>, usize, Option<f64>) {
        let on: Vec<&Device> = self.devices.iter().filter(|d| d.services.rip && d.kind.routes() && !d.off).collect();
        if on.is_empty() {
            return (HashMap::new(), 0, None);
        }
        // Updates so far: one when RIP starts, then one every 30 s.
        let since = on.iter().filter_map(|d| d.services.rip_since).fold(None, |a: Option<f64>, t| Some(a.map_or(t, |a| a.max(t))));
        let allowed = since.map_or(usize::MAX, |t| ((self.clock - t).max(0.0) / 30.0) as usize + 1);
        let segs = self.segments();
        let own = |d: &Device| -> Vec<(usize, Ipv4Addr, u8)> { d.ifaces.iter().enumerate().filter_map(|(i, f)| Some((*segs.get(&(d.id, i))?, f.addr()?.0, f.addr()?.1))).collect() };
        // Router -> (network, prefix) -> (hops, next hop); its own networks at 0 hops.
        type Table = HashMap<(Ipv4Addr, u8), (u8, Option<Ipv4Addr>)>;
        let mut table: HashMap<u32, Table> = on.iter().map(|d| (d.id, own(d).into_iter().map(|(_, ip, p)| ((net_of(ip, p), p), (0, None))).collect())).collect();
        // (router, neighbour, the neighbour's address on the network they share)
        let mut nbrs = vec![];
        for a in &on {
            for b in on.iter().filter(|b| b.id != a.id) {
                for (sa, _, _) in own(a) {
                    if let Some((_, ip, _)) = own(b).into_iter().find(|x| x.0 == sa) {
                        nbrs.push((a.id, b.id, ip));
                    }
                }
            }
        }
        let mut rounds = 0;
        let mut waiting = None;
        while rounds < 32 {
            if rounds >= allowed {
                // There's more to learn, but the next update hasn't been sent yet.
                let next = since.unwrap_or(0.0) + 30.0 * allowed as f64 - self.clock;
                let pending = nbrs.iter().any(|(r, n, _)| table[n].iter().any(|(k, (m, _))| m + 1 <= 15 && table[r].get(k).is_none_or(|x| m + 1 < x.0)));
                waiting = pending.then_some(next.max(0.0));
                break;
            }
            let heard = table.clone();
            let mut changed = false;
            for (r, n, via) in &nbrs {
                for (k, (m, _)) in &heard[n] {
                    let m = m + 1;
                    let mine = table.get_mut(r).unwrap();
                    if m <= 15 && mine.get(k).is_none_or(|x| m < x.0) {
                        mine.insert(*k, (m, Some(*via)));
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            rounds += 1;
        }
        let out = table
            .into_iter()
            .map(|(r, t)| {
                let mut v: Vec<_> = t.into_iter().filter_map(|((net, p), (m, via))| Some((net, p, via?, m))).collect();
                v.sort();
                (r, v)
            })
            .collect();
        (out, rounds, waiting)
    }

    /// The RIP routes each router has learned by now, and the update rounds it took.
    pub fn rip_routes(&self) -> (HashMap<u32, Vec<(Ipv4Addr, u8, Ipv4Addr, u8)>>, usize) {
        let (r, n, _) = self.rip_state();
        (r, n)
    }

    /// Finds everything that will stop the network from working, or is outdated.
    pub fn check(&self) -> Vec<Issue> {
        crate::wifi::Region::set(self.region);
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
                        push(Level::Error, Some(d.id), None, crate::i18n::trf("{} and {} both use {}. Every address must be unique.", &[&d.name, &(self.name(o)), &ip]));
                    }
                }
            }
        }
        for d in &self.devices {
            let id = Some(d.id);
            // Thread and Zigbee gadgets join their hub by radio instead (issues.rs checks that).
            if d.kind.foreign() || d.on_802154() {
                continue;
            }
            if !self.connected(d.id) {
                push(Level::Warning, id, None, crate::i18n::trf("{} isn't connected to anything.", &[&d.name]));
                continue;
            }
            for f in &d.ifaces {
                if !f.ip.trim().is_empty() && f.addr().is_none() {
                    push(Level::Error, id, None, crate::i18n::trf("{}: '{}' isn't a valid IPv4 address.", &[&d.name, &f.ip]));
                }
                if let Some((ip, p)) = f.addr() {
                    if p == 0 || p > 30 && !(p == 32 && d.kind.is_host()) {
                        push(Level::Warning, id, None, crate::i18n::trf("{}: prefix /{} is unusual; home networks use /24 (255.255.255.0).", &[&d.name, &p]));
                    }
                    if ip.octets()[0] == 169 && ip.octets()[1] == 254 {
                        push(Level::Error, id, None, crate::i18n::trf("{} only has a link-local address ({}) — no DHCP server reached it.", &[&d.name, &ip]));
                    }
                }
            }
            if d.kind.is_host() {
                let Some((ip, p)) = d.ifaces[0].addr() else {
                    if d.dhcp_client {
                        push(Level::Tip, id, None, crate::i18n::trf("{} will get its address by DHCP when the simulation starts.", &[&d.name]));
                    } else {
                        push(Level::Error, id, None, crate::i18n::trf("{} has no IP address. Set one or turn on DHCP.", &[&d.name]));
                    }
                    continue;
                };
                match parse_ip(&d.gateway) {
                    Some(gw) if !same_net(gw, ip, p) => push(Level::Error, id, None, crate::i18n::trf("{}'s gateway {} is outside its own network {}/{}.", &[&d.name, &gw, &(net_of(ip, p)), &p])),
                    Some(gw) => {
                        if let Some(&s) = segs.get(&(d.id, 0)) {
                            if self.owner_in(&segs, s, gw).is_none() {
                                push(Level::Warning, id, None, crate::i18n::trf("{}'s gateway {} doesn't exist in its network.", &[&d.name, &gw]));
                            }
                        }
                    }
                    None if !d.gateway.trim().is_empty() => push(Level::Error, id, None, crate::i18n::trf("{}: gateway '{}' isn't a valid address.", &[&d.name, &d.gateway])),
                    None if self.devices.iter().any(|x| x.kind.routes()) => push(Level::Tip, id, None, crate::i18n::trf("{} has no default gateway, so it can only reach its own network.", &[&d.name])),
                    None => {}
                }
                if !d.dhcp_client {
                    for s in &self.devices {
                        if let (true, Some(f), Some(t)) = (s.services.dhcp && segs.get(&(s.id, 0)) == segs.get(&(d.id, 0)), parse_ip(&s.dhcp_from), parse_ip(&s.dhcp_to)) {
                            if (u32::from(f)..=u32::from(t)).contains(&u32::from(ip)) {
                                push(Level::Warning, id, None, crate::i18n::trf("{}'s fixed address {} is inside {}'s DHCP pool — it may be handed out twice.", &[&d.name, &ip, &s.name]));
                            }
                        }
                    }
                }
            }
            if d.services.dhcp {
                if let (Some((ip, p)), Some(f), Some(t)) = (d.ifaces.first().and_then(|f| f.addr()), parse_ip(&d.dhcp_from), parse_ip(&d.dhcp_to)) {
                    if !same_net(f, ip, p) || !same_net(t, ip, p) || u32::from(f) > u32::from(t) {
                        push(Level::Error, Some(d.id), None, crate::i18n::trf("{}'s DHCP pool {}–{} doesn't fit its network {}/{}.", &[&d.name, &f, &t, &(net_of(ip, p)), &p]));
                    }
                }
            }
            if d.kind.radio() && d.bands.iter().any(|b| *b) {
                match d.security {
                    Security::Open => push(Level::Error, id, None, crate::i18n::trf("{}'s Wi-Fi has no password. Anyone nearby can join and read unencrypted traffic.", &[&d.name])),
                    Security::Wep => push(Level::Error, id, None, crate::i18n::trf("{} uses WEP, which can be cracked in minutes. Use WPA3 (or WPA2/WPA3 mixed).", &[&d.name])),
                    Security::Wpa2 => push(Level::Tip, id, None, crate::i18n::trf("{}: WPA3 is safer than WPA2 and required for 6 GHz Wi-Fi.", &[&d.name])),
                    Security::Wpa3 | Security::Wpa3Ent => {}
                }
            }
        }
        // Two DHCP servers in one network.
        let mut dhcp: HashMap<usize, u32> = HashMap::new();
        for d in self.devices.iter().filter(|d| d.services.dhcp && self.connected(d.id)) {
            if let Some(&s) = segs.get(&(d.id, 0)) {
                if let Some(o) = dhcp.insert(s, d.id) {
                    push(Level::Warning, Some(d.id), None, crate::i18n::trf("{} and {} both hand out addresses in the same network. Keep one DHCP server.", &[&d.name, &(self.name(o))]));
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
                    push(Level::Error, None, lk, crate::i18n::trf("Wi-Fi between {} and {}: one side must be a router or access point.", &[&a.name, &b.name]));
                } else if !cl.kind.wifi_client() && !cl.kind.radio() {
                    push(Level::Error, None, lk, crate::i18n::trf("{} has no Wi-Fi. Use a cable.", &[&cl.name]));
                }
            } else {
                if l.medium.cable() && l.medium != Medium::Fiber && len > 100.0 {
                    push(Level::Error, None, lk, crate::i18n::trf("The cable {}–{} is {} m; copper Ethernet is limited to 100 m.", &[&a.name, &b.name, &format!("{:.0}", len)]));
                }
            }
        }
        if !self.devices.iter().any(|d| d.kind == Kind::Internet) {
            push(Level::Tip, None, None, crate::i18n::tr("No internet connection in this plan. Add the Internet cloud and connect it to a router.").into());
        }
        self.check_more(&mut v);
        v.sort_by_key(|i| i.level);
        v
    }

    /// A device by its name or host name.
    pub fn lookup_name(&self, word: &str) -> Option<&Device> {
        self.lookup_dev(word)
    }

    fn lookup_dev(&self, word: &str) -> Option<&Device> {
        let w = word.to_lowercase();
        self.devices.iter().find(|d| d.hostname() == w || d.name.to_lowercase() == w)
    }

    /// Opens a TCP connection to `target` on `port`: name lookup, then the SYN on its way there.
    /// The caller then `accept`s (SYN-ACK, ACK) or `refuse`s (RST). None, with the reason in `o`, when it doesn't arrive.
    fn tcp_open(&self, id: u32, target: &str, port: u16, o: &mut Outcome) -> Option<Tcp> {
        let dst = self.resolve(id, target, o)?;
        let Some(src) = self.src_ip(id) else {
            o.lines.push("No IP address — is DHCP working?".into());
            return None;
        };
        // Each side starts counting at a random number (its ISN); the client also picks a free port of its own.
        let tick = (self.clock * 1000.0) as u64;
        let isn = |who: u64| crate::sim::hash(self.seed ^ tick, who, (u32::from(dst) as u64) << 16 | port as u64) as u32;
        let mut t = self.send(id, src, dst, "TCP", port, None);
        let mut k = Tcp { client: (src, 49152 + (isn(0xE9) % 16384) as u16, id), server: (dst, port, t.dst_dev.unwrap_or(id)), cs: isn(id as u64), ss: 0, fwd: Hdr::default(), back: Hdr::default(), site: t.site, real: t.real, ms: t.ms, seen: t.seen_src.unwrap_or(src), nat: t.nat };
        k.ss = isn(k.server.2 as u64 | 1 << 40);
        k.seg(o, true, "SYN", 0, "TCP", "SYN — may I open a connection?".into());
        let mut head = o.log.pop().unwrap();
        carry(&mut head, &mut t.log);
        k.fwd = head.hdr.clone();
        o.log.push(head);
        o.log.extend(t.log.clone());
        let (ok, err) = (t.ok, t.error.clone());
        o.traces.push(t);
        if !ok {
            o.lines.push(format!("Can't connect to {target}: {err}"));
            return None;
        }
        Some(k)
    }

    /// The server answers the SYN: SYN-ACK back, the client's ACK, and the connection is open.
    fn accept(&self, k: &mut Tcp, o: &mut Outcome) {
        let (dst, port, dev) = k.server;
        let mut back = self.send(dev, dst, k.seen, "TCP", port, k.nat);
        k.seg(o, false, "SYN, ACK", 0, "TCP", "SYN-ACK — yes, let's talk".into());
        let mut head = o.log.pop().unwrap();
        carry(&mut head, &mut back.log);
        k.back = head.hdr.clone();
        o.log.push(head);
        o.log.extend(back.log.clone());
        k.ms += back.ms;
        o.traces.push(back);
        k.seg(o, true, "ACK", 0, "TCP", "ACK — connection open".into());
    }

    /// Nothing listens on the port: the server answers with a reset.
    fn refuse(&self, k: &mut Tcp, o: &mut Outcome, what: &str) {
        o.lines.push(format!("Connection refused: {} doesn't offer {what} (port {}).", self.name(k.server.2), k.server.1));
        k.seg(o, false, "RST, ACK", 0, "TCP", "RST — nothing listens on this port".into());
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
        let mut k = self.tcp_open(id, target, port, o)?;
        let target_dev = self.dev(k.server.2).unwrap();
        let mut status = "200 OK";
        let content = if let Some(s) = k.site {
            (svc_name == "web").then(|| SITES[s].2.to_string())
        } else if k.real {
            (svc_name == "web").then(|| format!("<p>{target} is a real website. The browser fetches it from the real internet, at this network's speed.</p>"))
        } else {
            let s = &target_dev.services;
            match svc_name {
                // Pages come from the webserver folder; "" and a folder mean its index.html.
                "web" if s.web => Some({
                    let p = path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
                    let file = if p.is_empty() || !p.rsplit('/').next().unwrap_or("").contains('.') { format!("webserver/{p}/index.html") } else { format!("webserver/{p}") };
                    match target_dev.file(&file).or_else(|| target_dev.file(&format!("webserver/{p}.html"))) {
                        Some(f) => f.to_string(),
                        None if p.is_empty() => format!("{} web page", target_dev.name),
                        None => {
                            status = "404 Not Found";
                            format!("<h1>404 Not Found</h1><p>{} has no page called {p}.</p>", target_dev.name)
                        }
                    }
                }),
                "file sharing" if s.files => Some({
                    let list: Vec<String> = target_dev.files.iter().filter_map(|f| f.0.strip_prefix("shared/")).filter(|n| !n.ends_with(".keep")).map(String::from).collect();
                    let listed = if list.is_empty() { "the shared folder is empty".to_string() } else { list.join(", ") };
                    format!("\\\\{}\\shared — connected with SMB 3.1.1 (encrypted): {listed}", target_dev.hostname())
                }),
                "printing" if s.print => Some(format!("{} is ready (IPP Everywhere, no driver needed)", target_dev.name)),
                "DNS" if s.dns || target_dev.kind.routes() => Some(format!("{} answers DNS on port 53", target_dev.name)),
                _ => None,
            }
        };
        let Some(text) = content else {
            self.refuse(&mut k, o, svc_name);
            return None;
        };
        self.accept(&mut k, o);
        if k.real {
            o.fetch = Some((format!("https://{target}/{path}"), k.ms));
        }
        if proto == "HTTPS" {
            k.seg(o, true, "PSH, ACK", 517, "TLS", "TLS 1.3 ClientHello — let's agree on a secret key".into());
            k.seg(o, false, "PSH, ACK", 2800, "TLS", "TLS 1.3 ServerHello and certificate — traffic is now encrypted".into());
            let get = format!("GET /{path} HTTP/2  Host: {target}");
            k.seg(o, true, "PSH, ACK", get.len() as u32 + 60, "HTTP", get);
            k.seg(o, false, "PSH, ACK", text.len() as u32, "HTTP", format!("HTTP/2 {status} — {} bytes", text.len()));
            k.seg(o, true, "ACK", 0, "TCP", "ACK — got it all".into());
            o.lines.push(format!("HTTP/2 {status}  ({target}, secured with TLS 1.3)"));
        } else {
            o.lines.push(format!("{proto} port {port} open on {target}"));
        }
        k.close(o);
        Some(text)
    }

    /// The mail server a domain names in an MX record ("school.test MX mail.school.test"); without one,
    /// mail goes to the domain's own address (the implicit MX of RFC 5321).
    fn mx_of(&self, domain: &str) -> String {
        self.devices.iter().filter(|s| s.services.dns).flat_map(|s| s.dns_records.lines()).find_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            (w.len() == 3 && w[0].eq_ignore_ascii_case(domain) && w[1].eq_ignore_ascii_case("MX")).then(|| w[2].to_string())
        })
        .unwrap_or_else(|| domain.to_string())
    }

    /// Sends an email the way real mail travels: the device hands it to its mail server (the one its mail account
    /// names, or else one on its own network; submission, port 587); that server looks up the recipient domain's MX
    /// record and relays it there over SMTP (port 25). Without a mail server nearby the device submits straight to
    /// the recipient's. A server with mailboxes refuses names it doesn't have: the mail bounces back. True when delivered.
    pub fn send_mail(&mut self, id: u32, to: &str, subject: &str, text: &str, o: &mut Outcome) -> bool {
        let Some((user, domain)) = to.split_once('@').filter(|(u, d)| !u.is_empty() && !d.is_empty() && !text.is_empty()) else {
            o.lines.push("usage: mail <name@domain> <text>   e.g. mail anna@school.test Hello!".into());
            return false;
        };
        let (user, domain) = (user.to_lowercase(), domain.to_lowercase());
        let mx = self.mx_of(&domain);
        let segs = self.segments();
        let home = segs.get(&(id, 0));
        let account = self.dev(id).and_then(|d| d.mail_account.clone()).filter(|a| !a.address.is_empty());
        let mine = account.as_ref().and_then(|a| self.lookup_dev(&a.server)).filter(|s| s.id != id).map(|s| (s.id, s.first_ip()));
        let outgoing = mine.or_else(|| self.devices.iter().find(|s| s.services.mail && s.id != id && home.is_some() && (0..s.ifaces.len()).any(|i| segs.get(&(s.id, i)) == home)).map(|s| (s.id, s.first_ip())));
        let me = self.dev(id).unwrap().hostname();
        let from = account.as_ref().map_or_else(|| format!("{me}@{domain}"), |a| a.address.clone());
        let smtp = |k: &mut Tcp, o: &mut Outcome, last: &str| {
            k.seg(o, true, "PSH, ACK", 30, "SMTP", format!("EHLO {me}, STARTTLS — from here on it's encrypted"));
            k.seg(o, true, "PSH, ACK", (from.len() + to.len()) as u32 + 24, "SMTP", format!("MAIL FROM:<{from}>  RCPT TO:<{to}>"));
            k.seg(o, true, "PSH, ACK", (subject.len() + text.len()) as u32 + 10, "SMTP", if subject.is_empty() { format!("DATA: \"{text}\"") } else { format!("DATA: Subject: {subject}") });
            k.seg(o, false, "PSH, ACK", 30, "SMTP", last.to_string());
            k.close(o);
        };
        // 1. Submission: to the own mail server, or straight to the recipient's.
        let first = match outgoing {
            Some((_, Some(ip))) => ip.to_string(),
            _ => mx.clone(),
        };
        let Some(mut k) = self.tcp_open(id, &first, 587, o) else { return false };
        let server = k.server.2;
        if !self.dev(server).is_some_and(|s| s.services.mail) || k.site.is_some() || k.real {
            self.refuse(&mut k, o, "email");
            return false;
        }
        self.accept(&mut k, o);
        // Mail for its own users' domain (the sender's, or one whose MX names it) stays on this server.
        let srv = self.dev(server).unwrap();
        let own = account.as_ref().and_then(|a| a.address.rsplit_once('@')).is_some_and(|f| f.1.eq_ignore_ascii_case(&domain)) || [srv.hostname(), srv.name.to_lowercase()].contains(&mx.to_lowercase()) || srv.has_ip(mx.parse().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED));
        let relay = outgoing.is_some_and(|(s, _)| s == server) && !own;
        let mut inbox = server;
        // 2. Relay: the outgoing server asks DNS for the domain's MX and passes the mail on over port 25.
        if relay {
            smtp(&mut k, o, "250 OK — queued for delivery");
            o.log.push(Log { src: self.name(server), dst: domain.clone(), proto: "DNS", layer: "Application", info: format!("MX lookup for {domain}: mail goes to {mx}"), dev: server, ..Default::default() });
            let Some(mut k2) = self.tcp_open(server, &mx, 25, o) else {
                o.lines.push(format!("{} accepted the mail but can't reach {mx}, {domain}'s mail server: it will retry for a few days.", self.name(server)));
                return false;
            };
            let dest = k2.server.2;
            if dest != server {
                if !self.dev(dest).is_some_and(|s| s.services.mail) || k2.site.is_some() || k2.real {
                    self.refuse(&mut k2, o, "email");
                    o.lines.push(format!("{mx} doesn't take mail: {} would bounce it back to you.", self.name(server)));
                    return false;
                }
                self.accept(&mut k2, o);
                smtp(&mut k2, o, "250 OK — delivered");
                inbox = dest;
            }
        } else {
            smtp(&mut k, o, "250 OK — delivered");
        }
        // 3. The receiving server checks the mailbox exists.
        let dest = self.dev(inbox).unwrap();
        let known = dest.mailboxes.is_empty() || dest.mailboxes.iter().any(|m| m.eq_ignore_ascii_case(&user) || m.eq_ignore_ascii_case(to));
        if !known {
            let why = format!("{} has no mailbox called '{user}': the mail bounces back to {from}.", dest.name);
            o.log.push(Log { src: self.name(inbox), dst: from.clone(), proto: "SMTP", layer: "Application", info: format!("550 No such user {user} — {}", dest.name), dev: inbox, ..Default::default() });
            o.lines.push(why.clone());
            let bounce = crate::soft::Mail { from: format!("mailer-daemon@{domain}"), to: from.clone(), subject: format!("Undeliverable: {}", if subject.is_empty() { text } else { subject }), body: why, read: false };
            if let Some(d) = self.dev_mut(server).filter(|d| d.services.mail) {
                d.inbox.push(bounce);
            }
            return false;
        }
        o.lines.push(if inbox == server { format!("Mail sent to {to} via {} (SMTP, port 587).", self.name(server)) } else { format!("Mail sent to {to}: {} took it (port 587) and relayed it to {} (port 25, MX of {domain}).", self.name(server), self.name(inbox)) });
        let mail = crate::soft::Mail { from, to: to.to_lowercase(), subject: subject.into(), body: text.into(), read: false };
        self.dev_mut(inbox).unwrap().inbox.push(mail);
        true
    }

    /// Fetches the mail of an address (none: the device's mail account): on the mail server itself directly, from
    /// anywhere else over IMAP to the account's server or the domain's MX. The mails as (server, index on it).
    pub fn fetch_mail(&self, id: u32, who: &str, o: &mut Outcome) -> Vec<(u32, usize)> {
        let me = self.dev(id).unwrap();
        let account = me.mail_account.clone().filter(|a| !a.address.is_empty());
        let who = if who.is_empty() { account.as_ref().map_or(String::new(), |a| a.address.clone()) } else { who.to_string() };
        let pick = |server: u32| -> Vec<(u32, usize)> {
            let w = who.to_lowercase();
            self.dev(server).map_or(vec![], |s| s.inbox.iter().enumerate().filter(|(_, m)| w.is_empty() || m.to.eq_ignore_ascii_case(&w)).map(|(i, _)| (server, i)).collect())
        };
        if who.is_empty() && me.services.mail {
            return pick(id);
        }
        let Some((user, domain)) = who.split_once('@') else {
            o.lines.push("usage: inbox <name@domain>   (on the mail server itself: inbox)".into());
            return vec![];
        };
        // IMAP goes to the account's server, or to the one the domain's MX record names.
        let target = account.as_ref().filter(|a| a.address.eq_ignore_ascii_case(&who) && !a.server.is_empty()).and_then(|a| self.lookup_dev(&a.server)).and_then(|s| s.first_ip()).map_or_else(|| self.mx_of(domain), |ip| ip.to_string());
        let Some(mut k) = self.tcp_open(id, &target, 993, o) else { return vec![] };
        if !self.dev(k.server.2).is_some_and(|s| s.services.mail) || k.site.is_some() || k.real {
            self.refuse(&mut k, o, "email");
            return vec![];
        }
        self.accept(&mut k, o);
        k.seg(o, true, "PSH, ACK", 40, "IMAP", format!("TLS, then LOGIN {user} (the password travels encrypted)"));
        k.seg(o, true, "PSH, ACK", 20, "IMAP", "SELECT INBOX".into());
        let got = pick(k.server.2);
        let n = got.len();
        k.seg(o, false, "PSH, ACK", 200 * n as u32 + 20, "IMAP", format!("* {n} EXISTS — {n} message(s)"));
        k.close(o);
        got
    }

    /// Reads an inbox for the command line.
    fn read_inbox(&self, id: u32, who: &str, o: &mut Outcome) {
        let got = self.fetch_mail(id, who, o);
        if got.is_empty() && !o.lines.iter().any(|l| l.starts_with("usage")) && o.traces.iter().all(|t| t.ok) {
            o.lines.push("No mail yet.".into());
        }
        for (n, (s, i)) in got.iter().enumerate() {
            let m = &self.dev(*s).unwrap().inbox[*i];
            let text = if m.subject.is_empty() { m.body.clone() } else { format!("{} — {}", m.subject, m.body) };
            o.lines.push(format!("{:>2}  from {}  to {}:  {text}", n + 1, m.from, m.to));
        }
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
        for (sw, mac, link) in o.traces.iter().flat_map(|t| t.macs.iter()) {
            let table = self.mac_seen.entry(*sw).or_default();
            // A MAC address seen on another port has moved: the newest port wins.
            match table.iter_mut().find(|e| e.0 == *mac) {
                Some(e) => e.1 = *link,
                None => table.push((mac.clone(), *link)),
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
                        "mail <to> <text>    send an email (SMTP), e.g. mail anna@school.test Hi",
                        "inbox <address>     read an inbox (IMAP); on the mail server: inbox",
                        "arp                 neighbours I know by MAC address",
                        "mac                 which MAC address is behind which port",
                        "route               my routing table (R = learned by RIP)",
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
                let cg = self.cgnat(&d);
                for (i, f) in d.ifaces.iter().enumerate() {
                    let addr = match &cg {
                        // On the mobile line the router itself holds a shared-range address; the WAN entry is the provider's.
                        Some(cg) if i == 1 => format!("{cg}/10 (mobile line; the provider shares {} among many customers)", f.ip),
                        _ if f.ip.is_empty() => "no address".into(),
                        _ => format!("{}/{}", f.ip, f.prefix),
                    };
                    o.lines.push(format!("{}: {addr}  mac {}", f.name, f.mac));
                }
                o.lines.push(format!("gateway: {}   dns: {}   {}", or_dash(&d.gateway), or_dash(&d.dns), if d.dhcp_client { "(from DHCP)" } else { "(fixed)" }));
                match self.v6_of(&d) {
                    Some(v6) => o.lines.push(format!("IPv6: {v6}/64 (global, from the router via SLAAC) + fe80:: link-local")),
                    None => o.lines.push("IPv6: fe80:: link-local only (no IPv6 from the router)".into()),
                }
                o.lines.extend(self.v6_delegation(&d));
                if let Some(cg) = cg {
                    o.lines.push(format!("Mobile line: the router only has {cg} from the provider's shared range (carrier-grade NAT) — nothing from outside reaches it, so no port forwarding."));
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
                o.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "ICMP", layer: "Network", info: "Echo request".into(), dev: id, ..Default::default() });
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
                o.log.push(Log { src: dst.to_string(), dst: src.to_string(), proto: "ICMP", layer: "Network", info: "Echo reply".into(), dev: id, ..Default::default() });
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
            "arp" => o.lines.extend(self.arp_lines(id)),
            "mac" | "mactable" | "mac-address-table" => o.lines.extend(self.mac_lines(id)),
            "mail" | "sendmail" => {
                self.send_mail(id, arg, "", &words.get(2..).unwrap_or_default().join(" "), &mut o);
            }
            "inbox" | "imap" => self.read_inbox(id, arg, &mut o),
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
                let (rip, rounds, waiting) = self.rip_state();
                let learned = rip.get(&id).map_or(&[][..], |v| v);
                for (n, p, gw, m) in learned {
                    o.lines.push(format!("{:<18} {:<16} R  metric {m}", format!("{n}/{p}"), gw));
                }
                if !d.gateway.is_empty() {
                    o.lines.push(format!("{:<18} {:<16}", "default (0.0.0.0/0)", d.gateway));
                }
                if let Some(s) = waiting {
                    o.lines.push(format!("R = learned by RIP so far ({rounds} update round(s)). Still spreading: the next update goes out in {s:.0} s, one router further each time."));
                } else if !learned.is_empty() {
                    o.lines.push(format!("R = learned by RIP (metric = routers on the way, 16 = unreachable). Settled after {rounds} update round(s); RIP sends one every 30 s."));
                } else if d.services.rip {
                    o.lines.push("RIP is on, but no neighbour router runs it yet.".into());
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

impl Network {
    /// The ARP cache: neighbours this device has talked to, by address and MAC.
    pub fn arp_lines(&self, id: u32) -> Vec<String> {
        // Only neighbours it has talked to: ARP asks for a MAC address the first time it's needed.
        let segs = self.segments();
        let known = self.arp_seen.get(&id).cloned().unwrap_or_default();
        let mut v = vec![];
        for (i, _) in self.dev(id).map(|d| d.ifaces.as_slice()).unwrap_or_default().iter().enumerate() {
            let Some(&s) = segs.get(&(id, i)) else { continue };
            for x in &self.devices {
                for (j, f) in x.ifaces.iter().enumerate() {
                    if x.id != id && segs.get(&(x.id, j)) == Some(&s) && f.addr().is_some_and(|a| known.contains(&a.0)) {
                        v.push(format!("{:<16} {}   {}", f.ip, f.mac, x.name));
                    }
                }
            }
        }
        if v.is_empty() {
            v.push("No entries yet: a device learns a neighbour's MAC address (ARP) the first time it talks to it — try a ping first.".into());
        }
        v
    }

    /// The switch's MAC address table: which address it saw arrive on which port.
    pub fn mac_lines(&self, id: u32) -> Vec<String> {
        let seen = self.mac_seen.get(&id).map_or(&[][..], |v| v);
        if seen.is_empty() {
            return vec!["No entries yet: a switch learns which MAC address is behind which port from the frames passing through — try a ping first.".into()];
        }
        let mut v = vec![format!("{:<18} {:<24} Device", "MAC address", "Port")];
        for (mac, link) in seen {
            let owner = self.devices.iter().find(|d| d.ifaces.iter().any(|f| f.mac == *mac)).map(|d| d.name.clone()).unwrap_or_default();
            let port = match self.links.iter().find(|l| l.id == *link) {
                Some(l) if l.medium.wireless() => format!("Wi-Fi ({})", self.name(l.other(id).0)),
                Some(l) => l.port_at(id).map(|p| self.port_name(id, p)).unwrap_or_else(|| format!("cable to {}", self.name(l.other(id).0))),
                None => "?".into(),
            };
            v.push(format!("{mac:<18} {port:<24} {owner}"));
        }
        v
    }
}

/// A port's MAC address; a VLAN sub-interface uses its physical port's.
fn mac_of(d: &Device, i: usize) -> String {
    let f = &d.ifaces[i];
    match f.parent {
        Some(p) if f.mac.is_empty() => d.ifaces.get(p).map(|x| x.mac.clone()).unwrap_or_default(),
        _ => f.mac.clone(),
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
        "flat" | "roaming" | "extender" | "apartment" | "speakers" | "office" | "nightmare" | "campus" | "rip" | "mail" => crate::scenarios::build(name, &mut n),
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
    n.fit_software();
    // A template is new text: it starts in the interface's language.
    n.title = crate::i18n::tr(&n.title).to_string();
    n.story = crate::i18n::tr(&n.story).to_string();
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
    fn the_shopping_list_leaves_out_what_is_already_there() {
        let mut n = template("home");
        let (items, total) = crate::shopping(&n);
        assert!(!items.iter().any(|i| i.0.contains(Kind::Laptop.label())), "{items:?}");
        let laptop = n.devices.iter().position(|d| d.kind == Kind::Laptop).unwrap();
        n.devices[laptop].owned = Some(false);
        assert!((crate::shopping(&n).1 - total - n.devices[laptop].price).abs() < 0.01);
        // Two access points of the same model are one line.
        let aps: Vec<usize> = (0..n.devices.len()).filter(|i| n.devices[*i].kind == Kind::AccessPoint).collect();
        if aps.len() >= 2 {
            for i in &aps {
                n.devices[*i].model = "Test AP".into();
            }
            let row = crate::shopping(&n).0.into_iter().find(|i| i.0.starts_with("Test AP")).unwrap();
            assert_eq!(row.1, aps.len());
        }
    }

    #[test]
    fn only_a_plan_opens_as_a_plan_and_damaged_numbers_are_dropped() {
        assert!(Network::from_json(r#"{"format":"netsketch-survey","points":[]}"#).is_err());
        assert!(Network::from_json("{}").is_err());
        let mut n = template("flat");
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(Network::from_json(&json).unwrap().devices.len(), n.devices.len());
        // 1e39 doesn't fit an f32 and reads as infinity.
        let bad = json.replacen(&format!("\"pos\":[{},", serde_json::to_string(&n.devices[0].pos[0]).unwrap()), "\"pos\":[1e39,", 1);
        let mut m = Network::from_json(&bad).unwrap();
        m.sanitize();
        assert_eq!(m.devices.len(), n.devices.len() - 1);
        n.labels.push(RoomLabel { id: 999, pos: [0.0, 0.0], floor: -100, text: "x".into() });
        n.sanitize();
        assert_eq!(n.labels.last().unwrap().floor, -3);
    }

    #[test]
    fn tcp_counts_bytes_with_its_sequence_numbers_and_dns_uses_plain_udp() {
        let mut n = template("classroom");
        let pc = n.lookup_dev("pc-1").unwrap().id;
        let o = n.exec(pc, "open www.school.test");
        let tcp: Vec<&Hdr> = o.log.iter().filter(|l| l.hdr.l4 == "TCP" && !l.hdr.flags.is_empty() && l.layer != "Network").map(|l| &l.hdr).collect();
        let flags: Vec<&str> = tcp.iter().map(|h| h.flags).collect();
        assert_eq!(&flags[..3], ["SYN", "SYN, ACK", "ACK"], "{flags:?}");
        assert_eq!(&flags[flags.len() - 3..], ["FIN, ACK", "FIN, ACK", "ACK"], "{flags:?}");
        let (syn, synack, ack) = (tcp[0], tcp[1], tcp[2]);
        assert_eq!((synack.ack, ack.seq, ack.ack), (syn.seq.wrapping_add(1), syn.seq.wrapping_add(1), synack.seq.wrapping_add(1)));
        // Every segment's number is the last one's plus the bytes it carried; the other side acknowledges exactly that.
        let client = syn.ports.0;
        let mut next = (syn.seq, synack.seq);
        for h in &tcp {
            let mine = if h.ports.0 == client { &mut next.0 } else { &mut next.1 };
            assert_eq!(h.seq, *mine, "{h:?}");
            *mine = h.seq.wrapping_add(h.len + h.flags.contains("SYN") as u32 + h.flags.contains("FIN") as u32);
        }
        // The hop lines carry the segment's ports, and TTL goes down by one at the router.
        let hops: Vec<&Hdr> = o.log.iter().filter(|l| l.layer == "Network" && l.hdr.ports.0 == client).map(|l| &l.hdr).collect();
        assert_eq!(hops.iter().map(|h| h.ip.unwrap().2).collect::<Vec<_>>(), [64, 63]);
        assert!(hops.iter().all(|h| h.flags == "SYN" && h.mac.is_some()));
        let dns = o.log.iter().find(|l| l.proto == "DNS").unwrap();
        assert_eq!((dns.hdr.l4, dns.hdr.ports.1, dns.hdr.flags), ("UDP", 53, ""));
        assert!(dns.layers().iter().any(|(l, t)| *l == "Transport" && t.starts_with("UDP")));
    }

    #[test]
    fn mail_goes_by_smtp_to_the_domains_server_and_is_read_by_imap() {
        let mut n = template("classroom");
        let srv = n.lookup_dev("web-server").unwrap().id;
        let ip = n.dev(srv).unwrap().first_ip().unwrap();
        n.dev_mut(srv).unwrap().dns_records += &format!("\nschool.test {ip}");
        assert!(run(&mut n, "pc-1", "mail anna@school.test Hello there").contains("refused"));
        n.dev_mut(srv).unwrap().services.mail = true;
        let pc = n.lookup_dev("pc-1").unwrap().id;
        let o = n.exec(pc, "mail anna@school.test Hello there");
        assert!(o.lines.join(" ").contains("Mail sent"), "{:?}", o.lines);
        assert!(o.log.iter().any(|l| l.proto == "SMTP" && l.hdr.ports.1 == 587 && l.info.contains("Hello there")));
        run(&mut n, "pc-2", "mail ben@school.test Not for Anna");
        assert!(run(&mut n, "web-server", "inbox").contains("Not for Anna"));
        let out = run(&mut n, "pc-4", "inbox anna@school.test");
        assert!(out.contains("Hello there") && !out.contains("Not for Anna"), "{out}");
        assert!(run(&mut n, "pc-1", "mail nobody").contains("usage"));
    }

    #[test]
    fn mail_is_relayed_to_the_server_the_mx_record_names() {
        let mut n = template("classroom");
        let srv = n.lookup_dev("web-server").unwrap().id;
        let ip = n.dev(srv).unwrap().first_ip().unwrap();
        // The school's mail lives on the web server; pc-2 runs the mail server of its own network.
        n.dev_mut(srv).unwrap().dns_records += &format!("\nschool.test MX mail.school.test\nmail.school.test {ip}");
        n.dev_mut(srv).unwrap().services.mail = true;
        let pc2 = n.lookup_dev("pc-2").unwrap().id;
        n.dev_mut(pc2).unwrap().services.mail = true;
        let pc1 = n.lookup_dev("pc-1").unwrap().id;
        let o = n.exec(pc1, "mail anna@school.test Via the relay");
        let text = o.lines.join(" ");
        assert!(text.contains("relayed"), "{text}");
        assert!(o.log.iter().any(|l| l.proto == "SMTP" && l.hdr.ports.1 == 25), "relay on port 25");
        assert!(o.log.iter().any(|l| l.proto == "DNS" && l.info.contains("MX lookup")));
        assert!(run(&mut n, "pc-4", "inbox anna@school.test").contains("Via the relay"));
        assert!(!run(&mut n, "pc-2", "inbox").contains("Via the relay"), "the relay keeps no copy");
    }

    #[test]
    fn switches_learn_which_mac_is_behind_which_port() {
        let mut n = template("classroom");
        let sw = n.devices.iter().find(|d| d.kind == Kind::Switch).unwrap().id;
        assert!(n.mac_lines(sw)[0].contains("No entries"));
        let pc2 = n.lookup_dev("pc-2").unwrap().ifaces[0].ip.clone();
        run(&mut n, "pc-1", &format!("ping {pc2}"));
        let table = n.mac_lines(sw).join("\n");
        let mac = |name: &str| n.lookup_dev(name).unwrap().ifaces[0].mac.clone();
        assert!(table.contains(&mac("pc-1")) && table.contains(&mac("pc-2")), "{table}");
        assert!(table.contains("port ") && table.contains("PC 1"), "{table}");
        assert!(run(&mut n, "router-1", "mac").contains("No entries"));
    }

    #[test]
    fn rip_counts_hops_and_gives_up_after_fifteen() {
        // 18 routers in a row, a network between each pair: R0 only learns the ones up to 15 routers away.
        let mut n = Network::default();
        let rs: Vec<u32> = (0..18).map(|i| n.add_device(Kind::Router, [i as f32 * 60.0, 0.0])).collect();
        for w in rs.windows(2) {
            n.connect(w[0], w[1], Medium::Cat6).unwrap();
        }
        n.auto_address();
        for r in &rs {
            let d = n.dev_mut(*r).unwrap();
            d.routes.clear();
            d.services.rip = true;
        }
        let (rip, rounds) = n.rip_routes();
        let r0 = &rip[&rs[0]];
        let far = |k: usize| r0.iter().find(|x| x.0 == Ipv4Addr::new(10, 0, k as u8 + 1, 0)).map(|x| x.3);
        assert_eq!((far(1), far(15), far(16)), (Some(1), Some(15), None), "{r0:?}");
        assert!((15..=17).contains(&rounds), "{rounds}");
        assert!(run(&mut n, "router-1", "route").contains("R  metric 15"));
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
    fn software_files_mailboxes_and_a_host_firewall() {
        // An old plan: the page in web_page, services switched on, nothing installed.
        let mut n = template("home");
        let srv = n.add_device(Kind::Server, [400.0, 400.0]);
        let sw = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        n.connect(srv, sw, Medium::Cat6).unwrap();
        n.auto_address();
        {
            let d = n.dev_mut(srv).unwrap();
            d.installed.clear();
            d.files.clear();
            d.web_page = "<h1>OLD PAGE</h1>".into();
            d.services.mail = true;
        }
        n.sanitize();
        let d = n.dev(srv).unwrap();
        assert!(d.has(crate::soft::Soft::WebServer) && d.has(crate::soft::Soft::MailServer) && d.web_page.is_empty());
        let host = d.hostname();
        let pc = n.lookup_dev("office-pc").unwrap().id;
        assert!(n.exec(pc, &format!("open {host}")).lines.join(" ").contains("OLD PAGE"));
        // Uninstalled, the web server is gone.
        n.dev_mut(srv).unwrap().uninstall(crate::soft::Soft::WebServer);
        assert!(!n.exec(pc, &format!("open {host}")).lines.join(" ").contains("OLD PAGE"));
        // Mail with an account goes to the account's server and is kept with the plan; unknown names bounce.
        {
            let d = n.dev_mut(srv).unwrap();
            d.mailboxes = vec!["anna".into(), "ben".into()];
        }
        let sname = n.dev(srv).unwrap().name.clone();
        n.dev_mut(pc).unwrap().mail_account = Some(crate::soft::MailAccount { address: "ben@home.test".into(), server: sname.clone() });
        let mut o = Outcome::default();
        assert!(n.send_mail(pc, "anna@home.test", "Hi", "See you at 8", &mut o), "{:?}", o.lines);
        assert!(!n.send_mail(pc, "zoe@home.test", "Hi", "anyone?", &mut o));
        let saved: Network = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        let inbox = &saved.dev(srv).unwrap().inbox;
        assert!(inbox.iter().any(|m| m.to == "anna@home.test" && m.subject == "Hi"));
        assert!(inbox.iter().any(|m| m.to == "ben@home.test" && m.subject.starts_with("Undeliverable")));
        let mut o = Outcome::default();
        assert_eq!(n.fetch_mail(pc, "", &mut o).len(), 1, "ben gets the bounce");
        // File sharing lists the shared folder.
        {
            let d = n.dev_mut(srv).unwrap();
            d.install(crate::soft::Soft::FileShare);
            d.set_file("shared/plan.txt", "x");
        }
        assert!(n.exec(pc, &format!("test {host} files")).lines.join(" ").contains("plan.txt"));
        // The server's own firewall turns pings away and says so.
        assert!(n.exec(pc, &format!("ping {host}")).lines.join(" ").contains(", 4 received"));
        {
            let d = n.dev_mut(srv).unwrap();
            d.install(crate::soft::Soft::Firewall);
            d.rules.push_str("deny any any\n");
        }
        let out = n.exec(pc, &format!("ping {host}"));
        assert!(out.lines.join(" ").contains("firewall") || out.traces.iter().any(|t| t.error.contains("firewall")), "{:?}", out.lines);
        // The router's DHCP server lists who took an address from it.
        let r = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        assert!(n.leases(r).iter().any(|l| l.0 == pc));
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
        n.links.push(Link { id: 900, a: 9999, a_if: 0, b: 1, b_if: 0, medium: Medium::Cat6, length_m: None, unplugged: false, a_port: None, b_port: None, kit: String::new(), path: vec![], lag: 0 });
        let r = n.devices.iter().position(|d| d.kind == Kind::HomeRouter).unwrap();
        n.devices[r].ifaces.truncate(1);
        n.devices[r].poe_class = 77;
        let pc = n.devices.iter().position(|d| d.kind == Kind::Desktop).unwrap();
        n.devices[pc].ifaces.clear();
        // Heights, batteries and channels far out of range.
        n.devices[r].mount = 1e30;
        n.devices[r].ups_wh = -1e30;
        n.devices[r].zb_ch = 200;
        n.storey_heights = vec![(0, 1e30), (1, -5.0)];
        let json = serde_json::to_string(&n).unwrap();
        let mut back: Network = serde_json::from_str(&json).unwrap();
        back.sanitize();
        assert!(back.links.iter().all(|l| l.a != 9999));
        let d = &back.devices[r];
        assert!(d.mount == 10.0 && d.ups_wh == 0.0 && d.zb_ch == 26 && back.floor_z(2) <= 20.0 && back.floor_z(2) >= 4.0);
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