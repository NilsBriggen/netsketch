//! Ready-made plans that each show a real-world problem or lesson, and the find-the-fault exercises.
use crate::net::{Iface, Kind, Medium, Network, Security};
use crate::ports::{PortGroup, PortKind};
use crate::wifi::{Wall, WallKind};

/// (key, menu label, one-line description)
pub const SCENARIOS: &[(&str, &str, &str)] = &[
    ("flat", "Small flat", "A 58 m² two-room flat: one router in the hall, a few devices, a neighbour through the wall."),
    ("home", "Typical home", "Router, switch, an access point upstairs, walls of brick and concrete."),
    ("roaming", "Walk & roam", "Two access points: walk a phone through the house and watch it hand over."),
    ("extender", "Range-extender trouble", "A badly placed extender with its own name: half speed and sticky devices."),
    ("apartment", "Crowded apartment block", "Five neighbours' networks on your channels."),
    ("speakers", "The speaker that ate the network", "Mesh speakers win spanning tree and become the backbone."),
    ("nightmare", "The nightmare home", "Every classic home-network mistake at once — find and fix them one by one."),
    ("campus", "University campus", "Four buildings on fibre, a data centre, 16 access points, VLANs, a packed lecture hall."),
    ("office", "Office with 100 VLANs", "Core router with 100 VLAN interfaces, four floors, redundant links."),
    ("classroom", "Two networks and a router (lesson)", "The classic first lesson: routing between two networks."),
    ("rip", "Three routers in a row (lesson)", "Routers that don't know the way yet: let them tell each other by RIP."),
    ("blank", "Empty plan", "Start from scratch."),
];

pub fn wall(n: &mut Network, a: [f32; 2], b: [f32; 2], kind: WallKind) {
    n.next_id += 1;
    n.walls.push(Wall { id: n.next_id, a, b, kind, floor: 0 });
}

fn named(n: &mut Network, kind: Kind, pos: [f32; 2], name: &str) -> u32 {
    let id = n.add_device(kind, pos);
    n.dev_mut(id).unwrap().name = name.into();
    id
}

/// Outer walls of a house plus inner rooms (used by the home template).
pub fn home_walls(n: &mut Network) {
    let (l, t, r, b) = (220.0, 20.0, 900.0, 590.0);
    wall(n, [l, t], [r, t], WallKind::Brick);
    wall(n, [r, t], [r, b], WallKind::Brick);
    wall(n, [r, b], [l, b], WallKind::Brick);
    wall(n, [l, b], [l, 380.0], WallKind::Brick);
    wall(n, [l, 380.0], [l, 230.0], WallKind::LowE);
    wall(n, [l, 230.0], [l, t], WallKind::Brick);
    wall(n, [520.0, t], [520.0, 240.0], WallKind::Drywall);
    wall(n, [660.0, 240.0], [r, 240.0], WallKind::Drywall);
    wall(n, [560.0, 360.0], [560.0, b], WallKind::Concrete);
    wall(n, [740.0, 440.0], [740.0, b], WallKind::Wood);
}

pub fn build(name: &str, n: &mut Network) {
    // Map position from metres.
    let m = |x: f32, y: f32| [x * crate::net::PX_PER_M, y * crate::net::PX_PER_M];
    match name {
        "flat" => {
            n.title = "Small flat".into();
            n.story = "A 9 × 6.5 m flat: bedroom, bathroom and hall on one side, living room with kitchen on the other. The router sits in the hall where the fibre comes in. Turn on the Wi-Fi map to see how far it reaches through the bathroom wall.".into();
            // Outer walls: concrete towards the neighbours, brick outside, a big coated window in the living room.
            for (a, b, k) in [
                (m(0.0, 0.0), m(9.0, 0.0), WallKind::Brick),
                (m(9.0, 0.0), m(9.0, 1.0), WallKind::Brick),
                (m(9.0, 1.0), m(9.0, 3.5), WallKind::LowE),
                (m(9.0, 3.5), m(9.0, 6.5), WallKind::Brick),
                (m(9.0, 6.5), m(0.0, 6.5), WallKind::Concrete),
                (m(0.0, 6.5), m(0.0, 0.0), WallKind::Concrete),
                // Bedroom | living room, with a door.
                (m(4.0, 0.0), m(4.0, 2.6), WallKind::Drywall),
                // Bedroom | bathroom and hall.
                (m(0.0, 3.5), m(2.3, 3.5), WallKind::Drywall),
                (m(3.2, 3.5), m(4.0, 3.5), WallKind::Drywall),
                // Tiled bathroom wall.
                (m(2.3, 3.5), m(2.3, 6.5), WallKind::Brick),
                // Hall | living room, with a door.
                (m(4.0, 3.5), m(4.0, 4.4), WallKind::Drywall),
                (m(4.0, 5.4), m(4.0, 6.5), WallKind::Drywall),
            ] {
                wall(n, a, b, k);
            }
            let inet = named(n, Kind::Internet, m(3.1, 8.3), "Internet");
            let r = named(n, Kind::HomeRouter, m(3.1, 5.6), "Router");
            let lap = named(n, Kind::Laptop, m(1.3, 1.3), "Laptop");
            let ph = named(n, Kind::Phone, m(6.0, 4.3), "Phone");
            let tv = named(n, Kind::Tv, m(8.2, 5.6), "TV");
            let sp = named(n, Kind::SmartHome, m(6.0, 0.9), "Kitchen plug");
            n.connect(inet, r, Medium::Fiber).unwrap();
            for x in [lap, ph, tv, sp] {
                n.connect(r, x, Medium::WiFi).unwrap();
            }
            for (text, x, y) in [("Bedroom", 2.0, 2.4), ("Bathroom", 1.15, 5.0), ("Hall", 3.1, 4.2), ("Living room & kitchen", 6.5, 2.9)] {
                n.add_label(m(x, y), 0, text);
            }
            let nb = named(n, Kind::Neighbor, m(-2.0, 2.0), "Next door");
            n.dev_mut(nb).unwrap().busy = 0.3;
            n.auto_address();
        }
        "roaming" => {
            n.title = "Walk & roam".into();
            n.story = "Switch to Simulate, click the phone and press 'Take a walk' (or drag it around). Watch its signal drop and see when it hands over to the other access point. Then turn on fast roaming (802.11k/v/r) and walk again.".into();
            let inet = n.add_device(Kind::Internet, [60.0, 260.0]);
            let r = named(n, Kind::HomeRouter, [240.0, 260.0], "Hall router");
            let sw = named(n, Kind::Switch, [480.0, 120.0], "Switch");
            let ap = named(n, Kind::AccessPoint, [900.0, 300.0], "Garden-side Wi-Fi");
            let ph = named(n, Kind::Phone, [300.0, 380.0], "Phone");
            let lap = named(n, Kind::Laptop, [820.0, 440.0], "Laptop");
            n.connect(inet, r, Medium::Fiber).unwrap();
            n.connect(r, sw, Medium::Cat6).unwrap();
            n.connect(sw, ap, Medium::Cat6).unwrap();
            n.dev_mut(sw).unwrap().poe_class = 2;
            n.dev_mut(sw).unwrap().poe_budget = 60.0;
            n.connect(r, ph, Medium::WiFi).unwrap();
            n.connect(ap, lap, Medium::WiFi).unwrap();
            for (a, b, k) in [
                ([140.0, 20.0], [1040.0, 20.0], WallKind::Brick),
                ([1040.0, 20.0], [1040.0, 560.0], WallKind::LowE),
                ([1040.0, 560.0], [140.0, 560.0], WallKind::Brick),
                ([140.0, 560.0], [140.0, 20.0], WallKind::Brick),
                ([420.0, 200.0], [420.0, 560.0], WallKind::Brick),
                ([620.0, 20.0], [620.0, 380.0], WallKind::Concrete),
                ([760.0, 380.0], [1040.0, 380.0], WallKind::Drywall),
            ] {
                wall(n, a, b, k);
            }
            n.auto_address();
        }
        "extender" => {
            n.title = "Range-extender trouble".into();
            n.story = "The extender sits far from the router behind two brick walls and uses its own network name. Speed-test from the TV, then look at the problems: half speed, a weak feed, and a laptop that joined the extender although the router is right next to it.".into();
            let inet = n.add_device(Kind::Internet, [60.0, 200.0]);
            let r = named(n, Kind::HomeRouter, [240.0, 200.0], "Router");
            let ext = named(n, Kind::Extender, [760.0, 260.0], "Plug-in extender");
            let lap = named(n, Kind::Laptop, [320.0, 330.0], "Laptop");
            let tv = named(n, Kind::Tv, [900.0, 160.0], "Bedroom TV");
            let ph = named(n, Kind::Phone, [520.0, 420.0], "Phone");
            n.dev_mut(ext).unwrap().ssid = "HomeNet_EXT".into();
            n.connect(inet, r, Medium::Fiber).unwrap();
            n.connect(r, ext, Medium::WiFi).unwrap();
            n.connect(ext, lap, Medium::WiFi).unwrap();
            n.connect(ext, tv, Medium::WiFi).unwrap();
            n.connect(r, ph, Medium::WiFi).unwrap();
            for (a, b, k) in [([440.0, 20.0], [440.0, 520.0], WallKind::Brick), ([640.0, 20.0], [640.0, 520.0], WallKind::Brick), ([820.0, 300.0], [1000.0, 300.0], WallKind::Drywall)] {
                wall(n, a, b, k);
            }
            n.auto_address();
        }
        "apartment" => {
            n.title = "Crowded apartment block".into();
            n.story = "Five neighbours' networks share the air with yours, everyone kept the factory channels, and the kitchen microwave spills into 2.4 GHz while it runs. Look at the problems, run 'wifi' on the laptop to see the networks around you, then press 'Pick better channels'.".into();
            let inet = n.add_device(Kind::Internet, [80.0, 300.0]);
            let r = named(n, Kind::HomeRouter, [460.0, 300.0], "Our router");
            let lap = named(n, Kind::Laptop, [560.0, 380.0], "Laptop");
            let ph = named(n, Kind::Phone, [380.0, 400.0], "Phone");
            let tv = named(n, Kind::Tv, [600.0, 220.0], "TV");
            n.connect(inet, r, Medium::Fiber).unwrap();
            for x in [lap, ph, tv] {
                n.connect(r, x, Medium::WiFi).unwrap();
            }
            for (i, p) in [[300.0, 120.0], [640.0, 80.0], [820.0, 300.0], [640.0, 540.0], [280.0, 540.0]].into_iter().enumerate() {
                let nb = named(n, Kind::Neighbor, p, &format!("Flat {}", i + 1));
                let d = n.dev_mut(nb).unwrap();
                d.ch = [[6, 6, 1, 6, 11][i], [36, 36, 44, 100, 36][i], 37];
                d.busy = [0.5, 0.4, 0.3, 0.6, 0.2][i];
            }
            // The kitchen microwave sits two rooms from the router.
            let mw = named(n, Kind::Interference, [700.0, 420.0], "Kitchen microwave");
            let _ = mw;
            // Party walls between flats and a concrete stairwell.
            for (a, b, k) in [
                ([200.0, 200.0], [760.0, 200.0], WallKind::Brick),
                ([760.0, 200.0], [760.0, 460.0], WallKind::Brick),
                ([760.0, 460.0], [200.0, 460.0], WallKind::Brick),
                ([200.0, 460.0], [200.0, 200.0], WallKind::Concrete),
            ] {
                wall(n, a, b, k);
            }
            n.auto_address();
        }
        "speakers" => {
            n.title = "The speaker that ate the network".into();
            n.story = "Two multi-room speakers are each wired to a switch and also talk over their own 2.4 GHz mesh. All devices ship with spanning-tree priority 32768, so the lowest MAC address becomes the root — that depends on the actual hardware. The speakers have 100 Mbit ports and count path costs on the old short scale, so paths through them look cheap to modern switches. In some houses the speakers win and the switch-to-switch cable goes on standby; in others nothing happens. Run 'speedtest' and 'stp' on the Office PC, then press Shuffle under Settings in the overview to try other devices.".into();
            let inet = n.add_device(Kind::Internet, [60.0, 120.0]);
            let lr_sp = named(n, Kind::Speaker, [520.0, 420.0], "Living room speaker");
            let of_sp = named(n, Kind::Speaker, [880.0, 420.0], "Office speaker");
            let r = named(n, Kind::HomeRouter, [240.0, 120.0], "Router");
            let main = named(n, Kind::Switch, [520.0, 120.0], "Main switch");
            let office = named(n, Kind::Switch, [880.0, 120.0], "Office switch");
            let pc = named(n, Kind::Desktop, [1060.0, 60.0], "Office PC");
            let nas = named(n, Kind::Nas, [1060.0, 200.0], "NAS");
            let tv = named(n, Kind::Tv, [380.0, 300.0], "TV");
            n.connect(inet, r, Medium::Fiber).unwrap();
            n.connect(r, main, Medium::Cat6).unwrap();
            n.connect(main, office, Medium::Cat6).unwrap();
            n.connect(office, pc, Medium::Cat6).unwrap();
            n.connect(office, nas, Medium::Cat6).unwrap();
            n.connect(main, tv, Medium::Cat6).unwrap();
            n.connect(main, lr_sp, Medium::Cat6).unwrap();
            n.connect(office, of_sp, Medium::Cat6).unwrap();
            n.connect(lr_sp, of_sp, Medium::Mesh).unwrap();
            wall(n, [700.0, 20.0], [700.0, 520.0], WallKind::Brick);
            n.auto_address();
        }
        "office" => office(n),
        "nightmare" => nightmare(n),
        "campus" => campus(n),
        "rip" => rip(n),
        _ => {}
    }
}

/// Three routers in a row with empty routing tables: PC A can't reach PC C until RIP shares the routes.
fn rip(n: &mut Network) {
    n.title = "Three routers in a row".into();
    n.story = "PC A and PC C sit three routers apart. Each router only knows the networks plugged into it, so packets get stuck halfway. Turn on RIP on every router (Routes section): they tell their neighbours what they know, one hop at a time, and the routing tables fill themselves. Type route on a router to see the learned R lines.".into();
    let a = named(n, Kind::Desktop, [100.0, 260.0], "PC A");
    let rs: Vec<u32> = (1..=3).map(|i| named(n, Kind::Router, [100.0 + 180.0 * i as f32, 260.0], &format!("Router {i}"))).collect();
    let c = named(n, Kind::Desktop, [820.0, 260.0], "PC C");
    for w in [a, rs[0], rs[1], rs[2], c].windows(2) {
        n.connect(w[0], w[1], Medium::Cat6).unwrap();
    }
    n.auto_address();
    for r in rs {
        n.dev_mut(r).unwrap().routes.clear();
    }
}

fn g(n: u8, mbps: u32) -> PortGroup {
    PortGroup::rj45(n, mbps)
}

fn sfp(n: u8, mbps: u32) -> PortGroup {
    PortGroup { kind: PortKind::Fiber, ..PortGroup::rj45(n, mbps) }
}

/// VLAN v lives in 10.1.v.0/24 (v ≤ 255).
pub fn vlan_ip(v: u16) -> String {
    format!("10.{}.{}.1", 1 + v / 256, v % 256)
}

fn office(n: &mut Network) {
    n.title = "Office with 100 VLANs".into();
    n.story = "A core router carries 100 VLANs (100–199) on one trunk to the core switch; four floor switches hand them to desks. Departments can't see each other's broadcasts, every VLAN gets its own DHCP pool, and traffic between VLANs goes through the router. A redundant floor link is kept on standby by spanning tree. One PC was set to VLAN 250 by mistake — find it.".into();
    let inet = n.add_device(Kind::Internet, [60.0, 80.0]);
    let edge = named(n, Kind::HomeRouter, [240.0, 80.0], "Edge firewall");
    let core = named(n, Kind::Router, [460.0, 80.0], "Core router");
    let csw = named(n, Kind::Switch, [700.0, 80.0], "Core switch");
    n.dev_mut(core).unwrap().ports = vec![sfp(2, 10000), g(6, 1000)];
    n.dev_mut(csw).unwrap().ports = vec![sfp(4, 10000), g(8, 1000)];
    n.connect(inet, edge, Medium::Fiber).unwrap();
    n.connect(edge, core, Medium::Cat6a).unwrap();
    n.connect(core, csw, Medium::Fiber).unwrap();
    {
        let e = n.dev_mut(edge).unwrap();
        e.routes = "10.0.0.0/8 via 192.168.1.2".into();
        e.ssid = "Corp-Lobby".into();
        e.services.dhcp = false;
        e.notes = "Edge router with firewall/NAT. Sends 10.x back to the core router.".into();
    }
    let names = [(101, "Servers"), (110, "Sales"), (120, "Engineering"), (130, "Finance"), (140, "HR"), (150, "Printers"), (160, "Phones"), (170, "Cameras"), (199, "Guests")];
    {
        let c = n.dev_mut(core).unwrap();
        c.ifaces[0].ip = "192.168.1.2".into();
        c.ifaces[1].name = "eth1 (trunk)".into();
        c.gateway = "192.168.1.1".into();
        c.services.dhcp = true;
        c.dns = "10.1.101.10".into();
        for v in 100u16..200 {
            c.ifaces.push(Iface { name: format!("eth1.{v}"), ip: vlan_ip(v), prefix: 24, vlan: v, parent: Some(1), mac: c.ifaces[1].mac.clone() });
        }
        c.notes = names.iter().map(|(v, s)| format!("VLAN {v}: {s}")).collect::<Vec<_>>().join("\n") + "\nVLANs 100–199: reserved for teams and projects.";
    }
    let mut floors = vec![];
    for f in 0..4 {
        let x = 240.0 + f as f32 * 300.0;
        let sw = named(n, Kind::Switch, [x, 300.0], &format!("Floor {} switch", f + 1));
        // 24-port PoE+ access switches: phones, access points and cameras run off them.
        let d = n.dev_mut(sw).unwrap();
        (d.ports, d.poe_class, d.poe_budget, d.model) = (vec![g(24, 1000)], 2, 370.0, "24-port PoE+ switch".into());
        n.connect(csw, sw, Medium::Cat6a).unwrap();
        floors.push(sw);
    }
    n.dev_mut(csw).unwrap().priority = 4096;
    // Redundant link: spanning tree keeps it on standby.
    n.connect(floors[0], floors[1], Medium::Cat6).unwrap();
    let desks: [&[(Kind, &str, u16)]; 4] = [
        &[(Kind::Desktop, "Sales PC 1", 110), (Kind::Desktop, "Sales PC 2", 110), (Kind::Printer, "Printer F1", 150), (Kind::Camera, "Door intercom", 160)],
        &[(Kind::Desktop, "Dev PC 1", 120), (Kind::Desktop, "Dev PC 2", 120), (Kind::AccessPoint, "Floor 2 AP", 1), (Kind::Desktop, "Mislabelled PC", 250)],
        &[(Kind::Desktop, "Finance PC", 130), (Kind::Desktop, "HR PC", 140), (Kind::Camera, "Lobby camera", 170)],
        &[(Kind::Server, "Intranet server", 101), (Kind::Nas, "File server", 101), (Kind::Desktop, "Admin PC", 120)],
    ];
    for (f, list) in desks.iter().enumerate() {
        for (i, (k, name, vlan)) in list.iter().enumerate() {
            let id = named(n, *k, [180.0 + f as f32 * 300.0 + (i % 2) as f32 * 120.0, 460.0 + (i / 2) as f32 * 150.0], name);
            n.connect(floors[f], id, Medium::Cat6).unwrap();
            let d = n.dev_mut(id).unwrap();
            if let Some(i0) = d.ifaces.first_mut() {
                i0.vlan = *vlan;
            }
            if *k == Kind::AccessPoint {
                d.ssid = "Corp-Guest".into();
            }
        }
    }
    let ap = n.devices.iter().find(|d| d.name == "Floor 2 AP").unwrap().id;
    let guest = named(n, Kind::Laptop, [700.0, 780.0], "Visitor laptop");
    n.connect(ap, guest, Medium::WiFi).unwrap();
    n.dev_mut(guest).unwrap().ifaces[0].vlan = 199;
    for (name, ip) in [("Intranet server", "10.1.101.10"), ("File server", "10.1.101.11")] {
        let id = n.devices.iter().find(|d| d.name == name).unwrap().id;
        let d = n.dev_mut(id).unwrap();
        d.dhcp_client = false;
        d.ifaces[0].ip = ip.into();
        d.gateway = "10.1.101.1".into();
        d.dns = "10.1.101.10".into();
        if name == "Intranet server" {
            d.services.dns = true;
            d.dns = "192.168.1.1".into();
            d.dns_records = "intranet.corp.test 10.1.101.10\nfiles.corp.test 10.1.101.11".into();
            d.web_page = "<html><body bgcolor=\"#eef2ff\"><h1>Intranet</h1><p>Welcome to the company intranet. <a href=\"vlans.html\">VLAN plan</a></p></body></html>".into();
            let plan: String = names.iter().map(|(v, s)| format!("<p><b>VLAN {v}</b> {s} — {}/24</p>", vlan_ip(*v).trim_end_matches(".1").to_string() + ".0")).collect();
            d.web_files.push(("vlans.html".into(), format!("<html><body><h1>VLAN plan</h1>{plan}<p><a href=\"/\">Back</a></p></body></html>")));
        }
    }
    n.dev_mut(edge).unwrap().security = Security::Wpa3;
    n.run_dhcp();
}

/// A messy real-world home: every mistake comes from a plausible decision, nothing is scripted.
fn nightmare(n: &mut Network) {
    n.title = "The nightmare home".into();
    n.story = "Years of 'just plug it in': the provider's box sits in a metal cabinet, the old router was added as an 'access point' with its own DHCP still on, a plug-in extender with its own name, two mesh speakers, a cheap switch in the kids' room with a cable plugged in at both ends, an old Cat 5 cable to the TV, powerline to the garden office, a camera on a switch without PoE, two devices with the same fixed address, neighbours on the same channels, a microwave next to the router — and a 50 Mbit/s line. Start with the red problems (the loop first), then work down the list.".into();
    let inet = n.add_device(Kind::Internet, [60.0, 520.0]);
    n.dev_mut(inet).unwrap().plan_mbps = 50.0;
    let isp = named(n, Kind::HomeRouter, [250.0, 520.0], "Provider box");
    {
        let d = n.dev_mut(isp).unwrap();
        crate::wifi::RADIO_PRESETS[0].apply(d);
        d.ssid = "HomeNet".into();
        d.security = Security::Wpa2;
        d.band_steer = false;
        d.width[0] = 40;
        d.tx[0] = 23.0;
        d.ch = [6, 36, 37];
    }
    let old = named(n, Kind::HomeRouter, [700.0, 140.0], "Old router (used as access point)");
    {
        let d = n.dev_mut(old).unwrap();
        crate::wifi::RADIO_PRESETS[0].apply(d);
        d.ssid = "HomeNet_old".into();
        d.security = Security::Wep;
        d.ifaces[0].ip = "192.168.0.1".into();
        (d.dhcp_from, d.dhcp_to) = ("192.168.0.100".into(), "192.168.0.199".into());
        d.ch = [6, 36, 37];
        d.dhcp_client = false;
    }
    let sw = named(n, Kind::Switch, [480.0, 380.0], "Hall switch");
    let kids = named(n, Kind::Switch, [820.0, 380.0], "Kids' room switch");
    let kids2 = named(n, Kind::Switch, [980.0, 280.0], "Cheap 5-port switch");
    for s in [kids, kids2] {
        let d = n.dev_mut(s).unwrap();
        (d.stp, d.model, d.ports) = (false, "Unmanaged 5-port switch".into(), vec![g(5, 1000)]);
    }
    n.dev_mut(sw).unwrap().stp = false;
    let tv = named(n, Kind::Tv, [540.0, 110.0], "Living room TV");
    let tvsw = named(n, Kind::Switch, [430.0, 260.0], "Old TV switch");
    {
        let d = n.dev_mut(tvsw).unwrap();
        (d.stp, d.ports, d.model) = (false, vec![g(5, 100)], "Old 100 Mbit switch".into());
    }
    let cam = named(n, Kind::Camera, [1060.0, 520.0], "Garden camera");
    let nas = named(n, Kind::Nas, [600.0, 520.0], "NAS");
    let pr = named(n, Kind::Printer, [700.0, 520.0], "Printer");
    let console = named(n, Kind::Console, [1080.0, 380.0], "Console");
    let pc = named(n, Kind::Desktop, [1080.0, 220.0], "Kids' PC");
    let sp1 = named(n, Kind::Speaker, [300.0, 170.0], "Living room speaker");
    let sp2 = named(n, Kind::Speaker, [820.0, 610.0], "Kitchen speaker");
    let ext = named(n, Kind::Extender, [980.0, 60.0], "Plug-in extender");
    n.dev_mut(ext).unwrap().ssid = "HomeNet_EXT".into();
    let lap = named(n, Kind::Laptop, [760.0, 60.0], "Laptop");
    let ph = named(n, Kind::Phone, [320.0, 360.0], "Phone");
    let plug = named(n, Kind::SmartHome, [1240.0, 640.0], "Garden shed plug");
    let office = named(n, Kind::Desktop, [1240.0, 120.0], "Garden office PC");
    let mw = named(n, Kind::Interference, [230.0, 610.0], "Microwave");
    let _ = mw;
    n.connect(inet, isp, Medium::Cat5e).unwrap();
    n.connect(isp, sw, Medium::Cat5e).unwrap();
    n.connect(sw, old, Medium::Cat6).unwrap();
    n.connect(sw, tvsw, Medium::Cat5).unwrap();
    n.connect(tvsw, tv, Medium::Cat5).unwrap();
    n.connect(tvsw, sp1, Medium::Cat5e).unwrap();
    n.connect(sw, kids, Medium::Cat6).unwrap();
    n.connect(kids, kids2, Medium::Cat5e).unwrap();
    n.connect(kids2, console, Medium::Cat6).unwrap();
    n.connect(kids2, pc, Medium::Cat6).unwrap();
    // A second cable from the kids' switch back to the hall "for more speed": a loop, and no switch here runs spanning tree.
    n.connect(kids2, sw, Medium::Cat5e).unwrap();
    n.connect(kids, sp2, Medium::Cat6).unwrap();
    n.connect(sp1, sp2, Medium::Mesh).unwrap();
    n.connect(sw, nas, Medium::Cat6).unwrap();
    n.connect(sw, pr, Medium::Cat6).unwrap();
    n.connect(kids, cam, Medium::Cat6).unwrap();
    n.connect(isp, ph, Medium::WiFi).unwrap();
    n.connect(old, ext, Medium::WiFi).unwrap();
    n.connect(ext, lap, Medium::WiFi).unwrap();
    n.connect(isp, plug, Medium::WiFi).unwrap();
    n.connect(sw, office, Medium::Powerline).unwrap();
    for (a, b, k) in [
        ([180.0, 0.0], [1160.0, 0.0], WallKind::Brick),
        ([1160.0, 0.0], [1160.0, 680.0], WallKind::Brick),
        ([1160.0, 680.0], [180.0, 680.0], WallKind::Brick),
        ([180.0, 680.0], [180.0, 0.0], WallKind::LowE),
        ([200.0, 470.0], [300.0, 470.0], WallKind::Metal),
        ([300.0, 470.0], [300.0, 570.0], WallKind::Metal),
        ([640.0, 0.0], [640.0, 300.0], WallKind::Concrete),
        ([640.0, 440.0], [1160.0, 440.0], WallKind::Brick),
        ([900.0, 0.0], [900.0, 440.0], WallKind::Brick),
    ] {
        wall(n, a, b, k);
    }
    for (i, p) in [[300.0, -140.0], [1000.0, -140.0], [1320.0, 360.0]].into_iter().enumerate() {
        let nb = named(n, Kind::Neighbor, p, &format!("Neighbour {}", i + 1));
        let d = n.dev_mut(nb).unwrap();
        d.ch = [[6, 6, 1][i], [36, 36, 44][i], 37];
        d.busy = [0.5, 0.4, 0.3][i];
    }
    for d in n.devices.iter_mut() {
        if matches!(d.kind, Kind::Tv | Kind::Console | Kind::Desktop | Kind::Laptop) {
            d.uses.push(crate::extras::Use::Download);
        }
    }
    n.run_dhcp();
    // Two devices someone gave the same fixed address by hand.
    for id in [nas, pr] {
        let d = n.dev_mut(id).unwrap();
        d.dhcp_client = false;
        d.ifaces[0].ip = "192.168.1.150".into();
        (d.gateway, d.dns) = ("192.168.1.1".into(), "192.168.1.1".into());
    }
}

/// A university campus: core with fibre to every building, a data centre, enterprise Wi-Fi with roaming,
/// per-group VLANs, a guest network and a packed lecture hall.
fn campus(n: &mut Network) {
    n.title = "University campus".into();
    n.story = "Four buildings hang off a core switch on 10 Gbit/s fibre; the core router carries one VLAN per group (staff, students, campus Wi-Fi, guests, building devices, servers). Ceiling access points with fast roaming and personal logins (WPA3-Enterprise) cover every building, powered over PoE+. The lecture hall is packed at 10:00 — watch its airtime. Walk a phone from the library to the lecture hall, open learn.campus.test, or ping between buildings.".into();
    let vlans = [(10u16, "Staff"), (20, "Student labs"), (30, "Campus Wi-Fi"), (40, "Guests"), (50, "Building devices"), (100, "Servers")];
    let inet = n.add_device(Kind::Internet, [80.0, 80.0]);
    n.dev_mut(inet).unwrap().plan_mbps = 10000.0;
    let fw = named(n, Kind::HomeRouter, [300.0, 80.0], "Campus firewall");
    {
        let d = n.dev_mut(fw).unwrap();
        (d.ports, d.model) = (vec![sfp(2, 10000).wan(), sfp(2, 10000), g(4, 10000)], "Campus firewall / NAT".into());
        d.bands = [false, false, false];
        d.ssid.clear();
        d.services.dhcp = false;
        d.routes = "10.0.0.0/8 via 192.168.1.2".into();
        d.sqm = true;
    }
    let core = named(n, Kind::Router, [560.0, 80.0], "Core router");
    let csw = named(n, Kind::Switch, [820.0, 80.0], "Core switch");
    n.connect(inet, fw, Medium::Fiber).unwrap();
    {
        let d = n.dev_mut(core).unwrap();
        (d.ports, d.model) = (vec![sfp(8, 10000)], "Campus core router".into());
    }
    {
        let d = n.dev_mut(csw).unwrap();
        (d.ports, d.priority, d.model) = (vec![sfp(48, 10000)], 4096, "48-port 10G core switch".into());
    }
    n.connect(fw, core, Medium::Fiber).unwrap();
    n.connect(core, csw, Medium::Fiber).unwrap();
    {
        let c = n.dev_mut(core).unwrap();
        c.ifaces[0].ip = "192.168.1.2".into();
        c.ifaces[1].name = "eth1 (trunk)".into();
        c.gateway = "192.168.1.1".into();
        c.services.dhcp = true;
        let mac = c.ifaces[1].mac.clone();
        for (v, _) in vlans {
            c.ifaces.push(Iface { name: format!("eth1.{v}"), ip: vlan_ip(v), prefix: 24, vlan: v, parent: Some(1), mac: mac.clone() });
        }
        c.notes = vlans.iter().map(|(v, s)| format!("VLAN {v}: {s}")).collect::<Vec<_>>().join("\n");
    }
    let dns_ip = vlan_ip(100).trim_end_matches('1').to_string() + "10";
    n.dev_mut(core).unwrap().dns = dns_ip.clone();
    // Data centre.
    let dc = named(n, Kind::Switch, [1100.0, 80.0], "Data centre switch");
    {
        let d = n.dev_mut(dc).unwrap();
        (d.ports, d.model) = (vec![sfp(4, 10000), g(20, 10000)], "24-port 10G switch".into());
    }
    n.connect(csw, dc, Medium::Fiber).unwrap();
    let servers = [("LMS server", "learn.campus.test", 10, true), ("File server", "files.campus.test", 11, false), ("Backup NAS", "backup.campus.test", 12, false)];
    let mut records = String::new();
    for (i, (name, host, last, web)) in servers.iter().enumerate() {
        let kind = if i == 2 { Kind::Nas } else { Kind::Server };
        let id = named(n, kind, [1000.0 + i as f32 * 130.0, 230.0], name);
        n.connect(dc, id, Medium::Cat6a).unwrap();
        let ip = format!("{}{last}", vlan_ip(100).trim_end_matches('1'));
        records += &format!("{host} {ip}\n");
        let d = n.dev_mut(id).unwrap();
        d.ifaces[0].vlan = 100;
        d.ifaces[0].ip = ip;
        d.ports = vec![g(1, 10000)];
        d.dhcp_client = false;
        d.gateway = vlan_ip(100);
        d.dns = dns_ip.clone();
        d.services.web = *web;
        d.services.files = !web;
        if i == 0 {
            d.services.dns = true;
            d.dns = "192.168.1.1".into();
            d.web_page = "<html><body bgcolor=\"#eef6ff\"><h1>Learning platform</h1><p>Welcome back. <a href=\"timetable.html\">Today's timetable</a></p></body></html>".into();
            d.web_files.push(("timetable.html".into(), "<html><body><h1>Timetable</h1><p><b>10:00</b> Networks 101 — Lecture hall A</p><p><a href=\"/\">Back</a></p></body></html>".into()));
        }
    }
    let lms = n.devices.iter().find(|d| d.name == "LMS server").unwrap().id;
    n.dev_mut(lms).unwrap().dns_records = records.trim_end().into();
    // Buildings: (name, origin, access points, wired PCs on VLAN, Wi-Fi clients).
    let buildings: [(&str, [f32; 2], usize, usize, usize); 4] = [("Library", [80.0, 420.0], 4, 4, 5), ("Lecture hall A", [760.0, 420.0], 4, 0, 14), ("Science lab", [80.0, 1060.0], 4, 6, 3), ("Dormitory", [760.0, 1060.0], 4, 0, 6)];
    for (b, (bname, o, aps, pcs, clients)) in buildings.iter().enumerate() {
        let (w, h) = (600.0, 520.0);
        for (a, c, k) in [
            ([o[0], o[1]], [o[0] + w, o[1]], WallKind::Concrete),
            ([o[0] + w, o[1]], [o[0] + w, o[1] + h], WallKind::Concrete),
            ([o[0] + w, o[1] + h], [o[0], o[1] + h], WallKind::Concrete),
            ([o[0], o[1] + h], [o[0], o[1]], WallKind::LowE),
            ([o[0] + w / 2.0, o[1]], [o[0] + w / 2.0, o[1] + h * 0.6], WallKind::Drywall),
        ] {
            wall(n, a, c, k);
        }
        let dist = named(n, Kind::Switch, [o[0] + 90.0, o[1] + 70.0], &format!("{bname} switch"));
        {
            let d = n.dev_mut(dist).unwrap();
            (d.ports, d.poe_class, d.poe_budget, d.model) = (vec![g(48, 2500), sfp(4, 10000)], 2, 740.0, "48-port PoE+ multi-gig switch".into());
        }
        n.connect(csw, dist, Medium::Fiber).unwrap();
        n.links.last_mut().unwrap().length_m = Some(80.0 + 40.0 * b as f32);
        let mut ap_ids = vec![];
        for a in 0..*aps {
            let p = [o[0] + 110.0 + (a % 2) as f32 * 360.0, o[1] + 170.0 + (a / 2) as f32 * 260.0];
            let ap = named(n, Kind::AccessPoint, p, &format!("{bname} AP {}", a + 1));
            n.connect(dist, ap, Medium::Cat6a).unwrap();
            let d = n.dev_mut(ap).unwrap();
            crate::wifi::RADIO_PRESETS[4].apply(d);
            (d.ssid, d.security, d.fast_roam) = ("Campus".into(), Security::Wpa3Ent, true);
            // One radio, three names: each leads into its own VLAN.
            d.ssids = vec![("Campus-Guest".into(), 40), ("Campus-IoT".into(), 50)];
            ap_ids.push(ap);
        }
        for i in 0..*pcs {
            let pc = named(n, Kind::Desktop, [o[0] + 230.0 + (i % 3) as f32 * 90.0, o[1] + 40.0 + (i / 3) as f32 * 80.0], &format!("{bname} PC {}", i + 1));
            n.connect(dist, pc, Medium::Cat6).unwrap();
            n.dev_mut(pc).unwrap().ifaces[0].vlan = if b == 0 { 10 } else { 20 };
        }
        for i in 0..*clients {
            let kind = if i % 2 == 0 { Kind::Laptop } else { Kind::Phone };
            let p = [o[0] + 70.0 + (i % 5) as f32 * 110.0, o[1] + 300.0 + (i / 5) as f32 * 70.0];
            let cl = named(n, kind, p, &format!("{} student {}", bname.split_whitespace().next().unwrap_or(bname), i + 1));
            // Join the nearest access point.
            let ap = *ap_ids.iter().min_by(|x, y| {
                let (px, py) = (n.dev(**x).unwrap().pos, n.dev(**y).unwrap().pos);
                let dx = (px[0] - p[0]).hypot(px[1] - p[1]);
                let dy = (py[0] - p[0]).hypot(py[1] - p[1]);
                dx.total_cmp(&dy)
            }).unwrap();
            n.connect(ap, cl, Medium::WiFi).unwrap();
            let d = n.dev_mut(cl).unwrap();
            d.ifaces[0].vlan = 30;
            if b == 3 && i % 3 == 0 {
                d.uses.push(crate::extras::Use::Stream4k);
            }
        }
        // Building devices: a camera on PoE.
        let cam = named(n, Kind::Camera, [o[0] + w - 60.0, o[1] + h - 60.0], &format!("{bname} camera"));
        n.connect(dist, cam, Medium::Cat6).unwrap();
        n.dev_mut(cam).unwrap().ifaces[0].vlan = 50;
    }
    // A redundant fibre between two buildings: spanning tree keeps it on standby.
    let lib = n.devices.iter().find(|d| d.name == "Library switch").unwrap().id;
    let lab = n.devices.iter().find(|d| d.name == "Science lab switch").unwrap().id;
    n.connect(lib, lab, Medium::Fiber).unwrap();
    // Plan Wi-Fi channels so neighbouring access points don't share.
    n.auto_channels();
    // A sports hall across the field, ~70 m away: no cable, so two dishes on the roofs link it by radio.
    let dorm = n.devices.iter().find(|d| d.name == "Dormitory switch").unwrap().id;
    let b1 = named(n, Kind::AccessPoint, [1340.0, 1120.0], "Roof bridge (dormitory)");
    let b2 = named(n, Kind::AccessPoint, [2700.0, 1500.0], "Roof bridge (sports hall)");
    for b in [b1, b2] {
        let d = n.dev_mut(b).unwrap();
        crate::wifi::RADIO_PRESETS.iter().find(|p| p.beam > 0.0).unwrap().apply(d);
        // Outdoor links must use 5 GHz channels 100–140 (up to 30 dBm with the dish).
        (d.ssid, d.ch[1], d.poe_draw) = ("(bridge)".into(), 100, 8.0);
    }
    n.connect(dorm, b1, Medium::Cat6a).unwrap();
    n.connect(b1, b2, Medium::WiFi).unwrap();
    n.aim_at(b1, b2);
    n.aim_at(b2, b1);
    let hall_sw = named(n, Kind::Switch, [2860.0, 1580.0], "Sports hall switch");
    {
        let d = n.dev_mut(hall_sw).unwrap();
        (d.ports, d.poe_class, d.poe_budget, d.model) = (vec![g(8, 1000)], 2, 120.0, "8-port PoE+ switch".into());
    }
    n.connect(b2, hall_sw, Medium::Cat6).unwrap();
    let hall_ap = named(n, Kind::AccessPoint, [3000.0, 1700.0], "Sports hall AP");
    n.connect(hall_sw, hall_ap, Medium::Cat6a).unwrap();
    {
        let d = n.dev_mut(hall_ap).unwrap();
        crate::wifi::RADIO_PRESETS[4].apply(d);
        (d.ssid, d.security, d.fast_roam, d.ch) = ("Campus".into(), Security::Wpa3Ent, true, [6, 36, 33]);
        d.ssids = vec![("Campus-Guest".into(), 40), ("Campus-IoT".into(), 50)];
    }
    for i in 0..2 {
        let ph = named(n, Kind::Phone, [2950.0 + i as f32 * 120.0, 1800.0], &format!("Sports student {}", i + 1));
        n.connect(hall_ap, ph, Medium::WiFi).unwrap();
        n.dev_mut(ph).unwrap().ifaces[0].vlan = 30;
    }
    for (a, b) in [([2780.0, 1440.0], [3200.0, 1440.0]), ([3200.0, 1440.0], [3200.0, 1900.0]), ([3200.0, 1900.0], [2780.0, 1900.0]), ([2780.0, 1900.0], [2780.0, 1440.0])] {
        wall(n, a, b, WallKind::Concrete);
    }
    n.run_dhcp();
    // A visitor in the library joins the guest name and lands in VLAN 40.
    let lib_ap = n.devices.iter().find(|d| d.name == "Library AP 1").unwrap().id;
    let p = n.dev(lib_ap).unwrap().pos;
    let visitor = named(n, Kind::Phone, [p[0] + 60.0, p[1] + 90.0], "Visitor phone");
    n.connect(lib_ap, visitor, Medium::WiFi).unwrap();
    n.join(visitor, "Campus-Guest", 40);
}


/// "Find the fault" scenarios: what to achieve, and a check of the plan that says whether it's done.
pub const EXERCISES: &[(&str, &str)] = &[
    ("nightmare", "Clear every red problem from the list."),
    ("extender", "Make the extender share the router's network name, and let no device go through it when the router is just as strong."),
    ("speakers", "Give a switch a lower spanning-tree priority so it's the root in every house, whatever hardware the speakers have."),
    ("apartment", "Get your access points off the channels your neighbours crowd."),
    ("office", "Get the mislabelled PC working: it must not have any red problem."),
    ("rip", "Let PC A reach PC C without typing a single route: turn on RIP on the routers."),
];

/// Ok with praise when the exercise for this scenario is solved, Err with what's still wrong.
pub fn check_exercise(n: &Network) -> Result<String, String> {
    use crate::net::Level;
    let issues = n.check();
    let left = |what: &dyn Fn(&crate::net::Issue) -> bool| issues.iter().filter(|i| what(i)).map(|i| i.text.clone()).next();
    let still = match n.scenario.as_str() {
        "nightmare" => left(&|i| i.level == Level::Error),
        "extender" => left(&|i| i.level != Level::Tip && (i.text.contains("crosses the air twice") || i.text.contains("its own name")))
            .or_else(|| left(&|i| i.text.contains("uses its own name"))),
        "speakers" => {
            // Chosen on purpose: a switch with a priority below the default 32768.
            match n.stp().root.and_then(|r| n.dev(r)) {
                Some(d) if d.kind == Kind::Switch && d.priority < 32768 => None,
                Some(d) => Some(format!("{} is the root only by chance (priority {}).", d.name, d.priority)),
                None => Some("Nothing runs spanning tree.".into()),
            }
        }
        "apartment" => left(&|i| i.level != Level::Tip && (i.text.contains("of the airtime") || i.text.contains("overlap"))),
        "office" => {
            let pc = n.devices.iter().find(|d| d.name == "Mislabelled PC").map(|d| d.id);
            left(&|i| i.level == Level::Error && i.dev.is_some() && i.dev == pc)
        }
        "rip" => {
            let ip = |name: &str| n.devices.iter().find(|d| d.name == name).and_then(|d| Some((d.id, d.first_ip()?)));
            match (ip("PC A"), ip("PC C")) {
                (Some((a, ia)), Some((c, ic))) => {
                    let (there, back) = (n.send(a, ia, ic, "ICMP", 0, None), n.send(c, ic, ia, "ICMP", 0, None));
                    (!there.ok || !back.ok).then(|| format!("PC A can't reach PC C: {}", if there.ok { back.error } else { there.error }))
                }
                _ => Some("PC A or PC C has no address.".into()),
            }
        }
        _ => return Err("This plan has no exercise.".into()),
    };
    match still {
        None => Ok("Solved — well done!".into()),
        Some(t) => Err(format!("Not yet: {t}")),
    }
}

#[cfg(test)]
mod tests {
    use crate::net::{template, template_seeded, Kind, Level};

    fn id(n: &crate::net::Network, name: &str) -> u32 {
        n.devices.iter().find(|d| d.name == name).unwrap().id
    }

    #[test]
    fn office_vlans_route_and_get_dhcp() {
        let mut n = template("office");
        assert_eq!(n.dev(id(&n, "Core router")).unwrap().ifaces.iter().filter(|f| f.parent.is_some()).count(), 100);
        let sales = id(&n, "Sales PC 1");
        assert!(n.dev(sales).unwrap().ifaces[0].ip.starts_with("10.1.110."), "{:?}", n.dev(sales).unwrap().ifaces);
        let out = n.exec(sales, "open intranet.corp.test").lines.join("\n");
        assert!(out.contains("200 OK"), "{out}");
        let out = n.exec(sales, "ping example.com").lines.join("\n");
        assert!(out.contains(", 4 received"), "{out}");
        // Same switch, different VLAN: only reachable through the router.
        let out = n.exec(sales, "traceroute 10.1.150.100").lines.join("\n");
        assert!(out.contains("Core router"), "{out}");
        let issues = n.check();
        assert!(issues.iter().any(|i| i.level == Level::Error && i.text.contains("VLAN 250")), "{issues:?}");
        assert!(n.stp().blocked.len() == 1);
    }

    #[test]
    fn extender_scenario_flags_problems_and_halves_speed() {
        let mut n = template("extender");
        let tv = id(&n, "Bedroom TV");
        let out = n.exec(tv, "speedtest").lines.join("\n");
        assert!(out.contains("extender"), "{out}");
        let issues = n.check();
        assert!(issues.iter().any(|i| i.text.contains("never switch")), "{issues:?}");
        assert!(issues.iter().any(|i| i.text.contains("its own name")), "{issues:?}");
        let lap = id(&n, "Laptop");
        n.dev_mut(id(&n, "Plug-in extender")).unwrap().ssid = "HomeNet".into();
        n.reconnect(lap);
        assert_eq!(n.wifi_ends(n.wifi_link_of(lap).unwrap()).unwrap().0.kind, Kind::HomeRouter);
    }

    #[test]
    fn nightmare_home_shows_the_classic_problems_and_they_can_be_fixed() {
        let mut n = template_seeded("nightmare", 9);
        let texts: Vec<String> = n.check().into_iter().map(|i| i.text).collect();
        let all = texts.join("\n");
        for needle in ["broadcast storm", "both use 192.168.1.150", "hand out addresses in the same network", "WEP", "has no PoE", "EU limit", "disturbs it", "100 Mbit/s", "doesn't make it back"] {
            assert!(all.contains(needle), "missing '{needle}' in:\n{all}");
        }
        // Everything stops while the loop rages...
        let pc = id(&n, "Kids' PC");
        let out = n.exec(pc, "ping 192.168.1.1").lines.join(" ");
        assert!(out.contains("storm"), "{out}");
        // ...apply every one-click fix, and the house works again.
        for _ in 0..3 {
            for f in n.check().into_iter().filter_map(|i| i.fix) {
                n.apply_fix(&f);
            }
        }
        n.run_dhcp();
        let after = n.check().into_iter().map(|i| i.text).collect::<Vec<_>>().join("\n");
        assert!(!after.contains("storm"), "{after}");
        // With the loop gone, the slow line shows up at the busy hour.
        assert!(after.contains("busy hour"), "{after}");
        assert!(n.exec(pc, "ping example.com").lines.join(" ").contains("received"));
    }

    #[test]
    fn campus_routes_roams_and_counts_airtime() {
        let mut n = template_seeded("campus", 4);
        let student = n.devices.iter().find(|d| d.name == "Library student 1").unwrap().id;
        assert!(n.dev(student).unwrap().ifaces[0].ip.starts_with("10.1.30."), "{:?}", n.dev(student).unwrap().ifaces);
        let out = n.exec(student, "open learn.campus.test").lines.join(" ");
        assert!(out.contains("200 OK"), "{out}");
        let lab_pc = id(&n, "Science lab PC 1");
        let lib_pc = n.dev(id(&n, "Library PC 1")).unwrap().first_ip().unwrap().to_string();
        let out = n.exec(lab_pc, &format!("traceroute {lib_pc}")).lines.join(" ");
        assert!(out.contains("Core router"), "{out}");
        assert!(n.exec(student, "ping example.com").lines.join(" ").contains(", 4 received") || n.exec(student, "ping example.com").lines.join(" ").contains(" received"));
        let stp = n.stp();
        assert_eq!(n.dev(stp.root.unwrap()).unwrap().name, "Core switch");
        assert_eq!(stp.blocked.len(), 1);
        assert!(!n.check().iter().any(|i| i.level == crate::net::Level::Error), "{:?}", n.check().iter().filter(|i| i.level == crate::net::Level::Error).collect::<Vec<_>>());
    }

    #[test]
    fn sports_hall_rides_on_a_radio_bridge() {
        let mut n = template_seeded("campus", 6);
        let st = id(&n, "Sports student 1");
        assert!(n.dev(st).unwrap().ifaces[0].ip.starts_with("10.1.30."), "{:?}", n.dev(st).unwrap().ifaces);
        assert!(n.exec(st, "open learn.campus.test").lines.join(" ").contains("200 OK"));
        let bridge = n.links.iter().find(|l| n.dev(l.a).unwrap().name.starts_with("Roof bridge") && n.dev(l.b).unwrap().name.starts_with("Roof bridge")).unwrap().clone();
        let aimed = n.link_signal(&bridge).unwrap();
        // Turn one dish away: the link collapses.
        let b = bridge.a;
        n.dev_mut(b).unwrap().aim += 90.0;
        let off = n.link_signal(&bridge).unwrap();
        assert!(off.rssi < aimed.rssi - 20.0, "{aimed:?} vs {off:?}");
    }

    #[test]
    fn three_routers_work_once_rip_is_on() {
        let mut n = template("rip");
        assert!(super::check_exercise(&n).is_err());
        let routers: Vec<u32> = n.devices.iter().filter(|d| d.kind == Kind::Router).map(|d| d.id).collect();
        for r in &routers {
            n.dev_mut(*r).unwrap().services.rip = true;
        }
        assert!(super::check_exercise(&n).is_ok(), "{:?}", super::check_exercise(&n));
        let c = n.dev(id(&n, "PC C")).unwrap().ifaces[0].ip.clone();
        let out = n.exec(id(&n, "PC A"), &format!("traceroute {c}")).lines.join("\n");
        assert!(out.contains("Router 3") && out.contains("reached"), "{out}");
        let out = n.exec(routers[0], "route").lines.join("\n");
        assert!(out.contains("R  metric 2"), "{out}");
    }

    #[test]
    fn small_flat_is_small_and_works() {
        let mut n = template("flat");
        assert!(n.check().iter().all(|i| i.level != crate::net::Level::Error), "{:?}", n.check());
        let c = n.coverage(0).unwrap();
        assert!((c.area_m2 - 58.5).abs() < 6.0 && c.good > 0.9, "{} m², {}", c.area_m2, c.good);
        let lap = n.devices.iter().find(|d| d.name == "Laptop").unwrap().id;
        let out = n.exec(lap, "open example.com").lines.join("\n");
        assert!(out.contains("200 OK"), "{out}");
    }

    #[test]
    fn every_scenario_builds_and_checks() {
        for (key, _, _) in super::SCENARIOS {
            let n = template(key);
            let _ = n.check();
            assert!(!n.title.is_empty());
        }
    }

    #[test]
    fn every_exercise_starts_unsolved_and_names_what_is_wrong() {
        for (key, _) in super::EXERCISES {
            let mut n = crate::net::template(key);
            let r = super::check_exercise(&n);
            assert!(r.as_ref().is_err_and(|e| e.starts_with("Not yet")), "{key}: {r:?}");
            // The one-click fixes solve the ones a click can solve.
            n.fix_all();
            if ["extender", "apartment"].contains(key) {
                assert!(super::check_exercise(&n).is_ok(), "{key}: {:?}", super::check_exercise(&n));
            }
        }
    }
}