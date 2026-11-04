//! Checks for Wi-Fi, spanning tree, VLAN and port trouble, the one-click fixes, and the commands that show them.
use crate::net::{Issue, Kind, Level, Log, Medium, Network, Outcome, Security};
use crate::ports::PortKind;
use crate::wifi::{Band, CH24};
use std::collections::HashMap;
#[allow(unused_imports)]
use crate::extras::Use;

#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Set a bridge's spanning-tree priority.
    Priority(u32, u16),
    EnableStp(u32),
    Unplug(u32),
    AutoChannels,
    Wpa3(u32),
    FastRoamEverywhere,
    /// Join the best access point.
    Reconnect(u32),
    /// Give an extender the main network's name.
    SameSsid(u32, String),
    /// Make a switch supply PoE+ with enough budget.
    /// The switch, the total budget it needs (W) and the per-port class (index into extras::POE).
    GivePoe(u32, f32, u8),
    /// Turn on smart queueing on a home router.
    Sqm(u32),
    /// Move a cable's end (link, device) to the fastest free port.
    MovePort(u32, u32),
    /// Bundle two parallel cables between switches (LACP).
    Bundle(u32, u32),
    /// Move a smart-home hub to another Thread/Zigbee channel.
    ZigbeeChannel(u32, u8),
}

impl Fix {
    pub fn label(&self) -> &'static str {
        crate::i18n::tr(self.label_en())
    }
    fn label_en(&self) -> &'static str {
        match self {
            Fix::Priority(..) => "Make it the root",
            Fix::EnableStp(_) => "Turn on spanning tree",
            Fix::Unplug(_) => "Unplug the loop",
            Fix::AutoChannels => "Pick better channels",
            Fix::Wpa3(_) => "Switch to WPA3",
            Fix::FastRoamEverywhere => "Turn on fast roaming",
            Fix::Reconnect(_) => "Reconnect to the best",
            Fix::SameSsid(..) => "Use the same name",
            Fix::GivePoe(_, _, c) if *c >= 3 => "Make it a PoE++ switch",
            Fix::GivePoe(..) => "Make it a PoE+ switch",
            Fix::Sqm(_) => "Turn on smart queueing",
            Fix::MovePort(..) => "Use the faster port",
            Fix::Bundle(..) => "Bundle them (LACP)",
            Fix::ZigbeeChannel(..) => "Move to a free channel",
        }
    }
}

fn issue(level: Level, dev: Option<u32>, link: Option<u32>, text: String, fix: Option<Fix>) -> Issue {
    Issue { level, dev, link, text, fix }
}

impl Network {
    /// The busiest switch (or home router): the natural spanning-tree root.
    fn main_bridge(&self) -> Option<u32> {
        // The central switch connects the most other switches; ties go to the one with more cables.
        let score = |d: &&crate::net::Device| {
            let switches = self.links.iter().filter(|l| (l.a == d.id || l.b == d.id) && self.dev(l.other(d.id).0).is_some_and(|o| o.kind == Kind::Switch)).count();
            let all = self.links.iter().filter(|l| l.a == d.id || l.b == d.id).count();
            (switches, all)
        };
        self.devices
            .iter()
            .filter(|d| matches!(d.kind, Kind::Switch | Kind::HomeRouter) && d.stp)
            .max_by_key(score)
            .or_else(|| self.devices.iter().filter(|d| d.kind == Kind::Switch).max_by_key(score))
            .map(|d| d.id)
    }

    pub fn check_more(&self, v: &mut Vec<Issue>) {
        // --- Spanning tree ---
        let stp = self.stp();
        for id in &stp.storm {
            let l = self.links.iter().find(|l| l.id == *id).unwrap();
            let sw = [l.a, l.b].into_iter().find(|x| self.dev(*x).is_some_and(|d| d.kind == Kind::Switch));
            v.push(issue(
                Level::Error,
                None,
                Some(*id),
                crate::i18n::trf("Loop between {} and {} and nothing runs spanning tree: broadcasts circle forever (broadcast storm) and the whole network stops.", &[&(self.name(l.a)), &(self.name(l.b))]),
                Some(sw.map(Fix::EnableStp).unwrap_or(Fix::Unplug(*id))),
            ));
        }
        if let Some(root) = stp.root.and_then(|r| self.dev(r)) {
            let fix = self.main_bridge().filter(|m| *m != root.id).map(|m| Fix::Priority(m, 4096));
            let hijacked: Vec<u32> = stp
                .blocked
                .iter()
                .copied()
                .filter(|_| root.kind == Kind::Speaker)
                .filter(|id| self.links.iter().any(|l| l.id == *id && l.medium != Medium::Mesh && [l.a, l.b].iter().all(|x| self.dev(*x).is_some_and(|d| d.kind != Kind::Speaker))))
                .collect();
            let mesh_in_use = self.links.iter().any(|l| l.medium == Medium::Mesh && !stp.blocked.contains(&l.id));
            if root.kind == Kind::Speaker && !hijacked.is_empty() && mesh_in_use {
                let l = self.links.iter().find(|l| l.id == hijacked[0]).unwrap();
                v.push(issue(
                    Level::Error,
                    Some(root.id),
                    Some(l.id),
                    {
                        let mesh = self.links.iter().filter(|m| m.medium == Medium::Mesh && !stp.blocked.contains(&m.id)).map(|m| self.link_rate(m)).fold(f32::MAX, f32::min);
                        let (prio, mac) = self.bridge_id(root.id);
                        crate::i18n::trf("{} won the spanning-tree election (priority {}, lowest MAC {}). It counts its 100 Mbit ports and its mesh on the old short cost scale, so the path through the speakers looks cheaper than your cable: the cable {}–{} is on standby and traffic crosses the speakers' mesh (≈ {} Mbit/s right now). Make your main switch the root (priority 4096).", &[&root.name, &prio, &mac, &(self.name(l.a)), &(self.name(l.b)), &format!("{:.0}", mesh)])
                    },
                    fix,
                ));
            } else if root.kind == Kind::Speaker && self.devices.iter().any(|d| d.kind == Kind::Switch) {
                v.push(issue(Level::Warning, Some(root.id), None, crate::i18n::trf("{} is the spanning-tree root. It works today, but the root should be your main switch — give that priority 4096.", &[&root.name]), fix));
            } else if fix.is_some() && root.priority == 32768 && self.devices.iter().filter(|d| d.kind == Kind::Switch && d.stp).count() >= 3 {
                // With default priorities, the oldest switch (lowest MAC) runs the tree — often a closet switch.
                v.push(issue(Level::Tip, Some(root.id), None, crate::i18n::trf("{} became the spanning-tree root by its MAC address, not by design. Make the central switch the root so paths stay short.", &[&root.name]), fix));
            }
            for id in stp.blocked.iter().filter(|id| !hijacked.contains(id)) {
                let l = self.links.iter().find(|l| l.id == *id).unwrap();
                let switches = [l.a, l.b].iter().all(|x| self.dev(*x).is_some_and(|d| d.kind == Kind::Switch));
                let twin = self.links.iter().find(|o| o.id != l.id && o.medium.cable() && l.medium.cable() && !stp.blocked.contains(&o.id) && ((o.a, o.b) == (l.a, l.b) || (o.a, o.b) == (l.b, l.a)));
                match twin.filter(|_| switches) {
                    Some(o) => v.push(issue(Level::Tip, None, Some(*id), crate::i18n::trf("{} and {} have two cables between them and spanning tree keeps one on standby. Bundle them (LACP): both carry traffic and a broken cable costs no time.", &[&(self.name(l.a)), &(self.name(l.b))]), Some(Fix::Bundle(l.id, o.id)))),
                    None => v.push(issue(Level::Tip, None, Some(*id), crate::i18n::trf("Spanning tree keeps {}–{} on standby as a backup path (root: {}).", &[&(self.name(l.a)), &(self.name(l.b)), &root.name]), None)),
                }
            }
        }

        // --- Wi-Fi ---
        let mut hidden_pairs = std::collections::HashSet::new();
        for l in self.links.iter().filter(|l| l.medium == Medium::WiFi) {
            let Some((ap, cl)) = self.wifi_ends(l) else { continue };
            let Some(s) = self.link_signal(l) else { continue };
            let walls = if s.walls > 0 { crate::i18n::trf(" through {} wall(s)", &[&s.walls]) } else { String::new() };
            if !s.alive() {
                let text = if s.rssi >= crate::wifi::floor(s.band) {
                    crate::i18n::trf("{} hears {} ({} dBm) but its answer doesn't make it back ({} dBm): the router shouts, the device whispers. More router power won't help — move closer or add an access point.", &[&cl.name, &ap.name, &format!("{:.0}", s.rssi), &format!("{:.0}", s.up)])
                } else {
                    crate::i18n::trf("{} can't hear {} at all ({} dBm{}). Move it, or add an access point on this side.", &[&cl.name, &ap.name, &format!("{:.0}", s.rssi), &walls])
                };
                let other = self.roam_candidates(cl, Network::joined(cl, ap)).iter().any(|c| c.0 != ap.id);
                v.push(issue(Level::Error, Some(cl.id), Some(l.id), text, other.then_some(Fix::Reconnect(cl.id))));
                continue;
            }
            let hidden = self.hidden(l);
            if !hidden.is_empty() && hidden_pairs.insert((cl.id.min(hidden[0]), cl.id.max(hidden[0]))) {
                v.push(issue(Level::Tip, Some(cl.id), Some(l.id), crate::i18n::trf("{} and {} both talk to {} but can't hear each other (walls between them): they send at the same time and collide. Another access point closer to one of them, or fewer walls in between, avoids it.", &[&cl.name, &(self.name(hidden[0])), &ap.name]), None));
            }
            if s.band == Band::B24 && !ap.band_steer && cl.bands[1] && ap.bands[1] {
                v.push(issue(Level::Tip, Some(cl.id), Some(l.id), crate::i18n::trf("{} sits on 2.4 GHz because it's the loudest band. With band steering on {} it would move to 5 GHz and get more speed.", &[&cl.name, &ap.name]), None));
            }
            if cl.kind == Kind::Extender {
                if s.rssi < -70.0 {
                    v.push(issue(Level::Warning, Some(cl.id), Some(l.id), crate::i18n::trf("{} hears {} at only {} dBm{}. It can only pass on what it receives, then halves it: place it halfway, where the signal is still good — or run a cable to a real access point.", &[&cl.name, &ap.name, &format!("{:.0}", s.rssi), &walls]), None));
                }
                if cl.ssid != ap.ssid {
                    v.push(issue(Level::Tip, Some(cl.id), None, crate::i18n::trf("{} uses its own name '{}'. Devices won't roam between it and '{}', so they cling to whichever they joined until the signal dies.", &[&cl.name, &cl.ssid, &ap.ssid]), Some(Fix::SameSsid(cl.id, ap.ssid.clone()))));
                }
                continue;
            }
            if cl.kind.radio() {
                continue;
            }
            if s.rssi < -75.0 {
                v.push(issue(Level::Warning, Some(cl.id), Some(l.id), crate::i18n::trf("{} gets a {} signal from {} ({} dBm{}): ≈ {} Mbit/s and dropouts.", &[&cl.name, &(s.quality()), &ap.name, &format!("{:.0}", s.rssi), &walls, &format!("{:.0}", s.mbps)]), None));
            }
            let now = self.rssi_at(ap, cl.pos, cl.floor, Band::B5);
            if let Some((best, rssi)) = self.roam_candidates(cl, Network::joined(cl, ap)).into_iter().find(|c| c.0 != ap.id) {
                let best_d = self.dev(best).unwrap();
                if ap.kind == Kind::Extender && rssi >= now - 3.0 && best_d.kind != Kind::Extender {
                    v.push(issue(Level::Warning, Some(cl.id), Some(l.id), crate::i18n::trf("{} goes through {} although {} is just as strong. Through an extender every packet crosses the air twice: half the speed.", &[&cl.name, &ap.name, &best_d.name]), Some(Fix::Reconnect(cl.id))));
                } else if rssi > now + 10.0 {
                    v.push(issue(Level::Warning, Some(cl.id), Some(l.id), crate::i18n::trf("{} sticks to {} ({} dBm) while {} offers {} dBm — a 'sticky client'. 802.11k/v/r helps devices move on in time.", &[&cl.name, &ap.name, &format!("{:.0}", now), &best_d.name, &format!("{:.0}", rssi)]), Some(Fix::Reconnect(cl.id))));
                }
            }
            // A stronger access point of yours under another name: devices never switch by themselves.
            if let Some(o) = self.devices.iter().filter(|o| o.kind.radio() && o.id != ap.id && o.ssid != ap.ssid).max_by(|a, b| self.best_rssi(a, cl.pos, cl.floor).total_cmp(&self.best_rssi(b, cl.pos, cl.floor))) {
                if self.best_rssi(o, cl.pos, cl.floor) > now + 6.0 {
                    let fix = (ap.kind == Kind::Extender).then(|| Fix::SameSsid(ap.id, o.ssid.clone()));
                    v.push(issue(Level::Warning, Some(cl.id), Some(l.id), crate::i18n::trf("{} is on '{}' via {} although {} ('{}') is much stronger here. Different network names: devices never switch by themselves.", &[&cl.name, &ap.ssid, &ap.name, &o.name, &o.ssid]), fix));
                }
            }
            if cl.kind == Kind::Console {
                v.push(issue(Level::Tip, Some(cl.id), Some(l.id), crate::i18n::trf("{} plays on Wi-Fi: lag spikes come from retries and shared airtime. A cable gives stable latency.", &[&cl.name]), None));
            }
        }
        // Radios that actually send (a firewall box with Wi-Fi switched off doesn't count).
        let on = |d: &&crate::net::Device| d.bands.iter().any(|b| *b);
        // Point-to-point bridges (directional) don't serve roaming phones.
        let radios: Vec<_> = self.devices.iter().filter(|d| matches!(d.kind, Kind::HomeRouter | Kind::AccessPoint) && d.beam == 0.0).filter(on).collect();
        for r in self.devices.iter().filter(|d| d.kind.radio()).filter(on) {
            for b in Band::ALL.into_iter().filter(|b| r.bands[b.i()]) {
                let (ch, w) = (r.ch[b.i()], r.width[b.i()]);
                if r.tx[b.i()] > b.eirp_limit(ch) + 0.01 {
                    v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("{}: {} dBm on {} is above the EU limit of {} dBm. It also doesn't help phones answer back.", &[&r.name, &format!("{:.0}", r.tx[b.i()]), &(b.label()), &format!("{:.0}", b.eirp_limit(ch))]), None));
                }
                if w > r.wifi.max_width(b.i()) {
                    v.push(issue(Level::Warning, Some(r.id), None, crate::i18n::trf("{}: {} can't use {} MHz on {} (max {} MHz).", &[&r.name, &(r.wifi.label()), &w, &(b.label()), &(r.wifi.max_width(b.i()))]), None));
                }
                if b != Band::B24 && !b.channels(w).contains(&ch) {
                    v.push(issue(Level::Warning, Some(r.id), None, crate::i18n::trf("{}: channel {} doesn't fit a {} MHz block on {} — pick one from the list.", &[&r.name, &ch, &w, &(b.label())]), Some(Fix::AutoChannels)));
                }
                if crate::wifi::dfs(b, ch, w) && r.kind != Kind::Neighbor {
                    v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("{} uses radar (DFS) channel {}: it must listen for a minute before sending and may jump away if radar shows up.", &[&r.name, &ch]), None));
                }
                if b == Band::B24 && w >= 40 {
                    v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("{} uses 40 MHz on 2.4 GHz: that's two of the three free channels — neighbours suffer and so do you.", &[&r.name]), None));
                }
            }
            if r.kind == Kind::Extender && r.wifi == crate::net::WifiStd::Wifi5 && !r.backhaul {
                v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("{} has one radio: it receives and repeats on the same channel. A mesh node with its own backhaul radio — or a cable — avoids the halving.", &[&r.name]), None));
            }
        }
        for r in &radios {
            if r.bands[0] && !CH24.contains(&r.ch[0]) {
                v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("{} uses 2.4 GHz channel {}. Only 1, 6 and 11 (or 1, 5, 9 and 13 in Europe) don't overlap; in-between channels collide with two neighbours at once.", &[&r.name, &(r.ch[0])]), Some(Fix::AutoChannels)));
            }
            for band in Band::ALL.into_iter().filter(|b| r.bands[*b as usize]) {
                let (c, who) = self.contention(r, band);
                if c > 1.35 {
                    let (noise, nets): (Vec<u32>, Vec<u32>) = who.iter().partition(|w| self.dev(**w).is_some_and(|d| d.kind == Kind::Interference));
                    let names = |ids: &[u32]| ids.iter().map(|w| self.name(*w)).collect::<Vec<_>>().join(", ");
                    let mut text = crate::i18n::trf("{} only gets about {}% of the airtime on {} channel {}", &[&r.name, &format!("{:.0}", 100.0 / c), &(band.label()), &(self.channel(r, band))]);
                    if !nets.is_empty() {
                        text += &crate::i18n::trf(": it takes turns with {}", &[&(names(&nets))]);
                    }
                    if !noise.is_empty() {
                        text += &crate::i18n::trf("{} {} disturbs it without waiting its turn — move it away, or use 5 GHz nearby", &[&(if nets.is_empty() { ":" } else { ";" }), &(names(&noise))]);
                    }
                    v.push(issue(Level::Warning, Some(r.id), None, text + ".", Some(Fix::AutoChannels)));
                }
            }
        }
        if radios.len() > 1 && radios.iter().any(|r| !r.fast_roam) && self.devices.iter().any(|d| matches!(d.kind, Kind::Phone | Kind::Laptop)) {
            v.push(issue(Level::Tip, None, None, crate::i18n::tr("Several access points but fast roaming (802.11k/v/r) is off: phones hang on to a far access point and calls stutter when they finally switch.").into(), Some(Fix::FastRoamEverywhere)));
        }
        for d in self.devices.iter().filter(|d| d.kind.radio() && d.bands.iter().any(|b| *b) && matches!(d.security, Security::Open | Security::Wep)) {
            v.push(issue(Level::Tip, Some(d.id), None, crate::i18n::trf("One click moves {} to WPA3.", &[&d.name]), Some(Fix::Wpa3(d.id))));
        }

        // --- Power over Ethernet ---
        // Per switch: the total its devices need and the most one port must give (with cable loss and some headroom).
        let mut need: HashMap<u32, (f32, f32)> = HashMap::new();
        for d in self.devices.iter().filter(|d| d.poe_draw > 0.0) {
            if let Some(l) = self.links.iter().find(|l| (l.a == d.id || l.b == d.id) && !l.medium.wireless()) {
                let e = need.entry(l.other(d.id).0).or_default();
                e.0 += d.poe_draw * 1.2;
                e.1 = e.1.max(d.poe_draw * 1.2);
            }
        }
        for (dev, text, sw) in self.poe_problems() {
            // An over-budget switch is the device itself (poe_problems names no other supplier then).
            let sw = sw.or_else(|| self.dev(dev).filter(|d| d.poe_class > 0).map(|d| d.id));
            let fix = sw.map(|s| {
                let (total, port) = need.get(&s).copied().unwrap_or((30.0, 30.0));
                // The smallest standard whose port carries the hungriest device, never less than it has (PoE+ at least).
                let class = (2..crate::extras::POE.len()).find(|&c| crate::extras::POE[c].1 >= port).unwrap_or(crate::extras::POE.len() - 1) as u8;
                let class = class.max(self.dev(s).map_or(0, |d| d.poe_class));
                Fix::GivePoe(s, (total / 10.0).ceil() * 10.0 + 10.0, class)
            });
            v.push(issue(Level::Error, Some(dev), None, text, fix));
        }

        // --- Busy hour: everyone's apps at once ---
        let load = self.loads();
        for l in &self.links {
            let u = self.utilisation(&load, l.id);
            if u > 0.8 {
                let who = load.who.get(&l.id).map(|w| w.iter().collect::<std::collections::HashSet<_>>().len()).unwrap_or(0);
                let sensitive = load.who.get(&l.id).is_some_and(|w| w.iter().any(|d| self.dev(*d).is_some_and(|d| d.uses.iter().any(|u| u.sensitive()))));
                v.push(issue(
                    if u > 1.0 { Level::Error } else { Level::Warning },
                    None,
                    Some(l.id),
                    format!(
                        "{} {}",
                        crate::i18n::trf(
                            "{} {}–{} is {}% full ({} share ≈ {} Mbit/s).",
                            &[
                                &(if (18.0..23.5).contains(&self.hour) { crate::i18n::trf("At the busy hour ({})", &[&(crate::more::clock(self.hour))]) } else { crate::i18n::trf("At {}", &[&(crate::more::clock(self.hour))]) }),
                                &self.name(l.a),
                                &self.name(l.b),
                                &format!("{:.0}", u * 100.0),
                                &(if who == 1 { crate::i18n::tr("one device's apps").to_string() } else { crate::i18n::trf("{} devices' apps", &[&who]) }),
                                &format!("{:.0}", self.link_rate(l)),
                            ]
                        ),
                        crate::i18n::tr(match (sensitive, self.sqm_on(l)) {
                            (_, true) => "Smart queueing keeps the delay low; more speed needs a faster plan or fewer heavy apps.",
                            (true, false) => "Video calls and games will lag — full queues add delay. Smart queueing (SQM) on the router keeps them smooth.",
                            _ => "",
                        })
                    )
                    .trim_end()
                    .to_string(),
                    [l.a, l.b].iter().filter_map(|x| self.dev(*x)).find(|d| d.kind == Kind::HomeRouter && !d.sqm).filter(|_| [l.a, l.b].iter().any(|x| self.dev(*x).is_some_and(|d| d.kind == Kind::Internet))).map(|r| Fix::Sqm(r.id)),
                ));
            }
        }
        for ap in self.devices.iter().filter(|d| d.kind.radio()) {
            let air = self.airtime(&load, ap.id);
            if air > 0.8 {
                v.push(issue(Level::Warning, Some(ap.id), None, crate::i18n::trf("{}'s Wi-Fi needs {}% of the airtime at the busy hour. Slow, far-away devices use the most — move them closer, wire them, or add an access point.", &[&ap.name, &format!("{:.0}", air * 100.0)]), None));
            }
        }

        // --- Running on the mobile backup ---
        for l in self.links.iter().filter(|l| l.medium == Medium::Cellular) {
            if self.link_ok(l) && self.link_between(l.a, l.b).is_some_and(|m| m.id == l.id) {
                v.push(issue(Level::Tip, None, Some(l.id), crate::i18n::trf("The main internet line is down: {} runs on its mobile backup (≈ 300 Mbit/s down, 50 up, about 30 ms more delay). Calls and browsing keep working.", &[&(self.name(if self.dev(l.a).is_some_and(|d| d.kind == Kind::Internet) { l.b } else { l.a }))]), None));
            }
        }

        // --- Router behind router ---
        for r in self.devices.iter().filter(|d| d.kind == Kind::HomeRouter) {
            if let Some(l) = self.links.iter().find(|l| (l.a == r.id && l.a_if == 1) || (l.b == r.id && l.b_if == 1)) {
                let up = self.dev(l.other(r.id).0).unwrap();
                if up.kind != Kind::Internet {
                    v.push(issue(Level::Warning, Some(r.id), Some(l.id), crate::i18n::trf("Double NAT: {} gets its internet through {}'s network, so addresses are translated twice. Port forwarding, some games and calls need extra work — set {} to access-point mode (cable into its LAN port, DHCP off) unless you want a separate network.", &[&r.name, &up.name, &r.name]), None));
                }
            }
        }

        // --- Remote access and UPnP ---
        for r in self.devices.iter().filter(|d| d.kind == Kind::HomeRouter) {
            if let Some((ok, why)) = self.remote_access(r) {
                v.push(issue(if ok { Level::Tip } else { Level::Warning }, Some(r.id), None, crate::i18n::trf("Remote access (VPN) to {}. {}", &[&r.name, &why]), None));
            }
            if r.upnp {
                v.push(issue(Level::Tip, Some(r.id), None, crate::i18n::trf("UPnP is on at {}: games and consoles open ports by themselves. Handy, but so can any infected device inside. Turn it off and forward only the ports you need.", &[&r.name]), None));
            }
        }

        // --- Thread and Zigbee ---
        for d in self.devices.iter().filter(|d| d.on_802154() && !d.off) {
            if self.smart_hub(d).is_none() {
                v.push(issue(Level::Warning, Some(d.id), None, crate::i18n::trf("{} uses {}, but no smart-home hub is in reach: nothing can switch it. Make the home router or a speaker a hub (Thread border router / Zigbee), or put a hub closer.", &[&d.name, &(d.smart.label())]), None));
            }
        }
        for h in self.devices.iter().filter(|h| h.hub && !h.off && self.devices.iter().any(|d| self.smart_hub(d).is_some_and(|x| x.0 == h.id))) {
            let clash = self.zb_clash(h, h.zb_ch);
            if let Some(&(_, wch)) = clash.first() {
                let who: Vec<String> = clash.iter().map(|c| self.name(c.0)).collect();
                v.push(issue(
                    Level::Tip,
                    Some(h.id),
                    None,
                    crate::i18n::trf("{}'s Thread/Zigbee channel {} sits inside busy 2.4 GHz Wi-Fi channel {} ({}): they share the air, so gadgets react late or drop off. Channels 15, 20 and 25 sit between Wi-Fi 1, 6 and 11.", &[&h.name, &h.zb_ch, &wch, &(who.join(", "))]),
                    self.free_zb_channel(h).map(|c| Fix::ZigbeeChannel(h.id, c)),
                ));
            }
        }

        // --- Crowded access points ---
        for ap in self.devices.iter().filter(|d| d.kind.radio()) {
            let n = self.links.iter().filter(|l| l.medium == Medium::WiFi && self.wifi_ends(l).is_some_and(|(a, c)| a.id == ap.id && !c.kind.radio())).count();
            if n > 30 {
                v.push(issue(Level::Warning, Some(ap.id), None, crate::i18n::trf("{} serves {} devices. Beyond about 30 active devices an access point spends more time managing turns than sending — add access points and lower their power.", &[&ap.name, &n]), None));
            }
        }

        // --- Routers that face the internet without NAT ---
        for r in self.devices.iter().filter(|d| d.kind == Kind::Router) {
            let to_inet = self.links.iter().any(|l| (l.a == r.id || l.b == r.id) && self.dev(l.other(r.id).0).is_some_and(|o| o.kind == Kind::Internet));
            if to_inet && r.ifaces.iter().any(|f| f.addr().is_some_and(|(ip, _)| ip.is_private())) {
                v.push(issue(Level::Error, Some(r.id), None, crate::i18n::trf("{} connects private addresses straight to the internet, but a plain router doesn't translate them (NAT). Nothing inside can reach the internet: put a firewall / home router in front.", &[&r.name]), None));
            }
        }

        // --- Not enough sockets ---
        for d in self.devices.iter().filter(|d| !d.kind.is_host() && d.kind != Kind::Internet && d.port_count() > 0) {
            let used = self.links.iter().filter(|l| l.medium.takes_port() && (l.a == d.id || l.b == d.id)).count();
            if used > d.port_count() {
                v.push(issue(Level::Error, Some(d.id), None, crate::i18n::trf("{} has {} ports but {} cables plugged in. Add a switch or pick a model with more ports.", &[&d.name, &(d.port_count()), &used]), None));
            }
        }

        // --- Cables in the wrong kind of socket, or a slow port while a faster one is free ---
        for l in self.links.iter().filter(|l| l.medium.takes_port()) {
            for id in [l.a, l.b] {
                let Some(d) = self.dev(id).filter(|d| d.kind != Kind::Internet) else { continue };
                let Some((p, g)) = l.port_at(id).and_then(|p| Some((p, d.port(p)?))) else { continue };
                let other = self.name(l.other(id).0);
                // A fibre internet line into an RJ45 WAN port is normal: the provider's fibre box converts it.
                if l.medium == Medium::Fiber && g.kind == PortKind::Rj45 && !self.internet_end(l, id) {
                    v.push(issue(Level::Warning, Some(id), Some(l.id), crate::i18n::trf("The fibre from {} ends in {}'s copper {}: that needs a media converter. Pick a model with an SFP port.", &[&other, &d.name, &(self.port_name(id, p))]), None));
                } else if l.medium != Medium::Fiber && g.kind == PortKind::Fiber {
                    v.push(issue(Level::Tip, Some(id), Some(l.id), crate::i18n::trf("The cable from {} goes into {}'s SFP {}: that needs an RJ45 SFP module.", &[&other, &d.name, &(self.port_name(id, p))]), None));
                }
                if let Some(q) = self.faster_free_port(l, id) {
                    v.push(issue(Level::Tip, Some(id), Some(l.id), crate::i18n::trf("{} is in {}'s {} while {} is free.", &[&other, &d.name, &(self.port_name(id, p)), &(self.port_name(id, q))]), Some(Fix::MovePort(l.id, id))));
                }
            }
        }

        // --- Links that quietly slow things down ---
        for l in &self.links {
            match l.medium {
                Medium::Cat5 => v.push(issue(Level::Tip, None, Some(l.id), crate::i18n::trf("{}–{} is old Cat 5 cable: fine for 1 Gbit/s, nothing faster{}.", &[&(self.name(l.a)), &(self.name(l.b)), &(if self.negotiated(l) <= 100 { " — and here an old port runs it at 100 Mbit/s: everything behind it is capped there" } else { "" })]), None)),
                Medium::Powerline => v.push(issue(Level::Tip, None, Some(l.id), crate::i18n::trf("Powerline {}–{}: speed depends on the house wiring and drops when appliances run.", &[&(self.name(l.a)), &(self.name(l.b))]), None)),
                _ => {}
            }
        }

        // --- VLANs ---
        let segs = self.segments();
        for d in &self.devices {
            if d.kind.is_host() {
                if let Some(f) = d.ifaces.first() {
                    if !(1..=4094).contains(&f.vlan) {
                        v.push(issue(Level::Error, Some(d.id), None, crate::i18n::trf("{}: VLAN {} doesn't exist — VLAN numbers go from 1 to 4094.", &[&d.name, &f.vlan]), None));
                    } else if f.vlan != 1 {
                        let seg = segs.get(&(d.id, 0));
                        let has_router = self.devices.iter().any(|r| r.kind.routes() && r.ifaces.iter().enumerate().any(|(i, _)| segs.get(&(r.id, i)) == seg));
                        if !has_router {
                            v.push(issue(Level::Error, Some(d.id), None, crate::i18n::trf("{} is in VLAN {} but no router has an interface there: it gets no address and can't leave its VLAN.", &[&d.name, &f.vlan]), None));
                        }
                    }
                }
            }
            if d.kind == Kind::Router {
                let mut by_vlan: HashMap<(usize, u16), usize> = HashMap::new();
                let mut nets: HashMap<std::net::Ipv4Addr, u16> = HashMap::new();
                for f in &d.ifaces {
                    let port = f.parent.unwrap_or(usize::MAX);
                    *by_vlan.entry((port, f.vlan)).or_default() += 1;
                    if let Some((ip, p)) = f.addr() {
                        if let Some(o) = nets.insert(crate::net::net_of(ip, p), f.vlan) {
                            if o != f.vlan {
                                v.push(issue(Level::Error, Some(d.id), None, crate::i18n::trf("{}: VLAN {} and VLAN {} use the same network {}/{}.", &[&d.name, &o, &f.vlan, &(crate::net::net_of(ip, p)), &p]), None));
                            }
                        }
                    }
                }
                if by_vlan.iter().any(|(k, n)| k.0 != usize::MAX && *n > 1) {
                    v.push(issue(Level::Error, Some(d.id), None, crate::i18n::trf("{} has the same VLAN twice on one port.", &[&d.name]), None));
                }
                let subs = d.ifaces.iter().filter(|f| f.parent.is_some()).count();
                if subs > 20 {
                    v.push(issue(Level::Tip, Some(d.id), None, crate::i18n::trf("{} routes {} VLANs over one trunk cable (router-on-a-stick): all traffic between VLANs shares that cable. At this size a layer-3 core switch is the usual choice.", &[&d.name, &subs]), None));
                }
            }
        }
    }

    /// Applies a one-click fix and says what changed.
    pub fn apply_fix(&mut self, f: &Fix) -> String {
        match f {
            Fix::Priority(id, p) => {
                let d = self.dev_mut(*id).unwrap();
                d.stp = true;
                d.priority = *p;
                crate::i18n::trf("{} now has spanning-tree priority {} and becomes the root.", &[&d.name, &p])
            }
            Fix::EnableStp(id) => {
                let d = self.dev_mut(*id).unwrap();
                d.stp = true;
                crate::i18n::trf("Spanning tree is on for {}; it blocks the loop.", &[&d.name])
            }
            Fix::Unplug(l) => {
                self.remove_link(*l);
                crate::i18n::tr("Removed the looping cable.").into()
            }
            Fix::AutoChannels => self.auto_channels(),
            Fix::Wpa3(id) => {
                let d = self.dev_mut(*id).unwrap();
                d.security = Security::Wpa3;
                crate::i18n::trf("{} now uses WPA3.", &[&d.name])
            }
            Fix::FastRoamEverywhere => {
                for d in self.devices.iter_mut().filter(|d| d.kind.radio()) {
                    d.fast_roam = true;
                }
                crate::i18n::tr("802.11k/v/r is on for all your access points.").into()
            }
            Fix::Reconnect(id) => self.reconnect(*id),
            Fix::MovePort(link, id) => {
                let Some(l) = self.links.iter().find(|l| l.id == *link).cloned() else { return crate::i18n::tr("That cable is gone.").into() };
                let Some(q) = self.best_free_port(&l, *id) else { return crate::i18n::tr("No faster port is free.").into() };
                self.move_port(*link, *id, q);
                crate::i18n::trf("{} now goes into {}'s {}.", &[&(self.name(l.other(*id).0)), &(self.name(*id)), &(self.port_name(*id, q))])
            }
            Fix::Bundle(x, y) => {
                let lag = self.links.iter().map(|l| l.lag).max().unwrap_or(0).saturating_add(1);
                for l in self.links.iter_mut().filter(|l| l.id == *x || l.id == *y) {
                    l.lag = lag;
                }
                crate::i18n::tr("The two cables now work as one bundle (LACP).").into()
            }
            Fix::ZigbeeChannel(id, ch) => {
                let d = self.dev_mut(*id).unwrap();
                d.zb_ch = *ch;
                crate::i18n::trf("{}'s Thread/Zigbee network now uses channel {}.", &[&d.name, &ch])
            }
            Fix::Sqm(id) => {
                let d = self.dev_mut(*id).unwrap();
                d.sqm = true;
                crate::i18n::trf("{} now runs smart queueing: the line stays responsive when it's full.", &[&d.name])
            }
            Fix::GivePoe(id, w, class) => {
                let d = self.dev_mut(*id).unwrap();
                // What a switch with each PoE standard costs on top of a plain one: the upgrade is paid once.
                const EXTRA: [f32; 5] = [0.0, 40.0, 60.0, 120.0, 160.0];
                let class = (*class).min(4).max(d.poe_class);
                d.price += EXTRA[class as usize] - EXTRA[d.poe_class.min(4) as usize];
                d.poe_class = class;
                d.poe_budget = d.poe_budget.max(*w);
                crate::i18n::trf("{} now supplies {} ({} W in total).", &[&d.name, &(crate::extras::POE[class as usize].0), &format!("{:.0}", d.poe_budget)])
            }
            Fix::SameSsid(id, ssid) => {
                let d = self.dev_mut(*id).unwrap();
                d.ssid = ssid.clone();
                crate::i18n::trf("{} now broadcasts '{}'.", &[&d.name, &ssid])
            }
        }
    }

    /// Moves a wireless client to the strongest access point with its network name.
    pub fn reconnect(&mut self, id: u32) -> String {
        let Some(link) = self.wifi_link_of(id).cloned() else { return crate::i18n::tr("No Wi-Fi connection to change.").into() };
        let Some((ap, cl)) = self.wifi_ends(&link).map(|(a, c)| (a.clone(), c.clone())) else { return String::new() };
        // Prefer a real access point over an extender when they're about as strong.
        let mut c = self.roam_candidates(&cl, Network::joined(&cl, &ap));
        c.iter_mut().for_each(|x| {
            if self.dev(x.0).map(|d| d.kind) == Some(Kind::Extender) {
                x.1 -= 6.0;
            }
        });
        // Staying put isn't a reconnect.
        c.retain(|x| x.0 != ap.id);
        c.sort_by(|a, b| b.1.total_cmp(&a.1));
        let Some(&(best, _)) = c.first() else { return crate::i18n::tr("No other access point in reach.").into() };
        let l = self.links.iter_mut().find(|x| x.id == link.id).unwrap();
        if l.a == ap.id {
            (l.a, l.a_if) = (best, 0);
        } else {
            (l.b, l.b_if) = (best, 0);
        }
        crate::i18n::trf("{} now uses {}.", &[&cl.name, &(self.name(best))])
    }

    /// Extra commands: speedtest, wifi, stp, vlan. Returns false when the command isn't one of them.
    pub fn exec_more(&mut self, id: u32, cmd: &str, arg: &str, o: &mut Outcome) -> bool {
        let d = self.dev(id).unwrap().clone();
        match cmd {
            "speedtest" | "iperf" => {
                let target = if arg.is_empty() { "example.com" } else { arg };
                let Some(dst) = self.resolve(id, target, o) else { return true };
                let Some(src) = d.first_ip() else {
                    o.lines.push("No IP address yet.".into());
                    return true;
                };
                let t = self.send(id, src, dst, "TCP", 443, None);
                o.log.extend(t.log.clone());
                if t.ok && t.path.len() < 2 {
                    o.lines.push(format!("{target} is this device itself — test against another device or a website."));
                    return true;
                }
                if !t.ok {
                    o.lines.push(format!("Speed test failed: {}", t.error));
                    o.traces.push(t);
                    return true;
                }
                let (raw, mut why) = self.path_rate(&t.path);
                // Other people's apps take their share of every link on the way.
                let load = self.loads();
                let free = t
                    .path
                    .windows(2)
                    .filter_map(|w| self.link_between(w[0], w[1]))
                    .map(|l| self.flow_rate(l) * (1.0 - self.utilisation_for(&load, l.id, d.qos)).max(0.05))
                    .fold(f32::MAX, f32::min);
                let mut mbps = raw.min(free);
                // One TCP connection can't beat its round trip and loss: Mathis et al. (MSS/RTT · 1.22/√p),
                // and a receive window of ~8 MB per round trip.
                let rtt = (t.ms * 2.0 + self.queue_ms(&load, &t.path)).max(0.5) / 1000.0;
                let (p, _, _) = self.path_air(&t.path);
                let mathis = 1460.0 * 8.0 / rtt * 1.22 / p.max(1e-6).sqrt() / 1e6;
                let window = 8.0 * 1024.0 * 1024.0 * 8.0 / rtt / 1e6;
                let single = mathis.min(window);
                // Speed tests open ~8 connections at once; a single download only gets `single`.
                if single * 8.0 < mbps {
                    mbps = single * 8.0;
                    why = format!("TCP itself: {:.0} ms round trip{} limits every connection", rtt * 1000.0, if p > 1e-4 { format!(" and {:.2}% loss", p * 100.0) } else { String::new() });
                }
                if free < raw * 0.95 {
                    why += " — and other devices' apps are using part of the line right now";
                }
                let qos = match d.qos {
                    crate::net::Qos::High => Some("high priority: a full line serves it before everyone else"),
                    crate::net::Qos::Low => Some("low priority: a full line serves everyone else first"),
                    crate::net::Qos::Normal => None,
                };
                // TCP never quite fills the pipe; live runs vary a little from test to test.
                let mbps = mbps * if self.live { 0.86 + 0.1 * crate::sim::unit(self.seed, (self.clock * 1000.0) as u64, 7) } else { 0.93 };
                o.log.push(Log { src: src.to_string(), dst: dst.to_string(), proto: "TCP", layer: "Transport", info: format!("Bulk transfer: ≈ {mbps:.0} Mbit/s"), dev: id, ..Default::default() });
                o.lines.push(format!("Speed test to {target}"));
                o.lines.push(format!("  Download ≈ {mbps:.0} Mbit/s   latency {:.1} ms", t.ms * 2.0));
                o.lines.push(format!("  Bottleneck: {why}"));
                if let Some(q) = qos {
                    o.lines.push(format!("  QoS: {q}"));
                }
                if single < mbps {
                    o.lines.push(format!("  One single download gets ≈ {single:.0} Mbit/s ({:.0} ms round trip{})", rtt * 1000.0, if p > 1e-4 { format!(", {:.2}% loss", p * 100.0) } else { String::new() }));
                }
                o.lines.push(format!("  Path: {}", t.path.iter().map(|p| self.name(*p)).collect::<Vec<_>>().join(" › ")));
                o.traces.push(t);
            }
            "wifi" | "iwconfig" | "scan" | "netsh" => {
                if let Some(l) = self.wifi_link_of(id) {
                    let (ap, _) = self.wifi_ends(l).unwrap();
                    let Some(s) = self.link_signal(l) else {
                        o.lines.push(format!("Joined to {}, but the two have no Wi-Fi band in common — no connection.", ap.name));
                        return true;
                    };
                    o.lines.push(format!("Connected to '{}' via {} on {} ch {}", ap.ssid, ap.name, s.band.label(), self.channel(ap, s.band)));
                    o.lines.push(format!("  Signal {:.0} dBm ({}), Wi-Fi link ≈ {:.0} Mbit/s, {} wall(s), airtime ÷{:.1}", s.rssi, s.quality(), s.mbps, s.walls, s.contention));
                    if let Some((e, why)) = self.internet_rate(id, &self.stp()).filter(|e| e.0 < s.mbps * 0.95) {
                        o.lines.push(format!("  To the internet ≈ {e:.0} Mbit/s — {why}"));
                    }
                    if self.radar(ap) {
                        o.lines.push(format!("  Radar on 5 GHz channel {}: {} must leave it for 30 minutes and sends on channel {} meanwhile.", ap.ch[1], ap.name, self.channel(ap, Band::B5)));
                    }
                    let hidden = self.hidden(l);
                    if !hidden.is_empty() {
                        o.lines.push(format!("  Can't hear {} on the same access point: their packets collide at {} (more retries).", hidden.iter().map(|h| self.name(*h)).collect::<Vec<_>>().join(", "), ap.name));
                    }
                } else {
                    o.lines.push(format!("{} isn't on Wi-Fi.", d.name));
                }
                o.lines.push("Networks in the air:".into());
                let mut seen: Vec<(f32, String)> = self
                    .devices
                    .iter()
                    .filter(|t| t.kind.transmits() && t.id != id)
                    .map(|t| {
                        let r = self.best_rssi(t, d.pos, d.floor);
                        (r, format!("  {:>5.0} dBm  {:<20} ch {:>2}/{:<3} {}", r, if t.ssid.is_empty() || t.kind == Kind::Speaker { &t.name } else { &t.ssid }, self.channel(t, Band::B24), self.channel(t, Band::B5), match t.kind { Kind::Neighbor => "(neighbour)", Kind::Interference => "(not Wi-Fi)", Kind::Speaker => "(speaker mesh)", _ => "" }))
                    })
                    .filter(|x| x.0 > -90.0)
                    .collect();
                seen.sort_by(|a, b| b.0.total_cmp(&a.0));
                o.lines.extend(seen.into_iter().map(|x| x.1));
            }
            "stp" | "spanning-tree" => {
                let stp = self.stp();
                match stp.root {
                    Some(r) => {
                        let (p, mac) = self.bridge_id(r);
                        o.lines.push(format!("Root bridge: {} (priority {p}, MAC {mac})", self.name(r)));
                    }
                    None => o.lines.push("No device runs spanning tree here.".into()),
                }
                if let Some(c) = stp.cost.get(&id) {
                    o.lines.push(format!("{}: root path cost {c}", d.name));
                }
                for b in &stp.blocked {
                    let l = self.links.iter().find(|l| l.id == *b).unwrap();
                    o.lines.push(format!("  standby: {} – {} ({})", self.name(l.a), self.name(l.b), l.medium.label()));
                }
                for b in &stp.storm {
                    let l = self.links.iter().find(|l| l.id == *b).unwrap();
                    o.lines.push(format!("  LOOP (storm): {} – {}", self.name(l.a), self.name(l.b)));
                }
            }
            "vlan" | "vlans" => {
                for f in &d.ifaces {
                    o.lines.push(format!("{:<10} VLAN {:<5} {}", f.name, f.vlan, if f.ip.is_empty() { "—".into() } else { format!("{}/{}", f.ip, f.prefix) }));
                }
            }
            _ => return false,
        }
        true
    }
}
