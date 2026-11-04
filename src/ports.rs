//! Ethernet ports: what a device has ("1 × 10G SFP+ WAN, 4 × 1G"), which port each cable plugs into,
//! and the speed that port allows. Cables keep their port; new ones take the best free port.
use crate::net::{Device, Kind, Link, Medium, Network};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortKind {
    #[default]
    Rj45,
    /// An SFP/SFP+ cage or a built-in fibre (PON) socket.
    Fiber,
    /// The phone line of a DSL modem.
    Dsl,
    /// The cable-TV line of a cable modem.
    Coax,
}

impl PortKind {
    pub const ALL: [PortKind; 4] = [PortKind::Rj45, PortKind::Fiber, PortKind::Dsl, PortKind::Coax];
    pub fn label(self) -> &'static str {
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
        match self {
            PortKind::Rj45 => "RJ45",
            PortKind::Fiber => "SFP / fibre",
            PortKind::Dsl => "DSL",
            PortKind::Coax => "Coax",
        }
    }
    /// For narrow buttons.
    pub fn short(self) -> &'static str {
        match self {
            PortKind::Fiber => "SFP",
            k => k.label(),
        }
    }
    /// A DSL or cable-TV socket only takes the internet line.
    pub fn line(self) -> bool {
        matches!(self, PortKind::Dsl | PortKind::Coax)
    }
}

/// Identical ports, like "4 × 1 Gbit/s RJ45".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct PortGroup {
    pub n: u8,
    pub mbps: u32,
    #[serde(default)]
    pub kind: PortKind,
    /// Meant for the internet line; other cables only take it when nothing else is free.
    #[serde(default)]
    pub wan: bool,
}

impl PortGroup {
    pub fn rj45(n: u8, mbps: u32) -> Self {
        PortGroup { n, mbps, kind: PortKind::Rj45, wan: false }
    }
    pub fn wan(mut self) -> Self {
        self.wan = true;
        self
    }
}

/// "10G", "2.5G", "300M"
pub fn short_speed(mbps: u32) -> String {
    match mbps {
        m if m >= 1000 && m % 1000 == 0 => format!("{}G", m / 1000),
        m if m >= 1000 => format!("{:.1}G", m as f32 / 1000.0),
        m => format!("{m}M"),
    }
}

/// "1×10G SFP, 4×1G": what a device has, for one-line summaries.
pub fn ports_summary(groups: &[PortGroup]) -> String {
    let kind = |k: PortKind| match k {
        PortKind::Rj45 => "",
        PortKind::Fiber => " SFP",
        PortKind::Dsl => " DSL",
        PortKind::Coax => " coax",
    };
    groups.iter().map(|g| format!("{}×{}{}", g.n, short_speed(g.mbps), kind(g.kind))).collect::<Vec<_>>().join(", ")
}

/// Port speeds the editor offers for Ethernet and SFP; DSL and coax take any line rate.
pub const SPEEDS: [u32; 5] = [100, 1000, 2500, 5000, 10000];

impl Medium {
    /// Plugs into an Ethernet port at both ends (a powerline adapter does too).
    pub fn takes_port(self) -> bool {
        !self.wireless() && self != Medium::Cellular
    }
}

impl Link {
    pub fn port_at(&self, id: u32) -> Option<u16> {
        if self.a == id { self.a_port } else { self.b_port }
    }
    pub fn set_port_at(&mut self, id: u32, p: Option<u16>) {
        if self.a == id { self.a_port = p } else { self.b_port = p }
    }
}

impl Device {
    pub fn port_count(&self) -> usize {
        self.ports.iter().map(|g| g.n as usize).sum()
    }
    /// The group port `i` (0-based) belongs to.
    pub fn port(&self, i: u16) -> Option<PortGroup> {
        let mut i = i as usize;
        for g in &self.ports {
            if i < g.n as usize {
                return Some(*g);
            }
            i -= g.n as usize;
        }
        None
    }
    /// Fastest Ethernet or fibre port (0 = none).
    pub fn fastest_port(&self) -> u32 {
        self.ports.iter().filter(|g| !g.kind.line()).map(|g| g.mbps).max().unwrap_or(0)
    }
}

/// How well a cable fits a port, best first: the right socket (not an adapter), the right side (WAN for the
/// internet line, LAN otherwise), then speed. None = it can't go in at all.
fn rank(g: PortGroup, medium: Medium, internet: bool) -> Option<(u8, u8, u32)> {
    let socket = match g.kind {
        _ if g.kind.line() && !internet => return None,
        PortKind::Dsl | PortKind::Coax => 2,
        PortKind::Fiber => if medium == Medium::Fiber { 2 } else { 1 },
        PortKind::Rj45 => if medium == Medium::Fiber { 1 } else { 2 },
    };
    Some((socket, (g.wan == internet) as u8, g.mbps))
}

impl Network {
    /// Whether this cable is the device's internet line: to the internet itself, or a router's WAN side.
    pub fn internet_end(&self, l: &Link, id: u32) -> bool {
        let d = self.dev(id);
        self.dev(l.other(id).0).is_some_and(|o| o.kind == Kind::Internet) || (d.is_some_and(|d| d.kind == Kind::HomeRouter) && l.end_if(id) == 1)
    }

    fn port_rank(&self, l: &Link, id: u32, p: u16) -> Option<(u8, u8, u32)> {
        rank(self.dev(id)?.port(p)?, l.medium, self.internet_end(l, id))
    }

    /// Whether a cable can go into this port at all.
    pub fn fits_port(&self, l: &Link, id: u32, p: u16) -> bool {
        self.port_rank(l, id, p).is_some()
    }

    /// Who is plugged into which port of a device: (port, the device at the other end).
    pub fn port_users(&self, id: u32) -> Vec<(u16, String)> {
        self.links.iter().filter(|l| (l.a == id || l.b == id) && l.medium.takes_port()).filter_map(|l| Some((l.port_at(id)?, self.name(l.other(id).0)))).collect()
    }

    /// Ports of a device that no cable uses.
    pub fn free_ports(&self, id: u32) -> Vec<u16> {
        let total = self.dev(id).map_or(0, |d| d.port_count()).min(256);
        let used: Vec<u16> = self.links.iter().filter(|l| (l.a == id || l.b == id) && l.medium.takes_port()).filter_map(|l| l.port_at(id)).collect();
        (0..total as u16).filter(|p| !used.contains(p)).collect()
    }

    /// The free port a cable would be best off in.
    pub fn best_free_port(&self, l: &Link, id: u32) -> Option<u16> {
        self.free_ports(id).into_iter().filter_map(|p| self.port_rank(l, id, p).map(|r| (r, Reverse(p), p))).max().map(|x| x.2)
    }

    /// A free port of the same kind and side that's faster, when this end's port is what holds the cable back.
    pub fn faster_free_port(&self, l: &Link, id: u32) -> Option<u16> {
        let now = self.port_rank(l, id, l.port_at(id)?)?;
        let cap = self.cable_mbps(l).min(self.end_mbps(l, l.other(id).0));
        if now.2 >= cap {
            return None;
        }
        let q = self.best_free_port(l, id)?;
        let then = self.port_rank(l, id, q)?;
        ((then.0, then.1) >= (now.0, now.1) && then.2 > now.2).then_some(q)
    }

    /// Speed allowed by the port a cable end is in (the device's fastest port while it has none).
    pub fn end_mbps(&self, l: &Link, id: u32) -> u32 {
        let Some(d) = self.dev(id) else { return 1000 };
        let s = l.port_at(id).and_then(|p| d.port(p)).map(|g| g.mbps).unwrap_or(d.fastest_port());
        if s > 0 { s } else { 1000 }
    }

    /// Gives every cable end a port: cables stay where they are while they fit, the rest take the best free
    /// port (the internet line first, then the cables to the fastest devices). The internet cloud has no ports.
    pub fn assign_ports(&mut self) {
        let mut set: Vec<(usize, u32, Option<u16>)> = vec![];
        for d in self.devices.iter().filter(|d| d.kind != Kind::Internet) {
            let total = d.port_count();
            let mut used = vec![false; total];
            let mut todo = vec![];
            for (i, l) in self.links.iter().enumerate().filter(|(_, l)| (l.a == d.id || l.b == d.id) && l.medium.takes_port()) {
                match l.port_at(d.id).filter(|p| (*p as usize) < total && !used[*p as usize] && self.port_rank(l, d.id, *p).is_some()) {
                    Some(p) => used[p as usize] = true,
                    None => todo.push(i),
                }
            }
            let peer_speed = |l: &Link| self.dev(l.other(d.id).0).map_or(0, |o| if o.kind == Kind::Internet { u32::MAX } else { o.fastest_port() });
            todo.sort_by_key(|&i| (!self.internet_end(&self.links[i], d.id), Reverse(peer_speed(&self.links[i])), self.links[i].id));
            for i in todo {
                let l = &self.links[i];
                let best = (0..total).filter(|&p| !used[p]).filter_map(|p| self.port_rank(l, d.id, p as u16).map(|r| (r, Reverse(p), p))).max().map(|x| x.2);
                if let Some(p) = best {
                    used[p] = true;
                }
                if best.map(|p| p as u16) != l.port_at(d.id) {
                    set.push((i, d.id, best.map(|p| p as u16)));
                }
            }
        }
        for (i, id, p) in set {
            self.links[i].set_port_at(id, p);
        }
    }

    /// New hardware: every cable of this device is plugged into the best port again.
    pub fn replug(&mut self, id: u32) {
        for l in self.links.iter_mut().filter(|l| l.a == id || l.b == id) {
            l.set_port_at(id, None);
        }
        self.assign_ports();
    }

    /// Moves a cable end to another port of its device; a cable already there swaps into the old one.
    pub fn move_port(&mut self, link: u32, id: u32, to: u16) {
        let from = self.links.iter().find(|l| l.id == link).and_then(|l| l.port_at(id));
        if let Some(other) = self.links.iter_mut().find(|l| l.id != link && (l.a == id || l.b == id) && l.medium.takes_port() && l.port_at(id) == Some(to)) {
            other.set_port_at(id, from);
        }
        if let Some(l) = self.links.iter_mut().find(|l| l.id == link) {
            l.set_port_at(id, Some(to));
        }
    }

    /// "port 3 · 1G RJ45"
    pub fn port_name(&self, id: u32, p: u16) -> String {
        match self.dev(id).and_then(|d| d.port(p)) {
            Some(g) => format!("port {} · {} {}{}", p as u16 + 1, short_speed(g.mbps), g.kind.label(), if g.wan { " WAN" } else { "" }),
            None => format!("port {}", p as u16 + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::template_seeded;

    fn router_with(n: &mut Network, ports: Vec<PortGroup>) -> u32 {
        let r = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().id;
        n.dev_mut(r).unwrap().ports = ports;
        r
    }

    #[test]
    fn the_internet_line_takes_the_wan_port_and_the_fast_device_the_fast_port() {
        let mut n = template_seeded("home", 2);
        let r = router_with(&mut n, vec![PortGroup { n: 1, mbps: 10000, kind: PortKind::Fiber, wan: true }, PortGroup::rj45(1, 10000), PortGroup::rj45(3, 1000)]);
        for l in n.links.iter_mut() {
            l.a_port = None;
            l.b_port = None;
        }
        let nas = n.add_device(Kind::Nas, [600.0, 300.0]);
        n.dev_mut(nas).unwrap().ports = vec![PortGroup::rj45(1, 10000)];
        let l = n.connect(r, nas, Medium::Cat6a).unwrap();
        n.assign_ports();
        let inet = n.links.iter().find(|l| n.dev(l.other(r).0).is_some_and(|o| o.kind == Kind::Internet) && (l.a == r || l.b == r)).unwrap();
        assert_eq!(inet.port_at(r), Some(0), "fibre internet line in the SFP WAN port");
        let nl = n.links.iter().find(|x| x.id == l).unwrap();
        assert_eq!(nl.port_at(r), Some(1), "the 10G NAS gets the 10G LAN port");
        assert_eq!(n.negotiated(nl), 10000);
        // Moving it to a 1G port caps it, and the check offers to move it back.
        let to = n.free_ports(r)[0];
        n.move_port(l, r, to);
        let nl = n.links.iter().find(|x| x.id == l).unwrap().clone();
        assert_eq!(n.negotiated(&nl), 1000);
        let tip = n.check().into_iter().find(|i| i.fix == Some(crate::issues::Fix::MovePort(l, r))).expect("a faster port is free");
        n.apply_fix(tip.fix.as_ref().unwrap());
        assert_eq!(n.negotiated(n.links.iter().find(|x| x.id == l).unwrap()), 10000);
    }

    #[test]
    fn cables_keep_their_port_and_moving_onto_a_used_port_swaps() {
        let mut n = template_seeded("home", 3);
        n.assign_ports();
        let before: Vec<(Option<u16>, Option<u16>)> = n.links.iter().map(|l| (l.a_port, l.b_port)).collect();
        n.assign_ports();
        assert_eq!(before, n.links.iter().map(|l| (l.a_port, l.b_port)).collect::<Vec<_>>(), "stable");
        let sw = n.devices.iter().find(|d| d.kind == Kind::Switch).unwrap().id;
        let mine: Vec<(u32, u16)> = n.links.iter().filter(|l| (l.a == sw || l.b == sw) && l.medium.takes_port()).map(|l| (l.id, l.port_at(sw).unwrap())).collect();
        assert!(mine.len() >= 2);
        let ((l1, p1), (l2, p2)) = (mine[0], mine[1]);
        n.move_port(l1, sw, p2);
        let port = |l: u32| n.links.iter().find(|x| x.id == l).unwrap().port_at(sw);
        assert_eq!((port(l1), port(l2)), (Some(p2), Some(p1)));
    }

    #[test]
    fn line_sockets_only_take_the_internet_and_fibre_into_rj45_is_flagged() {
        let mut n = template_seeded("home", 4);
        let r = router_with(&mut n, vec![PortGroup { n: 1, mbps: 300, kind: PortKind::Dsl, wan: true }, PortGroup::rj45(4, 1000)]);
        n.assign_ports();
        for l in n.links.iter().filter(|l| (l.a == r || l.b == r) && l.medium.takes_port()) {
            let p = n.dev(r).unwrap().port(l.port_at(r).unwrap()).unwrap();
            let to_inet = n.dev(l.other(r).0).unwrap().kind == Kind::Internet;
            assert_eq!(p.kind == PortKind::Dsl, to_inet, "only the internet line uses the DSL socket");
            if to_inet {
                assert_eq!(n.negotiated(l), 300, "the DSL line caps the internet link");
            }
        }
        let sw = n.devices.iter().find(|d| d.kind == Kind::Switch).unwrap().id;
        let l = n.links.iter().find(|l| (l.a == r && l.b == sw) || (l.a == sw && l.b == r)).unwrap().id;
        n.links.iter_mut().find(|x| x.id == l).unwrap().medium = Medium::Fiber;
        n.assign_ports();
        assert!(n.check().iter().any(|i| i.link == Some(l) && i.text.contains("media converter")));
    }

    #[test]
    fn an_old_plan_keeps_its_port_count_and_speed() {
        let json = r#"{"devices":[{"id":1,"kind":"HomeRouter","name":"R","ports":4,"port_mbps":2500},{"id":2,"kind":"Switch","name":"S","ports":8,"port_mbps":1000}]}"#;
        let mut n: Network = serde_json::from_str(json).unwrap();
        n.sanitize();
        let r = n.dev(1).unwrap();
        assert_eq!((r.port_count(), r.fastest_port()), (5, 2500), "4 LAN ports plus the WAN port the old count left out");
        assert!(r.ports[0].wan);
        assert_eq!(n.dev(2).unwrap().ports, vec![PortGroup::rj45(8, 1000)]);
        let saved = serde_json::to_string(&n).unwrap();
        assert!(saved.contains("port_groups") && !saved.contains("\"port_mbps\""));
    }
}
