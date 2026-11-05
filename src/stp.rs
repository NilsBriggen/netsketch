//! Spanning tree (802.1D/RSTP outcome): root election, the tree, blocked backup links, loops nobody blocks
//! and how long it takes to settle; link aggregation (LACP bundles count as one link).
use crate::net::{Kind, Network};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default, Debug)]
pub struct Stp {
    pub root: Option<u32>,
    /// Links spanning tree keeps as backup (not forwarding).
    pub blocked: HashSet<u32>,
    /// Links in loops that no spanning-tree device can break: broadcast storm.
    pub storm: Vec<u32>,
    /// Root path cost per bridge.
    pub cost: HashMap<u32, u32>,
    /// Each bridge's root port: the link it uses towards the root.
    pub root_port: HashMap<u32, u32>,
}

impl Network {
    /// The working cables bundled with `l` (LACP: same group number, same two ends), `l` included.
    pub fn bundle<'a>(&'a self, l: &'a crate::net::Link) -> Vec<&'a crate::net::Link> {
        if l.lag == 0 {
            return vec![l];
        }
        self.links.iter().filter(|o| o.id == l.id || (o.lag == l.lag && o.medium.cable() && self.link_ok(o) && ((o.a, o.b) == (l.a, l.b) || (o.a, o.b) == (l.b, l.a)))).collect()
    }

    /// Devices that forward frames between their ports (home routers on their LAN side).
    fn bridge_ends(&self, id: u32, iface: usize) -> bool {
        self.dev(id).is_some_and(|d| d.kind.bridges() || (d.kind == Kind::HomeRouter && iface == 0))
    }

    /// Bridge id: priority, then the lowest MAC address wins.
    pub fn bridge_id(&self, id: u32) -> (u16, String) {
        let d = self.dev(id).unwrap();
        (d.priority, d.ifaces.first().map(|f| f.mac.clone()).unwrap_or_default())
    }

    /// Speed a port reports to spanning tree (Mbit/s): the negotiated Ethernet speed, or the radio's link rate.
    pub fn port_speed(&self, l: &crate::net::Link) -> u32 {
        match self.signal(l, false) {
            Some(s) => s.phy.max(1.0) as u32,
            None => self.negotiated(l),
        }
    }

    /// What it costs `node` to receive over a port of this speed. Devices without spanning tree pass
    /// the messages on unchanged; others charge on their own scale.
    pub fn port_cost(&self, node: u32, speed: u32) -> u32 {
        let Some(d) = self.dev(node) else { return 0 };
        if !d.stp {
            return 0;
        }
        if d.stp_short {
            // 802.1D-1998 table.
            match speed {
                s if s >= 10_000 => 2,
                s if s >= 1_000 => 4,
                s if s >= 100 => 19,
                s if s >= 16 => 62,
                s if s >= 10 => 100,
                _ => 250,
            }
        } else {
            // 802.1D-2004: 20 Tbit/s divided by the port speed.
            (20_000_000 / speed.max(1)).max(1)
        }
    }

    /// How long ports stay silent after a cable or bridge changes. Rapid spanning tree (RSTP, today's default)
    /// agrees in about a second; classic 802.1D waits out its timers: up to 20 s for the old news to age out,
    /// then 15 s listening and 15 s learning. None when no bridge runs spanning tree.
    // shortcut: a bridge on the old short cost scale is taken to speak only classic spanning tree.
    pub fn stp_convergence(&self, stp: &Stp) -> Option<String> {
        let bridges: Vec<&crate::net::Device> = stp.cost.keys().filter_map(|id| self.dev(*id)).filter(|d| d.stp).collect();
        if bridges.is_empty() {
            return None;
        }
        Some(match bridges.iter().find(|d| d.stp_short) {
            Some(old) => format!("After a change: 30–50 s without traffic on the affected ports — {} only speaks classic spanning tree (20 s max age + 15 s listening + 15 s learning). With rapid spanning tree (RSTP) everywhere it takes about 1 s.", old.name),
            None => "After a change: about 1 s (rapid spanning tree, RSTP).".into(),
        })
    }

    pub fn stp(&self) -> Stp {
        if let Some(t) = self.memo.borrow().stp.as_ref() {
            return t.clone();
        }
        let t = self.stp_fresh();
        let mut m = self.memo.borrow_mut();
        if m.on {
            m.stp = Some(t.clone());
        }
        t
    }

    fn stp_fresh(&self) -> Stp {
        let edges: Vec<(u32, u32, u32, u32)> = self
            .links
            .iter()
            .filter(|l| self.bridge_ends(l.a, l.a_if) && self.bridge_ends(l.b, l.b_if) && self.link_ok(l) && self.link_up(l))
            // A bundle is one link to spanning tree (its first cable stands for all, at their summed speed).
            .filter(|l| self.bundle(l).iter().all(|o| o.id >= l.id))
            .map(|l| (l.id, l.a, l.b, self.bundle(l).iter().map(|o| self.port_speed(o)).sum()))
            .collect();
        let mut nodes: Vec<u32> = edges.iter().flat_map(|e| [e.1, e.2]).collect();
        nodes.sort();
        nodes.dedup();
        let mut out = Stp::default();
        let mut tree: HashSet<u32> = HashSet::new();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut root_size = 0;
        // Each connected group of bridges elects its own root.
        loop {
            let rest: Vec<u32> = nodes.iter().copied().filter(|n| !seen.contains(n)).collect();
            if rest.is_empty() {
                break;
            }
            // The group reachable from the first unseen bridge.
            let mut group = HashSet::from([rest[0]]);
            let mut stack = vec![rest[0]];
            while let Some(c) = stack.pop() {
                for e in edges.iter().filter(|e| e.1 == c || e.2 == c) {
                    let o = if e.1 == c { e.2 } else { e.1 };
                    if group.insert(o) {
                        stack.push(o);
                    }
                }
            }
            seen.extend(group.iter().copied());
            let stp_capable: Vec<u32> = group.iter().copied().filter(|n| self.dev(*n).is_some_and(|d| d.stp)).collect();
            let root = stp_capable.iter().copied().min_by(|a, b| self.bridge_id(*a).cmp(&self.bridge_id(*b))).unwrap_or_else(|| *group.iter().min().unwrap());
            // Report the root of the biggest group.
            if !stp_capable.is_empty() && group.len() > root_size {
                out.root = Some(root);
                root_size = group.len();
            }
            // Dijkstra from the root; ties go to the lower bridge id.
            let mut dist: HashMap<u32, (u32, (u16, String))> = HashMap::from([(root, (0, self.bridge_id(root)))]);
            let mut via: HashMap<u32, u32> = HashMap::new();
            let mut done: HashSet<u32> = HashSet::new();
            while let Some((&c, &(cd, _))) = dist.iter().filter(|(k, _)| !done.contains(*k)).min_by(|a, b| (a.1 .0, &a.1 .1).cmp(&(b.1 .0, &b.1 .1))) {
                done.insert(c);
                for e in edges.iter().filter(|e| e.1 == c || e.2 == c) {
                    let o = if e.1 == c { e.2 } else { e.1 };
                    let nd = cd + self.port_cost(o, e.3);
                    let better = match dist.get(&o) {
                        None => true,
                        Some(&(od, ref oid)) => !done.contains(&o) && (nd < od || (nd == od && self.bridge_id(c) < *oid)),
                    };
                    if better {
                        dist.insert(o, (nd, self.bridge_id(c)));
                        via.insert(o, e.0);
                    }
                }
            }
            tree.extend(via.values().copied());
            out.root_port.extend(via.iter().map(|(k, v)| (*k, *v)));
            for (n, (d, _)) in dist {
                out.cost.insert(n, d);
            }
            for e in edges.iter().filter(|e| group.contains(&e.1) && !tree.contains(&e.0)) {
                let can_block = [e.1, e.2].iter().any(|x| self.dev(*x).is_some_and(|d| d.stp));
                let all: Vec<u32> = self.links.iter().find(|l| l.id == e.0).map(|l| self.bundle(l).iter().map(|o| o.id).collect()).unwrap_or_default();
                if can_block {
                    out.blocked.extend(all);
                } else {
                    out.storm.extend(all);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::net::{template_seeded, Kind, Medium};

    /// Whether the speakers take over depends on the hardware (who has the lowest MAC). Over many
    /// random houses both outcomes appear, the takeover follows the physics, and the fix always works.
    #[test]
    fn speaker_takeover_emerges_from_the_hardware() {
        let (mut taken, mut fine) = (0, 0);
        for seed in 0..60u64 {
            let mut n = template_seeded("speakers", seed);
            let stp = n.stp();
            let sw = |n: &crate::net::Network, name: &str| n.devices.iter().find(|d| d.name == name).unwrap().id;
            let (main, office, pc) = (sw(&n, "Main switch"), sw(&n, "Office switch"), sw(&n, "Office PC"));
            let wired = n.links.iter().find(|l| [l.a, l.b] == [main, office] || [l.b, l.a] == [main, office]).unwrap().id;
            let root_is_speaker = n.dev(stp.root.unwrap()).unwrap().kind == Kind::Speaker;
            let hijacked = stp.blocked.contains(&wired);
            assert!(!hijacked || root_is_speaker, "seed {seed}: cable blocked without a speaker root");
            let out = n.exec(pc, "speedtest").lines.join("\n");
            assert_eq!(hijacked, out.contains("mesh"), "seed {seed}: {out}");
            if hijacked { taken += 1 } else { fine += 1 }
            n.dev_mut(main).unwrap().priority = 4096;
            assert!(!n.stp().blocked.contains(&wired), "seed {seed}: fix failed");
        }
        assert!(taken > 0 && fine > 0, "takeover {taken}, fine {fine}");
    }

    #[test]
    fn classic_spanning_tree_is_slow_to_settle() {
        let mut n = template_seeded("speakers", 1);
        let pc = n.devices.iter().find(|d| d.name == "Office PC").unwrap().id;
        assert!(n.exec(pc, "stp").lines.join(" ").contains("30–50 s"));
        let home = template_seeded("home", 1);
        assert!(home.stp_convergence(&home.stp()).is_none_or(|t| t.contains("about 1 s")));
    }

    #[test]
    fn bundled_cables_are_one_faster_link() {
        let mut n = template_seeded("blank", 1);
        let a = n.add_device(Kind::Switch, [0.0, 0.0]);
        let b = n.add_device(Kind::Switch, [100.0, 0.0]);
        let l1 = n.connect(a, b, Medium::Cat6).unwrap();
        let l2 = n.connect(a, b, Medium::Cat6).unwrap();
        assert_eq!(n.stp().blocked.len(), 1);
        let one = n.link_rate(n.links.iter().find(|l| l.id == l1).unwrap());
        assert!(n.check().iter().any(|i| i.text.contains("LACP")));
        for l in n.links.iter_mut() {
            l.lag = 1;
        }
        assert!(n.stp().blocked.is_empty() && n.stp().storm.is_empty());
        let both = n.link_rate(n.links.iter().find(|l| l.id == l2).unwrap());
        assert!((both - 2.0 * one).abs() < 1.0, "{one} {both}");
        // One connection still rides a single cable of the bundle.
        let single = n.flow_rate(n.links.iter().find(|l| l.id == l2).unwrap());
        assert!((single - one).abs() < 1.0, "{one} {single}");
        assert!(!n.check().iter().any(|i| i.text.contains("LACP")));
    }

    #[test]
    fn loop_without_spanning_tree_storms() {
        let mut n = template_seeded("blank", 1);
        let a = n.add_device(Kind::Switch, [0.0, 0.0]);
        let b = n.add_device(Kind::Switch, [100.0, 0.0]);
        n.connect(a, b, Medium::Cat6).unwrap();
        let l2 = n.add_device(Kind::Switch, [50.0, 80.0]);
        n.connect(b, l2, Medium::Cat6).unwrap();
        n.connect(l2, a, Medium::Cat6).unwrap();
        assert!(n.stp().storm.is_empty() && n.stp().blocked.len() == 1);
        for d in n.devices.iter_mut() {
            d.stp = false;
        }
        assert_eq!(n.stp().storm.len(), 1);
    }
}
