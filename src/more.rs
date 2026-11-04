//! Resilience ("what if this breaks?"), power use, traffic by time of day, router-behind-router (WAN port),
//! extra Wi-Fi names per access point, a simple content filter with an ad/tracker blocklist, UPS runtime and duplicating devices.
use crate::extras::{Use, GUEST_VLAN};
use crate::net::{Device, Kind, Link, Network};
use std::collections::{HashSet, VecDeque};

/// A starter list of well-known ad and tracker domains; real lists (hosts files with hundreds of thousands of
/// names, updated daily) are imported on the router and kept in `Device::blocklist`.
pub const AD_LIST: [&str; 8] = ["doubleclick.net", "googlesyndication.com", "googleadservices.com", "adnxs.com", "scorecardresearch.com", "taboola.com", "criteo.com", "ads.example.com"];

/// Typical power draw (W): while in use for TVs, computers and consoles (see `duty`), else the average over a
/// day including standby.
pub fn default_watts(k: Kind) -> f32 {
    use Kind::*;
    match k {
        Internet | Neighbor | Interference => 0.0,
        HomeRouter => 12.0,
        Router => 20.0,
        Switch => 3.0, // small unmanaged switch
        AccessPoint => 0.0, // powered over the cable: counted at its switch
        Extender => 5.0,
        Desktop => 60.0,
        Laptop => 20.0,
        Phone => 2.0,
        Server => 90.0,
        Nas => 25.0,
        Printer => 4.0,
        Tv => 80.0,
        SmartHome => 1.0,
        Speaker => 6.0,
        Camera => 0.0, // powered over the cable
        Console => 70.0,
    }
}

/// Standby draw (W) of devices that sleep when nobody uses them; None = on around the clock.
fn standby_w(k: Kind) -> Option<f32> {
    match k {
        Kind::Tv => Some(0.5),
        Kind::Desktop => Some(1.5),
        Kind::Laptop => Some(1.0),
        Kind::Console => Some(2.0),
        _ => None,
    }
}

/// Share of the day a device is switched on and used, as in a typical household survey: the traffic
/// curves below say when apps are busy, not how long the screen is on.
pub fn duty(d: &Device) -> f32 {
    let hours = match d.kind {
        Kind::Tv => 4.0,
        Kind::Desktop => 6.0,
        Kind::Laptop => 5.0,
        Kind::Console => 2.0,
        _ => 24.0,
    };
    hours / 24.0
}

/// How active an app is at a given hour, relative to its busy-hour demand (0..1).
pub fn activity(u: Use, hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    let evening: f32 = if (18.0..23.5).contains(&h) { 1.0 } else if (7.0..18.0).contains(&h) { 0.35 } else { 0.05 };
    let daytime: f32 = if (8.0..18.0).contains(&h) { 1.0 } else if (18.0..22.0).contains(&h) { 0.4 } else { 0.05 };
    match u {
        Use::Stream4k | Use::StreamHd | Use::Gaming | Use::Download | Use::Music => evening,
        Use::VideoCall | Use::Browsing => daytime.max(evening * 0.6),
        // Backups usually run at night, when nobody notices.
        Use::Backup => if (1.0..5.0).contains(&h) { 1.0 } else { 0.05 },
        Use::Camera => 1.0,
    }
}

pub fn clock(hour: f32) -> String {
    let h = hour.rem_euclid(24.0);
    format!("{:02}:{:02}", h as u32, ((h.fract()) * 60.0).round() as u32 % 60)
}

/// The names in a blocklist file, one per line: plain lists ("ads.example.com") and hosts files
/// ("0.0.0.0 ads.example.com") both work; comments, localhost lines and duplicates are dropped.
pub fn parse_blocklist(text: &str) -> String {
    let mut seen = HashSet::new();
    text.lines()
        .filter_map(|l| {
            let l = l.split('#').next()?.trim();
            let name = l.split_whitespace().last()?.trim_end_matches('.').to_lowercase();
            (name.contains('.') && !name.starts_with("localhost") && name.parse::<std::net::IpAddr>().is_err() && seen.insert(name.clone())).then_some(name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Network {
    /// The link two neighbours actually use: the main line while it works, the mobile backup otherwise.
    pub fn link_between(&self, a: u32, b: u32) -> Option<&Link> {
        let pair = |l: &&Link| (l.a == a && l.b == b) || (l.a == b && l.b == a);
        let ok = |l: &&Link| self.link_ok(l);
        self.links
            .iter()
            .filter(pair)
            .filter(ok)
            .min_by_key(|l| l.medium == crate::net::Medium::Cellular)
            .or_else(|| self.links.iter().find(pair))
    }

    /// A link that carries traffic right now: plugged in and both ends switched on.
    pub fn link_ok(&self, l: &Link) -> bool {
        !l.unplugged && self.dev(l.a).is_some_and(|d| !d.off) && self.dev(l.b).is_some_and(|d| !d.off)
    }

    /// Devices reachable from `start` over working links, without passing through `without`.
    fn reach(&self, start: u32, without: Option<u32>) -> HashSet<u32> {
        let mut seen = HashSet::from([start]);
        let mut q = VecDeque::from([start]);
        while let Some(c) = q.pop_front() {
            // End devices don't pass traffic on.
            if c != start && self.dev(c).is_some_and(|d| d.kind.is_host()) {
                continue;
            }
            for l in self.links.iter().filter(|l| (l.a == c || l.b == c) && self.link_ok(l)) {
                let o = l.other(c).0;
                if Some(o) != without && seen.insert(o) {
                    q.push_back(o);
                }
            }
        }
        seen
    }

    /// Single points of failure: for each piece of network gear, how many devices lose their way to the
    /// internet (or to the main router when there's no internet) if it fails. Worst first.
    pub fn weak_spots(&self) -> Vec<(u32, usize)> {
        let root = self.devices.iter().find(|d| d.kind == Kind::Internet).or_else(|| self.devices.iter().find(|d| d.kind.routes())).map(|d| d.id);
        let Some(root) = root else { return vec![] };
        let users = |set: &HashSet<u32>| self.devices.iter().filter(|d| d.kind.is_host() && set.contains(&d.id)).count();
        let base = users(&self.reach(root, None));
        let mut v: Vec<(u32, usize)> = self
            .devices
            .iter()
            .filter(|d| d.id != root && !d.kind.is_host() && !d.kind.foreign() && !d.off)
            .map(|d| (d.id, base.saturating_sub(users(&self.reach(root, Some(d.id))))))
            .filter(|x| x.1 > 0)
            .collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }

    /// Average power of everything in the plan (W), with PoE devices counted once (at their switch, plus losses).
    /// Network gear runs 24/7; TVs, computers and consoles only while in use, on standby the rest of the day.
    pub fn power_w(&self) -> f32 {
        let own: f32 = self
            .devices
            .iter()
            .filter(|d| !d.off)
            .map(|d| match standby_w(d.kind) {
                Some(sb) => d.watts * duty(d) + sb * (1.0 - duty(d)),
                None => d.watts,
            })
            .sum();
        let poe: f32 = self.devices.iter().filter(|d| !d.off && d.poe_draw > 0.0).map(|d| d.poe_draw * 0.6 * 1.1).sum();
        own + poe
    }

    /// Who powers a device: itself, or the switch it gets PoE from.
    fn power_source(&self, d: &Device) -> u32 {
        let src = self.links.iter().filter(|_| d.poe_draw > 0.0).find(|l| (l.a == d.id || l.b == d.id) && l.medium.takes_port()).and_then(|l| self.dev(l.other(d.id).0));
        src.filter(|s| s.poe_class > 0).map_or(d.id, |s| s.id)
    }

    /// Each UPS during a power cut: (device it's on, watts it carries — what it powers over cables included,
    /// hours it lasts). About 15% of the battery goes to the UPS's own conversion losses.
    pub fn ups_runtime(&self) -> Vec<(u32, f32, f32)> {
        self.devices
            .iter()
            .filter(|d| d.ups_wh > 0.0 && !d.off)
            .map(|d| {
                let poe: f32 = self.devices.iter().filter(|o| o.id != d.id && !o.off && self.power_source(o) == d.id).map(|o| o.poe_draw * 1.1).sum();
                let w = d.watts + poe;
                (d.id, w, d.ups_wh * 0.85 / w.max(0.1))
            })
            .collect()
    }

    /// Weak spots (see `weak_spots`) that a power cut switches off: no UPS of their own or at their PoE switch.
    pub fn no_ups(&self) -> Vec<u32> {
        self.weak_spots().into_iter().map(|x| x.0).filter(|id| self.dev(*id).and_then(|d| self.dev(self.power_source(d))).is_some_and(|p| p.ups_wh <= 0.0)).collect()
    }

    pub fn yearly_kwh(&self) -> f32 {
        self.power_w() * 24.0 * 365.0 / 1000.0
    }

    /// Makes `link` this home router's internet (WAN) port — a router behind another router.
    pub fn set_wan(&mut self, router: u32, link: u32, on: bool) {
        let port = if on { 1 } else { 0 };
        if let Some(l) = self.links.iter_mut().find(|l| l.id == link) {
            if l.a == router {
                l.a_if = port;
            } else if l.b == router {
                l.b_if = port;
            }
            // The cable moves over to the WAN side (or back): assign_ports picks the port.
            l.set_port_at(router, None);
        }
        self.assign_ports();
        let d = self.dev_mut(router).unwrap();
        d.dhcp_client = true;
    }

    /// Every network name a radio broadcasts, with the VLAN it leads to.
    pub fn names_of(d: &Device) -> Vec<(String, u16)> {
        let mut v = vec![(d.ssid.clone(), 1)];
        v.extend(d.ssids.iter().cloned());
        if d.ifaces.iter().any(|f| f.vlan == GUEST_VLAN) && !d.guest_ssid.is_empty() {
            v.push((d.guest_ssid.clone(), GUEST_VLAN));
        }
        v
    }

    /// The network name a Wi-Fi client is on.
    pub fn joined<'a>(cl: &'a Device, ap: &'a Device) -> &'a str {
        if cl.join.is_empty() { &ap.ssid } else { &cl.join }
    }

    /// Joins a client to one of its access point's names; it lands in that name's VLAN.
    pub fn join(&mut self, client: u32, name: &str, vlan: u16) {
        let d = self.dev_mut(client).unwrap();
        d.join = name.to_string();
        if let Some(f) = d.ifaces.first_mut() {
            f.vlan = vlan;
        }
        let mut log = vec![];
        self.dhcp_one(client, &mut log);
    }

    /// A router's content filter: "block-site example.net" lines and, when on, the ad and tracker blocklist
    /// (both also cover sub-domains).
    pub fn blocked_by(&self, d: &Device, name: &str) -> Option<String> {
        let hit = |site: &str| name == site || name.ends_with(&format!(".{site}"));
        let own = d.rules.lines().find_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let site = w.get(1)?;
            (w.first() == Some(&"block-site") && hit(site)).then(|| site.to_string())
        });
        own.or_else(|| AD_LIST.iter().copied().chain(d.blocklist.lines()).find(|s| d.adblock && !s.is_empty() && hit(s)).map(|s| format!("{s}, ad and tracker blocklist")))
    }

    /// Points a directional antenna straight at another device.
    pub fn aim_at(&mut self, id: u32, target: u32) {
        let Some(t) = self.dev(target).map(|d| d.pos) else { return };
        let d = self.dev_mut(id).unwrap();
        d.aim = (t[1] - d.pos[1]).atan2(t[0] - d.pos[0]).to_degrees().rem_euclid(360.0);
    }

    /// A copy of a device next to it, with fresh addresses and no cables.
    pub fn duplicate(&mut self, id: u32) -> Option<u32> {
        let src = self.dev(id)?.clone();
        let new = self.add_device(src.kind, [src.pos[0] + 50.0, src.pos[1] + 50.0]);
        let fresh = self.dev(new).unwrap().clone();
        let d = self.dev_mut(new).unwrap();
        *d = Device { id: new, name: format!("{} copy", src.name), pos: fresh.pos, ifaces: fresh.ifaces, ..src };
        if d.kind.is_host() {
            d.dhcp_client = true;
            d.gateway.clear();
            d.dns.clear();
        }
        Some(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{template_seeded, Medium};

    fn id(n: &Network, name: &str) -> u32 {
        n.devices.iter().find(|d| d.name == name).unwrap().id
    }

    #[test]
    fn failures_and_weak_spots() {
        let mut n = template_seeded("home", 1);
        let spots = n.weak_spots();
        // The home router carries everyone: it's the worst single point of failure.
        assert_eq!(n.dev(spots[0].0).unwrap().kind, Kind::HomeRouter);
        let pc = id(&n, "Office PC");
        assert!(n.exec(pc, "ping example.com").lines.join(" ").contains(", 4 received"));
        let sw = id(&n, "Switch 1");
        n.dev_mut(sw).unwrap().off = true;
        let out = n.exec(pc, "ping example.com").lines.join(" ");
        assert!(out.contains("100%") || out.contains("unreachable") || out.contains("No working path"), "{out}");
        n.dev_mut(sw).unwrap().off = false;
        let l = n.links.iter().position(|l| l.a == pc || l.b == pc).unwrap();
        n.links[l].unplugged = true;
        assert!(!n.exec(pc, "ping example.com").lines.join(" ").contains(", 4 received"));
    }

    #[test]
    fn router_behind_router_is_double_nat_and_still_works() {
        let mut n = template_seeded("home", 2);
        let sw = id(&n, "Switch 1");
        let r2 = n.add_device(Kind::HomeRouter, [700.0, 700.0]);
        n.dev_mut(r2).unwrap().ifaces[0].ip = "192.168.50.1".into();
        n.dev_mut(r2).unwrap().dhcp_from = "192.168.50.100".into();
        n.dev_mut(r2).unwrap().dhcp_to = "192.168.50.199".into();
        let l = n.connect(sw, r2, Medium::Cat6).unwrap();
        n.set_wan(r2, l, true);
        let lap = n.add_device(Kind::Laptop, [760.0, 760.0]);
        n.connect(r2, lap, Medium::WiFi).unwrap();
        n.run_dhcp();
        assert!(n.dev(r2).unwrap().ifaces[1].ip.starts_with("192.168.1."));
        assert!(n.exec(lap, "ping example.com").lines.join(" ").contains(", 4 received"));
        assert!(n.check().iter().any(|i| i.text.contains("Double NAT")));
    }

    #[test]
    fn extra_names_lead_into_vlans_and_filters_block_sites() {
        let mut n = template_seeded("campus", 3);
        let st = id(&n, "Library student 2");
        let ap = n.wifi_ends(n.wifi_link_of(st).unwrap()).unwrap().0.id;
        let (name, vlan) = n.dev(ap).unwrap().ssids[0].clone();
        n.join(st, &name, vlan);
        assert!(n.dev(st).unwrap().ifaces[0].ip.starts_with(&format!("10.1.{vlan}.")), "{:?}", n.dev(st).unwrap().ifaces);
        let mut h = template_seeded("home", 1);
        let r = id(&h, "Home router");
        h.dev_mut(r).unwrap().rules = "block-site video.example.net".into();
        let pc = id(&h, "Office PC");
        assert!(h.exec(pc, "open video.example.net").lines.join(" ").contains("Blocked"));
        assert!(h.exec(pc, "open example.com").lines.join(" ").contains("200 OK"));
    }

    #[test]
    fn mobile_backup_takes_over_when_the_line_fails() {
        let mut n = template_seeded("home", 1);
        let (inet, r) = (n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id, id(&n, "Home router"));
        let backup = n.connect(r, inet, Medium::Cellular).unwrap();
        let pc = id(&n, "Office PC");
        let normal = n.exec(pc, "speedtest").lines.join(" ");
        assert!(!normal.contains("Mobile"), "{normal}");
        let fibre = n.links.iter().find(|l| l.medium == Medium::Fiber).unwrap().id;
        n.links.iter_mut().find(|l| l.id == fibre).unwrap().unplugged = true;
        let out = n.exec(pc, "speedtest").lines.join(" ");
        assert!(out.contains("Mobile 5G backup") || out.contains("300"), "{out}");
        // Behind the mobile line's carrier-grade NAT, port forwarding can't work.
        let nas_ip = n.dev(id(&n, "NAS 1")).unwrap().first_ip().unwrap();
        n.dev_mut(r).unwrap().rules = format!("forward 443 {nas_ip}");
        let wan = n.dev(r).unwrap().ifaces[1].ip.clone();
        let from_outside = n.exec(inet, &format!("open {wan}")).lines.join(" ");
        assert!(from_outside.contains("carrier-grade NAT"), "{from_outside}");
        let cg = n.cgnat(n.dev(r).unwrap()).unwrap();
        assert!(cg.starts_with("100.") && n.exec(r, "ipconfig").lines.join(" ").contains(&format!("{cg}/10")));
        let ping = n.exec(pc, "ping example.com");
        assert!(ping.lines.join(" ").contains(", 4 received"));
        // Out: the router translates to its 100.64 address, the provider to the shared one; back: the reverse.
        let nat: Vec<&str> = ping.log.iter().filter(|l| l.proto == "NAT").map(|l| l.info.as_str()).collect();
        assert!(nat.iter().any(|i| i.contains(&format!("mobile-line address {cg}"))) && nat.iter().any(|i| i.contains("carrier-grade NAT replaces")), "{nat:?}");
        assert!(nat.iter().any(|i| i.contains(&format!("back to {cg}"))), "{nat:?}");
        assert!(n.check().iter().any(|i| i.link == Some(backup) && i.text.contains("mobile backup")));
        // Without a backup, the same failure takes the house offline.
        n.remove_link(backup);
        assert!(!n.exec(pc, "ping example.com").lines.join(" ").contains(", 4 received"));
    }

    /// One of every kind of device in a working home: each gets an address, reaches the internet the way it
    /// should, and every command and check runs without trouble.
    #[test]
    fn every_kind_of_device_works() {
        let mut n = template_seeded("blank", 7);
        let inet = n.add_device(Kind::Internet, [0.0, 0.0]);
        let r = n.add_device(Kind::HomeRouter, [200.0, 0.0]);
        let sw = n.add_device(Kind::Switch, [400.0, 0.0]);
        n.dev_mut(sw).unwrap().ports = vec![crate::ports::PortGroup::rj45(24, 1000)];
        n.dev_mut(sw).unwrap().poe_class = 2;
        n.dev_mut(sw).unwrap().poe_budget = 200.0;
        n.connect(inet, r, Medium::Fiber).unwrap();
        n.connect(r, sw, Medium::Cat6).unwrap();
        let mut report = vec![];
        for (i, k) in Kind::SHELF.into_iter().enumerate() {
            if matches!(k, Kind::Internet | Kind::HomeRouter) {
                continue;
            }
            let id = n.add_device(k, [300.0 + (i % 6) as f32 * 60.0, 120.0 + (i / 6) as f32 * 60.0]);
            let res = match k {
                Kind::Neighbor | Kind::Interference => n.connect(r, id, Medium::Cat6).err().map(|_| ()).ok_or(()),
                Kind::Phone | Kind::SmartHome | Kind::Laptop => n.connect(r, id, Medium::WiFi).map(|_| ()).map_err(|_| ()),
                Kind::Extender => n.connect(r, id, Medium::WiFi).map(|_| ()).map_err(|_| ()),
                _ => n.connect(sw, id, Medium::Cat6).map(|_| ()).map_err(|_| ()),
            };
            report.push((k, res));
        }
        n.run_dhcp();
        let issues = n.check();
        for (k, res) in &report {
            assert!(res.is_ok(), "{k:?}: connecting didn't behave");
            let d = n.devices.iter().find(|d| d.kind == *k).unwrap().clone();
            if k.is_host() {
                assert!(d.ifaces[0].ip.starts_with("192.168."), "{k:?} got {:?}", d.ifaces[0].ip);
                let out = n.exec(d.id, "ping example.com").lines.join(" ");
                assert!(out.contains(" received"), "{k:?}: {out}");
                for cmd in ["ipconfig", "traceroute example.com", "nslookup example.com", "speedtest", "wifi", "stp", "vlan", "arp", "route", "help"] {
                    let _ = n.exec(d.id, cmd);
                }
            }
            // Radios that serve others must be on air with real settings.
            if k.transmits() && *k != Kind::Interference {
                assert!(d.bands.iter().zip(d.width).zip(d.tx).all(|((on, w), t)| !on || (w > 0 && t > 0.0)), "{k:?}: {:?} {:?} {:?}", d.bands, d.width, d.tx);
            }
            assert!(issues.iter().all(|i| i.level != crate::net::Level::Error || i.dev != Some(d.id)), "{k:?}: {:?}", issues.iter().filter(|i| i.dev == Some(d.id)).collect::<Vec<_>>());
        }
        let _ = (n.weak_spots(), n.power_w(), n.loads(), n.stp());
    }

    #[test]
    fn ups_runtime_counts_what_it_powers() {
        let mut n = template_seeded("home", 1);
        let r = id(&n, "Home router");
        assert!(n.no_ups().contains(&r));
        n.dev_mut(r).unwrap().ups_wh = 100.0;
        let w = n.dev(r).unwrap().watts;
        let (dev, carried, hours) = n.ups_runtime()[0];
        assert!(dev == r && carried >= w && (hours - 85.0 / carried).abs() < 1e-3, "{carried} {hours}");
        assert!(!n.no_ups().contains(&r));
    }

    #[test]
    fn power_and_time_of_day() {
        let mut n = template_seeded("home", 1);
        let kwh = n.yearly_kwh();
        assert!(n.power_w() > 50.0 && (400.0..1500.0).contains(&kwh), "{kwh}");
        // A TV is on for part of the day: far below 80 W around the clock (700 kWh), above standby alone.
        let tv = id(&n, "Living room TV");
        let one = |n: &Network| n.yearly_kwh();
        n.dev_mut(tv).unwrap().off = true;
        let tv_kwh = kwh - one(&n);
        n.dev_mut(tv).unwrap().off = false;
        assert!((50.0..400.0).contains(&tv_kwh), "{tv_kwh}");
        let tv = id(&n, "Living room TV");
        let l = n.links.iter().find(|l| l.a == tv || l.b == tv).unwrap().id;
        let evening = n.utilisation(&n.loads(), l);
        n.hour = 4.0;
        assert!(n.utilisation(&n.loads(), l) < evening / 5.0);
        let copy = n.duplicate(tv).unwrap();
        assert_eq!(n.dev(copy).unwrap().kind, Kind::Tv);
        assert!(n.dev(copy).unwrap().ifaces[0].mac != n.dev(tv).unwrap().ifaces[0].mac);
    }

    #[test]
    fn an_imported_hosts_file_blocks_its_names_and_their_subdomains() {
        let list = super::parse_blocklist("# StevenBlack-style hosts file\n127.0.0.1 localhost\n0.0.0.0 tracker.example.org\n0.0.0.0 tracker.example.org\nads.example.net\n");
        assert_eq!(list, "tracker.example.org\nads.example.net");
        let mut n = template_seeded("home", 1);
        let r = id(&n, "Home router");
        n.dev_mut(r).unwrap().blocklist = list;
        let d = n.dev(r).unwrap().clone();
        assert!(n.blocked_by(&d, "cdn.tracker.example.org").is_none(), "only with the blocker on");
        n.dev_mut(r).unwrap().adblock = true;
        let d = n.dev(r).unwrap().clone();
        assert!(n.blocked_by(&d, "cdn.tracker.example.org").is_some() && n.blocked_by(&d, "doubleclick.net").is_some());
        assert!(n.blocked_by(&d, "example.org").is_none());
    }
}