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

/// A capacity traffic shares: one direction of a cable (`TO_A`/`TO_B`), both directions of a half-duplex link
/// (`BOTH`: powerline, speaker mesh), or the airtime of one radio channel (`AIR` + band): an access point and every
/// device on it take turns, and a single-radio repeater shares its upstream's channel.
pub type Res = (u32, u8);
const TO_A: u8 = 0;
const TO_B: u8 = 1;
const BOTH: u8 = 2;
const AIR: u8 = 3;

/// Everyone's traffic on every link, after sharing.
#[derive(Default, Debug)]
pub struct Load {
    /// Per link: what gets through (towards its end `a`, towards its end `b`), Mbit/s.
    pub per_link: HashMap<u32, (f32, f32)>,
    /// Who causes load on each link.
    pub who: HashMap<u32, Vec<u32>>,
    /// Per device and demand (index into `demands`): Mbit/s a transfer that grabs what it can got towards the device.
    pub got: HashMap<(u32, usize), f32>,
    /// What each link draws on: (capacity, its size, cost of one Mbit/s towards a, towards b).
    res: HashMap<u32, Vec<(Res, f32, f32, f32)>>,
    cap: HashMap<Res, f32>,
    /// Used per capacity and priority, and how many transfers share it.
    used: HashMap<(Res, Qos), f32>,
    count: HashMap<(Res, Qos), u32>,
    /// What apps ask of each capacity (a transfer that grabs what it can counts what it got).
    wanted: HashMap<Res, f32>,
}

impl Load {
    /// The fullest of the capacities a link draws on, by `of`.
    fn fullest(&self, link: u32, of: impl Fn(Res) -> f32) -> f32 {
        self.res.get(&link).map_or(0.0, |v| v.iter().map(|x| of(x.0) / x.1.max(1e-3)).fold(0.0, f32::max))
    }
    fn used_upto(&self, r: Res, q: Qos) -> f32 {
        Qos::ALL.iter().filter(|c| **c <= q).filter_map(|c| self.used.get(&(r, *c))).sum()
    }
}

/// The traffic model's memory, not saved: each radio channel's airtime from the last sharing (so Wi-Fi contention
/// follows what was really sent) and what each app got while busy (so a burst on a slow way lasts longer, as a
/// page or a video buffer really takes longer to fill).
#[derive(Clone, Default, Debug)]
pub struct TrafficMemory {
    pub air: HashMap<(u32, usize), f32>,
    pub got: HashMap<(u32, usize), f32>,
}

/// What one direction of a transfer asks for: a steady rate, or as much as it can get (TCP filling the way), at most so much.
#[derive(Clone, Copy, Debug)]
enum Want {
    Fixed(f32),
    Grab(f32),
}

/// One direction of one app's transfer while sharing.
struct Ask {
    dev: u32,
    idx: usize,
    to_dev: bool,
    q: Qos,
    cap: f32,
    grab: bool,
    res: Vec<(Res, f32)>,
    links: Vec<(u32, bool)>,
}

impl Network {
    /// Where a device's app traffic goes: the internet, or a NAS/server in the house for backups and cameras
    /// (the NAS's own backup goes to the cloud).
    fn target(&self, from: u32, u: Use) -> Option<Ipv4Addr> {
        if u.local() {
            if let Some(ip) = self.devices.iter().find(|d| d.id != from && matches!(d.kind, Kind::Nas | Kind::Server)).and_then(|d| d.first_ip()) {
                return Some(ip);
            }
        }
        self.devices.iter().any(|d| d.kind == Kind::Internet).then(|| crate::net::site_ip(0))
    }

    /// Where a device's own custom load goes: the device picked for it, else the internet.
    fn own_target(&self, d: &Device) -> Option<Ipv4Addr> {
        match self.dev(d.test_to).filter(|t| t.id != d.id).and_then(|t| t.first_ip()) {
            Some(ip) => Some(ip),
            None => self.target(d.id, Use::Browsing),
        }
    }

    /// The apps a device runs in the current traffic mode: its everyday ones, or the custom test load.
    pub fn apps<'a>(&self, d: &'a Device) -> &'a [Use] {
        if self.custom_traffic { &d.test_uses } else { &d.uses }
    }

    /// What a device asks of the network now: per app (None = its own custom load), how active it is and the
    /// average Mbit/s (down, up) it wants.
    pub fn demands(&self, d: &Device) -> Vec<(Option<Use>, f32, f32, f32)> {
        let mut v: Vec<_> = self.apps(d).iter().map(|u| (Some(*u), self.activity(*u), u.demand().0 * self.activity(*u), u.demand().1 * self.activity(*u))).collect();
        if self.custom_traffic && d.test_mbps.iter().any(|x| *x > 0.0) {
            v.push((None, 1.0, d.test_mbps[0], d.test_mbps[1]));
        }
        v
    }

    /// How active an app is now: a custom test load runs at full rate; everyday apps follow their curve over
    /// the day, times how busy the household is.
    pub fn activity(&self, u: Use) -> f32 {
        if self.custom_traffic { 1.0 } else { crate::more::activity(u, self.hour) * self.traffic }
    }

    /// The day at a glance, every half hour: how much the apps ask of the internet line and of the busiest cable
    /// or Wi-Fi (1.0 = all it carries; above that they get less than they want).
    pub fn day_load(&self) -> Vec<(f32, f32)> {
        let mut n = self.clone();
        n.live = false;
        let line: Vec<u32> = n.links.iter().filter(|l| [l.a, l.b].iter().any(|x| n.dev(*x).is_some_and(|d| d.kind == Kind::Internet))).map(|l| l.id).collect();
        (0..48)
            .map(|i| {
                n.hour = i as f32 * 0.5;
                let ld = n.loads();
                let links = n.links.iter().map(|l| n.wanted(&ld, l.id));
                let air = n.devices.iter().filter(|d| d.kind.radio()).map(|d| n.airtime(&ld, d.id));
                (line.iter().map(|l| n.wanted(&ld, *l)).fold(0.0, f32::max), links.chain(air).fold(0.0, f32::max))
            })
            .collect()
    }

    /// Everyone's apps shared out over the network as long-run averages: what plans, warnings and the chart use.
    pub fn loads(&self) -> Load {
        self.settle(false)
    }

    /// The same right now, in Simulate with live variation: pages load on clicks, videos fetch in chunks, calls
    /// jitter. A custom test load stays steady unless `custom_bursty`.
    pub fn loads_now(&self) -> Load {
        self.settle(self.live && (!self.custom_traffic || self.custom_bursty))
    }

    /// Shares twice: the second round's Wi-Fi contention follows the airtime the first round really used.
    fn settle(&self, live: bool) -> Load {
        self.memory.borrow_mut().air.clear();
        let first = self.share(live);
        let air: HashMap<(u32, usize), f32> = first.cap.keys().filter(|r| r.1 >= AIR).map(|r| ((r.0, (r.1 - AIR) as usize), first.used_upto(*r, Qos::Low))).collect();
        self.memory.borrow_mut().air = air;
        let out = self.share(live);
        if live {
            self.memory.borrow_mut().got.extend(out.got.iter().map(|(k, v)| (*k, *v)));
        }
        out
    }

    /// What a link draws on: (capacity, its size, cost of one Mbit/s towards a, towards b).
    fn link_res(&self, l: &crate::net::Link) -> Vec<(Res, f32, f32, f32)> {
        if l.medium == Medium::WiFi {
            if let (Some((ap, cl)), Some(s)) = (self.wifi_ends(l), self.link_signal(l)) {
                // Airtime: a Mbit/s sent costs 1/rate of the channel's time, at the rate of the direction it goes.
                let (to_cl, to_ap) = (1.0 / s.mbps.max(0.01), 1.0 / s.up_mbps.max(0.01));
                let r = (self.chan_src(ap).id, AIR + s.band.i() as u8);
                return vec![if l.a == cl.id { (r, 1.0, to_cl, to_ap) } else { (r, 1.0, to_ap, to_cl) }];
            }
        }
        let rate = self.link_rate(l);
        if matches!(l.medium, Medium::Mesh | Medium::Powerline | Medium::WiFi) {
            return vec![((l.id, BOTH), rate, 1.0, 1.0)];
        }
        // Full duplex; towards the internet only the plan's upload speed (a mobile line: about a sixth of its download).
        let up = |end: u32| {
            self.dev(end).filter(|d| d.kind == Kind::Internet).map(|i| match l.medium {
                Medium::Cellular => rate / 6.0,
                _ => ((if i.plan_up_mbps > 0.0 { i.plan_up_mbps } else { i.plan_mbps }) * if self.sqm_on(l) { 0.92 } else { 1.0 }).min(rate),
            })
        };
        vec![((l.id, TO_A), up(l.a).unwrap_or(rate), 1.0, 0.0), ((l.id, TO_B), up(l.b).unwrap_or(rate), 0.0, 1.0)]
    }

    /// Shares the capacities out: every transfer gets the same Mbit/s until a capacity on its way is full or it has
    /// what it wants (max-min fairness, which TCP roughly reaches), high priority first, then normal, then low.
    /// shortcut: TCP really favours short round trips a little; that's left out.
    fn share(&self, live: bool) -> Load {
        let mut out = Load::default();
        let mut asks: Vec<Ask> = vec![];
        let got = self.memory.borrow().got.clone();
        for d in self.devices.iter().filter(|d| !d.off) {
            let Some(src) = d.first_ip() else { continue };
            for (idx, (u, act, down, up)) in self.demands(d).into_iter().enumerate() {
                let dst = match u {
                    Some(u) => self.target(d.id, u),
                    None => self.own_target(d),
                };
                let Some(dst) = dst.filter(|_| act >= 0.01 && down + up >= 0.001) else { continue };
                let t = self.send(d.id, src, dst, "TCP", 443, None);
                if !t.ok {
                    continue;
                }
                let (mut back, mut along, mut links) = (vec![], vec![], vec![]);
                // One connection rides one cable of a bundle (LACP).
                let mut one = f32::INFINITY;
                for w in t.path.windows(2) {
                    let Some(l) = self.link_between(w[0], w[1]) else { continue };
                    let rs = out.res.entry(l.id).or_insert_with(|| self.link_res(l)).clone();
                    // The path runs from the device outwards: uploads go along it, downloads come back.
                    let towards_a = l.a == w[0];
                    for (r, c, wa, wb) in rs {
                        out.cap.insert(r, c);
                        let (b, a) = if towards_a { (wa, wb) } else { (wb, wa) };
                        for (v, x) in [(&mut back, b), (&mut along, a)] {
                            match v.iter_mut().find(|e: &&mut (Res, f32)| e.0 == r) {
                                Some(e) => e.1 += x,
                                None => v.push((r, x)),
                            }
                        }
                    }
                    if l.medium.cable() {
                        one = one.min(self.flow_rate(l));
                    }
                    links.push((l.id, towards_a));
                    out.who.entry(l.id).or_default().push(d.id);
                }
                let speed = |res: &[(Res, f32)]| res.iter().filter(|x| x.1 > 0.0).map(|(r, x)| out.cap[r] / x).fold(one, f32::min);
                let key = ((d.id as u64) << 16) | idx as u64;
                let (wd, wu) = self.want(key, u, act, (down, up), speed(&back), live, got.get(&(d.id, idx)).copied());
                for (to_dev, want, res) in [(true, wd, back), (false, wu, along)] {
                    let (cap, grab) = match want {
                        Want::Fixed(x) => (x, false),
                        Want::Grab(most) => (most.min(speed(&res)), true),
                    };
                    let links = links.iter().map(|(l, a)| (*l, *a == to_dev)).collect();
                    asks.push(Ask { dev: d.id, idx, to_dev, q: d.qos, cap, grab, res: res.into_iter().filter(|x| x.1 > 0.0).collect(), links });
                }
            }
        }
        let mut left = out.cap.clone();
        let mut rate = vec![0.0f32; asks.len()];
        for q in Qos::ALL {
            let mut on: Vec<usize> = (0..asks.len()).filter(|&i| asks[i].q == q && asks[i].cap > 1e-4 && !asks[i].res.is_empty()).collect();
            for _ in 0..asks.len() + left.len() + 2 {
                if on.is_empty() {
                    break;
                }
                let mut cost: HashMap<Res, f32> = HashMap::new();
                for &i in &on {
                    for (r, x) in &asks[i].res {
                        *cost.entry(*r).or_default() += x;
                    }
                }
                let step = on.iter().map(|&i| asks[i].cap - rate[i]).chain(cost.iter().map(|(r, x)| left[r].max(0.0) / x)).fold(f32::INFINITY, f32::min);
                if !step.is_finite() {
                    break;
                }
                for &i in &on {
                    rate[i] += step;
                }
                for (r, x) in &cost {
                    *left.get_mut(r).unwrap() -= step * x;
                }
                on.retain(|&i| asks[i].cap - rate[i] > 1e-4 && asks[i].res.iter().all(|(r, _)| left[r] > 1e-5 * out.cap[r].max(1.0)));
            }
        }
        for (a, r) in asks.iter().zip(rate) {
            for (res, x) in &a.res {
                *out.used.entry((*res, a.q)).or_default() += r * x;
                *out.count.entry((*res, a.q)).or_default() += 1;
                *out.wanted.entry(*res).or_default() += if a.grab { r } else { a.cap } * x;
            }
            for (l, to_a) in &a.links {
                let e = out.per_link.entry(*l).or_default();
                if *to_a { e.0 += r } else { e.1 += r }
            }
            if a.grab && a.to_dev {
                out.got.insert((a.dev, a.idx), r);
            }
        }
        out
    }

    /// What one of a device's demands asks for, (towards it, from it). As an average (not `live`) apps are steady
    /// at their mean rate and only a big download grabs; live they come and go like the real thing. `speed` is
    /// its way's speed towards the device, `got` what it got while busy last time.
    fn want(&self, key: u64, u: Option<Use>, act: f32, (down, up): (f32, f32), speed: f32, live: bool, got: Option<f32>) -> (Want, Want) {
        use Want::*;
        if !live {
            return match u {
                Some(Use::Download) if act >= 0.99 => (Grab(f32::INFINITY), Fixed(up)),
                // Downloading part of the time: on average that share of what the way carries.
                Some(Use::Download) => (Fixed(act * speed), Fixed(up)),
                _ => (Fixed(down), Fixed(up)),
            };
        }
        let t = self.clock;
        let unit = |salt: u64, slot: f64| crate::sim::unit(self.seed, key ^ salt, slot as i64 as u64);
        let jit = |period: f64, amp: f32| (1.0 + amp * crate::sim::wander(self.seed, key ^ 0x717, t, period)).max(0.0);
        // Everyday apps run in sessions of a few minutes, `act` of the time; a busier household (act > 1) uses more.
        let scale = if self.custom_traffic { 1.0 } else { act.max(1.0) };
        if !self.custom_traffic && unit(0x5E55, (t / 300.0).floor()) >= act {
            return (Fixed(0.0), Fixed(0.0));
        }
        let (down, up) = u.map_or((down, up), |u| (u.demand().0 * scale, u.demand().1 * scale));
        // How fast a burst goes: what it got last time, else the way's speed.
        let fast = got.filter(|g| *g > 0.01).unwrap_or(speed);
        match u {
            Some(Use::Browsing) => {
                // A click loads a page with its pictures and scripts (~2.5 MB = 20 Mbit) as fast as the way allows
                // (servers and TCP start-up: at most ~100 Mbit/s); in between only a trickle. Clicks come often
                // enough to average the app's demand; on a slow way a page takes long and the loads run together.
                const PAGE: f32 = 20.0;
                let took = (PAGE / fast.min(100.0).max(0.05)) as f64;
                let clicks = down / PAGE;
                let busy = (0..=took.ceil() as i64 + 1).any(|k| {
                    let s = t.floor() - k as f64;
                    let at = s + unit(0xC1C1, s) as f64;
                    unit(0xC11C, s) < clicks && t >= at && t < at + took
                });
                if busy { (Grab(100.0), Fixed(up.max(1.0))) } else { (Fixed(0.05), Fixed(0.02)) }
            }
            Some(u @ (Use::Stream4k | Use::StreamHd | Use::Music)) => {
                // Players fetch the next seconds of video (music: of the song) as fast as they can, then rest until
                // the buffer runs low. When the way is slower than the video, the rest disappears: it fetches nonstop
                // and the picture drops to a lower quality.
                let (period, most) = if u == Use::Music { (10.0, 20.0) } else { (4.0, 150.0) };
                let on = (down * period / fast.min(most).max(0.01)).max(0.6);
                let phase = unit(0xFA5E, 0.0) * period;
                if on >= period || ((t + phase as f64).rem_euclid(period as f64) as f32) < on {
                    (Grab(most), Fixed(up))
                } else {
                    (Fixed(0.02), Fixed(0.02))
                }
            }
            Some(Use::VideoCall) => (Fixed(down * jit(2.0, 0.15)), Fixed(up * jit(2.0, 0.15))),
            Some(Use::Gaming) => (Fixed(down * jit(0.5, 0.35)), Fixed(up * jit(0.5, 0.35))),
            // More movement in view: more to encode.
            Some(Use::Camera) => (Fixed(down), Fixed(up * jit(3.0, 0.25))),
            Some(Use::Download) => (Grab(f32::INFINITY), Fixed(up)),
            Some(Use::Backup) => (Fixed(down), Fixed(up)),
            None => (Fixed(down * jit(1.5, 0.25)), Fixed(up * jit(1.5, 0.25))),
        }
    }

    /// How full a link is (0..1): the fullest capacity it draws on (for Wi-Fi: its channel's airtime).
    pub fn utilisation(&self, load: &Load, link: u32) -> f32 {
        load.fullest(link, |r| load.used_upto(r, Qos::Low))
    }

    /// How much the apps on a link ask of it (1.0 = all it carries; above that they get less than they want).
    pub fn wanted(&self, load: &Load, link: u32) -> f32 {
        load.fullest(link, |r| load.wanted.get(&r).copied().unwrap_or(0.0))
    }

    /// How full a link looks to traffic of priority `q`: a full link serves higher priorities first, so only
    /// traffic of the same or a higher priority is in its way (with nobody prioritised: everyone's).
    pub fn utilisation_for(&self, load: &Load, link: u32, q: Qos) -> f32 {
        load.fullest(link, |r| load.used_upto(r, q))
    }

    /// What one more transfer of priority `q` would get over a link right now (Mbit/s, towards end a or b): what
    /// higher priorities leave, less what its own class uses, but at least a fair share with the transfers there.
    pub fn fair_share(&self, load: &Load, l: &crate::net::Link, towards_a: bool, q: Qos) -> f32 {
        let rs = load.res.get(&l.id).cloned().unwrap_or_else(|| self.link_res(l));
        rs.iter()
            .filter_map(|(r, c, wa, wb)| {
                let x = if towards_a { *wa } else { *wb };
                (x > 0.0).then(|| {
                    let free = c - Qos::ALL.iter().filter(|k| **k < q).filter_map(|k| load.used.get(&(*r, *k))).sum::<f32>();
                    let mine = load.used.get(&(*r, q)).copied().unwrap_or(0.0);
                    let n = load.count.get(&(*r, q)).copied().unwrap_or(0) as f32;
                    (free - mine).max(free / (n + 1.0)).max(0.0) / x
                })
            })
            .fold(self.flow_rate(l), f32::min)
    }

    /// Airtime an access point's channel needs for all its devices (1.0 = the channel is full; above that they
    /// get less than they want), on its busiest band.
    pub fn airtime(&self, load: &Load, ap: u32) -> f32 {
        let Some(d) = self.dev(ap) else { return 0.0 };
        let src = self.chan_src(d).id;
        (0..3u8).map(|b| load.wanted.get(&(src, AIR + b)).copied().unwrap_or(0.0)).fold(0.0, f32::max)
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
    fn a_busier_household_fills_links_more_and_the_day_peaks_in_the_evening() {
        let mut n = template_seeded("home", 3);
        let tv = id(&n, "Living room TV");
        let link = n.links.iter().find(|l| l.a == tv || l.b == tv).unwrap().id;
        let one = n.utilisation(&n.loads(), link);
        n.traffic = 2.0;
        let two = n.utilisation(&n.loads(), link);
        assert!((two - 2.0 * one).abs() < 1e-3 * one.max(1.0), "{one} {two}");
        let day = n.day_load();
        assert_eq!(day.len(), 48);
        // 21:00 is busier than 04:00, and the busiest link is never less full than the internet line.
        assert!(day[42].1 > day[8].1 && day.iter().all(|d| d.1 >= d.0), "{day:?}");
    }

    #[test]
    fn downloads_stop_at_their_slowest_link_and_leave_the_rest_to_others() {
        let mut n = template_seeded("home", 3);
        n.custom_traffic = true;
        let ids: Vec<u32> = ["Anna's laptop", "Living room TV", "NAS 1"].iter().map(|x| id(&n, x)).collect();
        for d in &ids {
            n.dev_mut(*d).unwrap().test_uses = vec![Use::Download];
        }
        let ld = n.loads();
        let into = |d: u32| {
            let l = n.links.iter().find(|l| l.a == d || l.b == d).unwrap();
            let x = ld.per_link[&l.id];
            (if l.a == d { x.0 } else { x.1 }, n.link_rate(l))
        };
        // The TV sits on a 100 Mbit/s port: its download gets that, and the laptop and the NAS share the rest.
        let (tv, port) = into(ids[1]);
        assert!((tv - port).abs() < 0.5 && port < 100.0, "{tv} {port}");
        let (lap, nas) = (into(ids[0]).0, into(ids[2]).0);
        assert!((lap - nas).abs() < 0.5 && (lap + nas + tv - 500.0).abs() < 1.0, "{lap} {nas} {tv}");
    }

    /// Mbit/s flowing towards (or away from, `away`) a device over its own link.
    fn into(n: &Network, ld: &Load, d: u32, away: bool) -> f32 {
        let l = n.links.iter().find(|l| l.a == d || l.b == d).unwrap();
        let x = ld.per_link.get(&l.id).copied().unwrap_or_default();
        if (l.a == d) != away { x.0 } else { x.1 }
    }

    #[test]
    fn wifi_devices_take_turns_on_one_channel() {
        let mut n = template_seeded("home", 3);
        n.custom_traffic = true;
        let (lap, phone) = (id(&n, "Anna's laptop"), id(&n, "Phone"));
        n.dev_mut(lap).unwrap().test_uses = vec![Use::Download];
        let alone = into(&n, &n.loads(), lap, false);
        n.dev_mut(phone).unwrap().test_uses = vec![Use::Download];
        let ld = n.loads();
        let (a, p) = (into(&n, &ld, lap, false), into(&n, &ld, phone, false));
        let link = n.links.iter().find(|l| l.a == lap || l.b == lap).unwrap().id;
        // Both on the same channel: together they can't use more than its airtime, so each gets less.
        assert!(a < alone * 0.8 && p > 1.0 && n.utilisation(&ld, link) <= 1.001, "{alone} {a} {p}");
    }

    #[test]
    fn the_line_uploads_only_as_fast_as_the_plan_and_house_traffic_stays_home() {
        let mut n = template_seeded("home", 3);
        n.custom_traffic = true;
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        n.dev_mut(inet).unwrap().plan_up_mbps = 50.0;
        let (nas, pc) = (id(&n, "NAS 1"), id(&n, "Office PC"));
        n.dev_mut(nas).unwrap().test_mbps = [0.0, 400.0];
        let ld = n.loads();
        assert!((into(&n, &ld, inet, false) - 50.0).abs() < 0.5, "{}", into(&n, &ld, inet, false));
        // The same load to the PC in the house never touches the line.
        n.dev_mut(nas).unwrap().test_to = pc;
        let ld = n.loads();
        assert!(into(&n, &ld, inet, false) < 0.01 && (into(&n, &ld, pc, false) - 400.0).abs() < 0.5);
    }

    #[test]
    fn browsing_comes_in_bursts_and_slow_video_streams_nonstop() {
        let mut n = template_seeded("home", 3);
        n.custom_traffic = true;
        n.custom_bursty = true;
        n.live = true;
        let (lap, tv) = (id(&n, "Anna's laptop"), id(&n, "Living room TV"));
        n.dev_mut(lap).unwrap().test_uses = vec![Use::Browsing];
        n.dev_mut(tv).unwrap().test_uses = vec![Use::Stream4k];
        let mut run = |n: &mut Network| -> (Vec<f32>, Vec<f32>) {
            n.memory.borrow_mut().got.clear();
            (0..1200).map(|i| {
                // An uneven step, so samples don't line up with the video's 4 s rhythm.
                n.clock = i as f64 * 0.37;
                let ld = n.loads_now();
                (into(n, &ld, lap, false), into(n, &ld, tv, false))
            }).unzip()
        };
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let quiet = |v: &[f32]| v.iter().filter(|x| **x < 0.5).count() as f32 / v.len() as f32;
        let (web, video) = run(&mut n);
        // A page now and then, fetched fast; mostly quiet, about 3 Mbit/s on average.
        assert!((1.5..6.0).contains(&mean(&web)) && quiet(&web) > 0.8 && web.iter().cloned().fold(0.0, f32::max) > 30.0, "{} {}", mean(&web), quiet(&web));
        // Video fetches a few seconds at a time, averaging its 25 Mbit/s.
        assert!((15.0..35.0).contains(&mean(&video)) && quiet(&video) > 0.4, "{} {}", mean(&video), quiet(&video));
        // On a 20 Mbit/s line the buffer never fills: it fetches nonstop, all the line gives.
        let inet = n.devices.iter().find(|d| d.kind == Kind::Internet).unwrap().id;
        n.dev_mut(inet).unwrap().plan_mbps = 20.0;
        let (_, video) = run(&mut n);
        assert!(quiet(&video) < 0.05 && mean(&video) > 15.0, "{} {}", mean(&video), quiet(&video));
    }

    #[test]
    fn wifi_busy_time_follows_what_was_really_sent() {
        let mut n = template_seeded("home", 3);
        n.custom_traffic = true;
        let lap = id(&n, "Anna's laptop");
        n.dev_mut(lap).unwrap().test_uses = vec![Use::Download];
        let ld = n.loads();
        let link = n.links.iter().find(|l| l.a == lap || l.b == lap).unwrap();
        let (ap, _) = n.wifi_ends(link).unwrap();
        let band = n.link_signal(link).unwrap().band;
        // The download fills the line, not the 150 Mbit/s guess: the channel's busy time is what it really used.
        assert!((n.demand_airtime(ap, band) - 0.02 - n.utilisation(&ld, link.id)).abs() < 0.05, "{} {}", n.demand_airtime(ap, band), n.utilisation(&ld, link.id));
    }

    #[test]
    fn an_own_load_flows_from_any_device_only_in_custom_mode() {
        let mut n = template_seeded("home", 3);
        let nas = id(&n, "NAS 1");
        n.dev_mut(nas).unwrap().test_mbps = [40.0, 10.0];
        let link = n.links.iter().find(|l| l.a == nas || l.b == nas).unwrap().clone();
        assert!(n.loads().per_link.get(&link.id).is_none(), "organic ignores it");
        n.custom_traffic = true;
        let x = n.loads().per_link[&link.id];
        let (to_nas, from_nas) = if link.a == nas { x } else { (x.1, x.0) };
        assert!((to_nas - 40.0).abs() < 1e-3 && (from_nas - 10.0).abs() < 1e-3, "{x:?}");
    }

    #[test]
    fn a_custom_test_load_runs_at_full_rate_whatever_the_hour() {
        let mut n = template_seeded("home", 3);
        let tv = id(&n, "Living room TV");
        let link = n.links.iter().find(|l| l.a == tv || l.b == tv).unwrap().id;
        n.hour = 4.0;
        let night = n.loads().per_link[&link];
        n.custom_traffic = true;
        assert!(n.loads().per_link.get(&link).is_none(), "no test load yet");
        n.dev_mut(tv).unwrap().test_uses = vec![Use::Stream4k, Use::Stream4k];
        let l = n.links.iter().find(|l| l.id == link).unwrap();
        let (a, b) = n.loads().per_link[&link];
        assert!((a + b - 2.0 * (25.0 + 0.3)).abs() < 1e-3 && a + b > 10.0 * (night.0 + night.1), "{a} {b} {night:?}");
        // The video comes in towards the TV.
        let to_tv = if l.a == tv { a } else { b };
        assert!((to_tv - 50.0).abs() < 1e-3, "{a} {b}");
    }

    #[test]
    fn high_priority_goes_first_on_a_full_line() {
        let mut n = template_seeded("home", 3);
        let pc = id(&n, "Office PC");
        // Everyone else downloads; the PC plays. Its own download would fill its own queue whatever its priority.
        for d in n.devices.iter_mut().filter(|d| d.kind.is_host()) {
            d.uses.push(if d.id == pc { Use::Gaming } else { Use::Download });
        }
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
