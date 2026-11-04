//! Built-in catalog of real routers, mesh nodes, range extenders, access points and outdoor bridges
//! (data/devices.json): pick a product and a device takes its radio, ports, power draw and price, instead of
//! typing every spec. Also Swiss internet plans (data/plans.json) and powerline kits (data/powerline.json).
use crate::net::{Device, Kind, WifiStd};
use crate::ports::{ports_summary, PortGroup};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Router,
    Mesh,
    Extender,
    Ap,
    /// Outdoor point-to-point radio with a directional antenna.
    Bridge,
}

/// One product as sold (one entry per node type for mesh kits).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub brand: String,
    /// As sold, without the brand.
    pub name: String,
    /// router: routes, with Wi-Fi (ISP boxes too); mesh: a mesh node that can also be the main router;
    /// extender: repeater, booster or satellite-only mesh node; ap: access point without routing (ceiling/wall, usually PoE).
    pub role: Role,
    /// Launch year.
    pub year: u16,
    /// Wi-Fi 4 (802.11n) hardware is listed as Wifi5 and its note starts with "Wi-Fi 4 (802.11n)".
    pub wifi: WifiStd,
    /// Most spatial streams on any one band, capped at 4.
    pub streams: u8,
    /// Radios for [2.4, 5, 6 GHz] and the widest channel each supports (2.4 GHz: the usual 20 MHz; 0 = no radio).
    pub bands: [bool; 3],
    pub width: [u16; 3],
    /// A radio set aside for the link back to the router (tri-band mesh, cross-band extender).
    pub backhaul: bool,
    /// Every wired socket (LAN, WAN, SFP/fibre, DSL, coax) as groups of identical ports, WAN side first.
    pub ports: Vec<PortGroup>,
    /// Typical average draw, and the most it takes over the cable when PoE-powered (datasheet max, 0 = own power supply).
    pub watts: f32,
    pub poe_w: f32,
    /// Street price of one unit (EUR/CHF, rounded; launch price if discontinued; 0 = only rented with a subscription).
    pub price: f32,
    /// One sentence for planners: WAN type, notable ports, mesh family; says so when a value is estimated.
    pub note: String,
    /// The page the specs were checked against.
    pub source: String,
    /// Outdoor bridges: antenna beam width (degrees); 0 or 360 = all around.
    #[serde(default)]
    pub beam: f32,
}

pub fn all() -> &'static [Product] {
    static ALL: OnceLock<Vec<Product>> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(include_str!("../data/devices.json")).expect("data/devices.json is checked by the tests"))
}

/// The product a device was set to, by its model name.
pub fn find(model: &str) -> Option<&'static Product> {
    all().iter().find(|p| p.title() == model)
}

/// Products that fit this kind of device and contain every word of the query (brand, name or note).
pub fn search(kind: Kind, query: &str) -> Vec<&'static Product> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    all().iter().filter(|p| p.fits(kind)).filter(|p| {
        let hay = format!("{} {} {}", p.title(), p.note, p.year).to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    }).collect()
}

impl Product {
    pub fn title(&self) -> String {
        format!("{} {}", self.brand, self.name)
    }

    /// Which devices on the map this product can be: routers and extenders can also run as plain access points.
    pub fn fits(&self, k: Kind) -> bool {
        let radio = self.bands.iter().any(|b| *b);
        match k {
            Kind::HomeRouter => matches!(self.role, Role::Router | Role::Mesh),
            // A plain router has no Wi-Fi settings: only wired routers and firewalls.
            Kind::Router => self.role == Role::Router && !radio,
            Kind::Neighbor => radio && matches!(self.role, Role::Router | Role::Mesh),
            Kind::Extender => matches!(self.role, Role::Extender | Role::Mesh),
            Kind::AccessPoint => radio,
            _ => false,
        }
    }

    /// "Wi-Fi 6 · 4×4 · 2.4 + 5 GHz (160 MHz) · 1×2.5G, 4×1G · 12 W"
    pub fn summary(&self) -> String {
        let names = ["2.4", "5", "6"];
        let bands: Vec<&str> = (0..3).filter(|&i| self.bands[i]).map(|i| names[i]).collect();
        let mut s = match (0..3).rev().find(|&i| self.bands[i]) {
            Some(i) => format!("{} · {n}×{n} · {} GHz ({} MHz)", short_std(self.wifi), bands.join(" + "), self.width[i], n = self.streams),
            None => "No Wi-Fi".to_string(),
        };
        if !self.ports.is_empty() {
            s += &format!(" · {}", ports_summary(&self.ports));
        }
        s + &format!(" · {} W", self.watts)
    }

    /// Copies the hardware onto a device (not its name, place or addresses). A neighbour only takes the radio.
    pub fn apply(&self, d: &mut Device) {
        d.model = self.title();
        d.wifi = self.wifi;
        d.streams = self.streams;
        d.bands = self.bands;
        d.width = self.width;
        // 0 = the EU limit per band, filled in by normalize_radio.
        d.tx = [0.0; 3];
        d.backhaul = self.backhaul;
        d.beam = if self.beam >= 360.0 { 0.0 } else { self.beam };
        // Plug its cables in again afterwards (Network::replug): the ports changed.
        if !d.kind.foreign() {
            d.ports = self.ports.clone();
            // A PoE device's power is counted at its switch (Network::power_w), not twice.
            d.watts = if self.poe_w > 0.0 { 0.0 } else { self.watts };
            d.poe_draw = self.poe_w;
            d.price = self.price;
        }
        crate::wifi::normalize_radio(d);
    }
}

/// An internet subscription as sold.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub provider: String,
    pub name: String,
    /// Advertised maximum (Mbit/s); only the download speed is simulated.
    pub down_mbps: f32,
    pub up_mbps: f32,
    /// fibre, dsl, cable or 5g
    pub line: String,
    /// CHF per month, regular price (0 = unknown).
    pub price_month: f32,
    /// The box that comes with it, as named in the device catalog where possible.
    pub router: String,
    pub note: String,
    pub source: String,
}

pub fn plans() -> &'static [Plan] {
    static ALL: OnceLock<Vec<Plan>> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(include_str!("../data/plans.json")).expect("data/plans.json is checked by the tests"))
}

impl Plan {
    /// "Swisscom blue Internet M (fibre)": the same plan name has other speeds on copper or 5G.
    pub fn title(&self) -> String {
        format!("{} {} ({})", self.provider, self.name, self.line)
    }
    /// "2000 / 2000 Mbit/s · fibre · CHF 69/month"
    pub fn summary(&self) -> String {
        let price = if self.price_month > 0.0 { format!(" · CHF {:.0}/month", self.price_month) } else { String::new() };
        format!("{:.0} / {:.0} Mbit/s · {}{price}", self.down_mbps, self.up_mbps, self.line)
    }
}

/// Plans containing every word of the query.
pub fn search_plans(query: &str) -> Vec<&'static Plan> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    plans().iter().filter(|p| {
        let hay = format!("{} {} {}", p.title(), p.line, p.router).to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    }).collect()
}

/// A pair of powerline adapters as sold.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerlineKit {
    pub brand: String,
    pub name: String,
    pub year: u16,
    /// HomePlug AV, HomePlug AV2 or G.hn
    pub standard: String,
    /// Advertised rate, and the real throughput between two adapters close together (Mbit/s).
    pub phy_mbps: u32,
    pub real_mbps: f32,
    /// Ethernet ports per adapter and their speed.
    pub ports: u8,
    pub port_mbps: u32,
    pub passthrough: bool,
    pub wifi: bool,
    /// Per adapter.
    pub watts: f32,
    /// Per kit of two.
    pub price: f32,
    pub note: String,
    pub source: String,
}

pub fn powerline() -> &'static [PowerlineKit] {
    static ALL: OnceLock<Vec<PowerlineKit>> = OnceLock::new();
    ALL.get_or_init(|| serde_json::from_str(include_str!("../data/powerline.json")).expect("data/powerline.json is checked by the tests"))
}

impl PowerlineKit {
    pub fn title(&self) -> String {
        format!("{} {}", self.brand, self.name)
    }
    /// "G.hn 2400 Mbit/s (2019) · 1×1G per adapter · pass-through socket · 3.5 W each"
    pub fn summary(&self) -> String {
        let extras = [(self.passthrough, " · pass-through socket"), (self.wifi, " · Wi-Fi")].iter().filter(|x| x.0).map(|x| x.1).collect::<String>();
        format!("{} {} Mbit/s ({}) · {}×{} per adapter{extras} · {} W each", self.standard, self.phy_mbps, self.year, self.ports, crate::ports::short_speed(self.port_mbps), self.watts)
    }
}

/// The powerline kit a link uses, by name.
pub fn find_kit(title: &str) -> Option<&'static PowerlineKit> {
    powerline().iter().find(|k| k.title() == title)
}

fn short_std(s: WifiStd) -> &'static str {
    match s {
        WifiStd::Wifi5 => "Wi-Fi 5",
        WifiStd::Wifi6 => "Wi-Fi 6",
        WifiStd::Wifi6E => "Wi-Fi 6E",
        WifiStd::Wifi7 => "Wi-Fi 7",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{template_seeded, Network};

    #[test]
    fn every_catalog_entry_is_plausible() {
        let all = all();
        assert!(all.len() >= 100, "only {} products", all.len());
        let mut seen = std::collections::HashSet::new();
        for p in all {
            let t = p.title();
            assert!(seen.insert(t.clone()), "{t} is listed twice");
            assert!(!p.brand.is_empty() && !p.name.is_empty() && !p.note.is_empty(), "{t}");
            assert!(p.source.starts_with("http"), "{t}: no source");
            assert!((1..=4).contains(&p.streams), "{t}: streams");
            assert!((2005..=2027).contains(&p.year), "{t}: year");
            assert!(p.bands.iter().any(|b| *b) || (p.role == Role::Router && p.note.starts_with("No Wi-Fi")), "{t}: no radio");
            for i in 0..3 {
                let max = p.wifi.max_width(i);
                assert_eq!(p.bands[i], p.width[i] > 0, "{t}: band {i} on/off and width disagree");
                assert!(p.width[i] <= max, "{t}: {} MHz on band {i} is more than {:?} allows", p.width[i], p.wifi);
            }
            assert!(p.watts > 0.0 && p.watts < 80.0, "{t}: {} W", p.watts);
            assert!(p.poe_w <= p.watts * 2.0 + 1.0, "{t}: PoE draw");
            assert!(p.price >= 0.0 && p.price < 2000.0, "{t}: price");
            let count: usize = p.ports.iter().map(|g| g.n as usize).sum();
            assert!(count <= 16 && p.ports.iter().all(|g| g.n > 0 && g.mbps > 0), "{t}: ports");
            for g in p.ports.iter().filter(|g| !g.kind.line()) {
                assert!(crate::ports::SPEEDS.contains(&g.mbps), "{t}: {} Mbit/s isn't an Ethernet speed", g.mbps);
            }
        }
        assert!(all.iter().filter(|p| p.brand == "Swisscom").count() >= 10, "all Swisscom boxes and boosters");
        for r in [Role::Router, Role::Mesh, Role::Extender, Role::Ap, Role::Bridge] {
            assert!(all.iter().any(|p| p.role == r), "{r:?}");
        }
    }

    #[test]
    fn a_catalog_router_replaces_the_hardware_but_keeps_the_device() {
        let mut n = template_seeded("home", 1);
        let id = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        let p = search(Kind::HomeRouter, "swisscom internet-box").into_iter().max_by_key(|p| p.year).expect("Swisscom routers are in the catalog");
        let d = n.dev_mut(id).unwrap();
        let (name, ssid) = (d.name.clone(), d.ssid.clone());
        p.apply(d);
        assert_eq!((d.name.clone(), d.ssid.clone(), d.model.clone()), (name, ssid, p.title()));
        assert_eq!((d.wifi, &d.ports, d.watts, d.price), (p.wifi, &p.ports, p.watts, p.price));
        for i in 0..3 {
            assert_eq!(d.bands[i], p.bands[i]);
            assert!(!d.bands[i] || d.tx[i] > 0.0, "an enabled band gets a real power");
        }
        assert_eq!(find(&d.model).map(|x| x.title()), Some(p.title()));
        // Its cables go into the new ports, the internet line on the WAN side.
        n.replug(id);
        let inet = n.links.iter().find(|l| (l.a == id || l.b == id) && n.dev(l.other(id).0).unwrap().kind == Kind::Internet).unwrap();
        let wan = n.dev(id).unwrap().port(inet.port_at(id).expect("plugged in")).unwrap();
        assert!(wan.wan, "{:?}", n.dev(id).unwrap().ports);
        n.check();
    }

    #[test]
    fn bridges_aim_and_wired_routers_fit_plain_routers() {
        let mut n = template_seeded("home", 5);
        let ap = n.add_device(Kind::AccessPoint, [300.0, 300.0]);
        let b = all().iter().find(|p| p.role == Role::Bridge && p.beam > 0.0 && p.beam < 360.0).expect("outdoor bridges");
        b.apply(n.dev_mut(ap).unwrap());
        assert_eq!(n.dev(ap).unwrap().beam, b.beam);
        let wired = search(Kind::Router, "");
        assert!(wired.len() >= 10 && wired.iter().all(|p| p.bands.iter().all(|x| !x)), "only wired routers for a plain router");
        assert!(search(Kind::HomeRouter, "").iter().any(|p| p.note.starts_with("No Wi-Fi")), "a gateway without Wi-Fi can be the home router");
    }

    #[test]
    fn plans_and_powerline_kits_are_plausible() {
        for p in plans() {
            let t = p.title();
            assert!((1.0..=25000.0).contains(&p.down_mbps) && p.up_mbps > 0.0 && p.up_mbps <= p.down_mbps * 1.01, "{t}: speeds");
            assert!(["fibre", "dsl", "cable", "5g"].contains(&p.line.as_str()), "{t}: line {}", p.line);
            assert!(p.source.starts_with("http") && (0.0..500.0).contains(&p.price_month), "{t}");
        }
        let titles: std::collections::HashSet<String> = plans().iter().map(|p| p.title()).collect();
        assert_eq!(titles.len(), plans().len(), "plan titles are unique");
        for provider in ["Swisscom", "Init7", "Sunrise", "Salt"] {
            assert!(plans().iter().any(|p| p.provider == provider), "{provider} plans");
        }
        assert!(powerline().len() >= 10);
        for k in powerline() {
            let t = k.title();
            assert!(k.real_mbps > 0.0 && k.real_mbps < k.phy_mbps as f32, "{t}: real speed");
            assert!(["HomePlug AV", "HomePlug AV2", "G.hn"].contains(&k.standard.as_str()), "{t}: {}", k.standard);
            assert!(k.watts > 0.0 && k.watts < 20.0 && k.price > 0.0 && k.source.starts_with("http"), "{t}");
        }
    }

    #[test]
    fn a_powerline_kit_sets_the_link_speed_and_price() {
        let mut n = template_seeded("home", 6);
        let l = n.links.iter().find(|l| l.medium.cable() && n.dev(l.a).unwrap().kind != Kind::Internet && n.dev(l.b).unwrap().kind != Kind::Internet).unwrap().id;
        fn link_of(n: &mut Network, l: u32) -> &mut crate::net::Link {
            n.links.iter_mut().find(|x| x.id == l).unwrap()
        }
        link_of(&mut n, l).medium = crate::net::Medium::Powerline;
        let rate = |n: &mut Network, kit: &str| {
            link_of(n, l).kit = kit.into();
            let x = n.links.iter().find(|x| x.id == l).unwrap().clone();
            n.link_rate(&x)
        };
        let slow = powerline().iter().min_by(|a, b| a.real_mbps.total_cmp(&b.real_mbps)).unwrap();
        let fast = powerline().iter().max_by(|a, b| a.real_mbps.total_cmp(&b.real_mbps)).unwrap();
        assert!(rate(&mut n, &slow.title()) < rate(&mut n, &fast.title()));
        // The shopping list prices the kit instead of a typical one.
        link_of(&mut n, l).kit.clear();
        let typical = crate::shopping(&n).1;
        link_of(&mut n, l).kit = fast.title();
        let extra = fast.price - crate::net::Medium::Powerline.per_link_price();
        assert!((crate::shopping(&n).1 - typical - extra).abs() < 0.01);
    }

    #[test]
    fn search_matches_words_in_any_order_and_respects_the_device_kind() {
        assert!(!search(Kind::Extender, "wlan-box SWISSCOM").is_empty());
        assert!(search(Kind::AccessPoint, "").iter().all(|p| p.bands.iter().any(|b| *b)), "an access point needs a radio");
        assert!(search(Kind::Extender, "").iter().all(|p| matches!(p.role, Role::Extender | Role::Mesh)));
        assert!(search(Kind::HomeRouter, "").iter().all(|p| matches!(p.role, Role::Router | Role::Mesh)));
        assert!(search(Kind::Laptop, "").is_empty());
        assert!(search(Kind::AccessPoint, "no such product xyz").is_empty());
    }
}
