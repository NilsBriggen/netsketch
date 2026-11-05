//! Measurements from the companion phone app: points where someone stood and scanned the air.
//! Each point lists every network heard there. In the plan, the points are pins you drag to where
//! they were taken; measured networks become neighbours (or are matched to your own access points),
//! and their position, strength and busy time are fitted to what was really measured.
use crate::net::{Device, Kind, Network, Security, WifiStd, PX_PER_M};
use crate::wifi::Band;
use serde::{Deserialize, Serialize};

/// What the companion app writes (`.netsurvey`, JSON).
#[derive(Deserialize)]
struct SurveyFile {
    format: String,
    #[serde(default)]
    device: String,
    points: Vec<FilePoint>,
    #[serde(default)]
    watch: Option<Watch>,
    #[serde(default)]
    findings: Vec<Finding>,
    #[serde(default)]
    lan: Vec<LanDevice>,
}

#[derive(Deserialize)]
struct FilePoint {
    name: String,
    #[serde(default)]
    floor: i8,
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
    #[serde(default)]
    time: String,
    #[serde(default)]
    connected: Option<Connected>,
    #[serde(default)]
    speed: Option<Speed>,
    #[serde(default)]
    ping: Option<Ping>,
    networks: Vec<Seen>,
}

/// One network heard at a point.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    pub ssid: String,
    pub bssid: String,
    /// Centre frequency (MHz) and channel number.
    pub freq: u32,
    pub channel: u16,
    pub width: u16,
    /// Average over the scans at this point (dBm).
    pub rssi: f32,
    pub security: String,
    pub standard: String,
    /// From the access point's own "BSS load" announcement, when it sends one.
    pub stations: Option<u16>,
    pub utilization: Option<f32>,
    /// Weakest and strongest reading over the scans at this point: a big spread means a flaky spot.
    pub rssi_min: Option<f32>,
    pub rssi_max: Option<f32>,
    /// What the access point announces: 802.11k/v/r, Wi-Fi RTT (fine timing), WPS, spatial streams, country.
    pub k: bool,
    pub v: bool,
    pub r: bool,
    pub ftm: bool,
    pub wps: bool,
    pub streams: Option<u8>,
    pub country: String,
    /// Distance measured with Wi-Fi RTT (802.11mc), when both sides support it.
    pub distance_m: Option<f32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Connected {
    pub ssid: String,
    pub bssid: String,
    pub rssi: f32,
    pub link_mbps: f32,
    pub freq: u32,
    /// Current send / receive rates and the best the pair could do; the Wi-Fi standard in use.
    pub tx_mbps: Option<f32>,
    pub rx_mbps: Option<f32>,
    pub max_mbps: Option<f32>,
    pub standard: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Speed {
    pub down_mbps: f32,
    pub latency_ms: f32,
    /// Upload and delay variation; 0 in files from phone app 1.0, which didn't measure them.
    pub up_mbps: f32,
    pub jitter_ms: f32,
}

/// Pings to the router at a point.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ping {
    pub avg_ms: f32,
    pub jitter_ms: f32,
    pub loss_pct: f32,
    pub max_ms: f32,
}

/// Something the phone noticed while watching the Wi-Fi: a channel change, a hand-over, a dropout, a delay spike.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WatchEvent {
    pub time: String,
    /// "channel", "roam", "drop", "spike", "loss" or "dip".
    pub kind: String,
    pub bssid: String,
    pub ssid: String,
    /// Old and new channel ("channel") or access point address ("roam").
    pub from: String,
    pub to: String,
    /// Seconds offline ("drop"), ms ("spike"), % ("loss"), dBm ("dip").
    pub value: f32,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Watch {
    pub start: String,
    pub end: String,
    pub samples: u32,
    pub events: Vec<WatchEvent>,
}

/// A problem the phone's own assistant found ("error", "warning" or "tip"), in the phone's language.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Finding {
    pub level: String,
    pub text: String,
    pub bssid: String,
    pub key: String,
}

/// A device the phone found on the network (UPnP, mDNS, a knock on common ports).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LanDevice {
    pub ip: String,
    pub name: String,
    pub kind: String,
    pub vendor: String,
    pub model: String,
}

/// What the phone watched, found and discovered, kept with the plan (the latest of each).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PhoneReport {
    pub phone: String,
    pub watch: Option<Watch>,
    pub findings: Vec<Finding>,
    pub lan: Vec<LanDevice>,
}

/// A measuring point in the plan.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MeasurePoint {
    pub id: u32,
    pub name: String,
    pub pos: [f32; 2],
    pub floor: i8,
    /// False until someone put the pin where the measurement was taken.
    pub placed: bool,
    pub time: String,
    pub phone: String,
    pub connected: Option<Connected>,
    pub speed: Option<Speed>,
    pub ping: Option<Ping>,
    pub networks: Vec<Seen>,
}

/// One physical access point as measured: all its networks (bands, names) grouped together.
#[derive(Clone, Debug)]
pub struct Measured {
    pub key: String,
    pub ssid: String,
    /// Strongest reading anywhere, and at which point.
    pub best: f32,
    pub best_point: u32,
    /// (band, channel, width) it sends on.
    pub radios: Vec<(Band, u16, u16)>,
    pub utilization: Option<f32>,
    pub security: String,
    pub standard: String,
    /// The device in the plan it belongs to (your access point or an added neighbour).
    pub dev: Option<u32>,
}

pub fn band_of(freq: u32) -> Band {
    match freq {
        0..=3000 => Band::B24,
        3001..=5925 => Band::B5,
        _ => Band::B6,
    }
}

/// Access points send one network per band and name from neighbouring addresses: group by the
/// first five bytes of the address (and the name).
pub fn key_of(s: &Seen) -> String {
    let b = s.bssid.to_lowercase();
    let prefix = b.rsplit_once(':').map(|(p, _)| p.to_string()).unwrap_or(b);
    format!("{}|{prefix}", s.ssid)
}

impl Network {
    /// Reads a companion survey file. Points without a position are lined up in the middle of the
    /// storey so they can be dragged to where they were taken.
    pub fn import_companion(&mut self, text: &str) -> Result<(usize, usize), String> {
        let f: SurveyFile = serde_json::from_str(text).map_err(|e| format!("not a survey file: {e}"))?;
        if f.format != "netsketch-survey" {
            return Err("not a NetSketch survey file".into());
        }
        let mut nets = 0;
        // A point already here (sent live, then imported from the shared file too) isn't added twice.
        let fresh: Vec<FilePoint> = f.points.into_iter().filter(|p| p.time.is_empty() || !self.points.iter().any(|q| q.time == p.time && q.name == p.name && q.phone == f.device)).collect();
        let n_points = fresh.len();
        // Positions must be real numbers on a plausible scale (within 10 km).
        if fresh.iter().any(|p| [p.x, p.y].iter().flatten().any(|v| !v.is_finite() || v.abs() > 10_000.0) || p.networks.iter().any(|s| !s.rssi.is_finite())) {
            return Err("the file has impossible positions or readings".into());
        }
        for (i, mut p) in fresh.into_iter().enumerate() {
            // The same storeys a plan allows (Network::sanitize).
            p.floor = p.floor.clamp(-3, 20);
            let placed = p.x.is_some() && p.y.is_some();
            let pos = match (p.x, p.y) {
                (Some(x), Some(y)) => [x * PX_PER_M, y * PX_PER_M],
                _ => {
                    let c = self.middle(p.floor);
                    [c[0] + (i % 6) as f32 * 1.5 * PX_PER_M, c[1] + (i / 6) as f32 * 1.5 * PX_PER_M]
                }
            };
            nets += p.networks.len();
            self.next_id += 1;
            self.points.push(MeasurePoint { id: self.next_id, name: p.name, pos, floor: p.floor, placed, time: p.time, phone: f.device.clone(), connected: p.connected, speed: p.speed, ping: p.ping, networks: p.networks });
        }
        self.take_report(&f.device, f.watch, f.findings, f.lan);
        // Networks named like one of your access points belong to it.
        for m in self.measured_networks() {
            if m.dev.is_none() {
                if let Some(d) = self.own_radio_for(&m) {
                    self.match_measured(&m.key, d);
                }
            }
        }
        Ok((n_points, nets))
    }

    /// Keeps the phone's watch log (events added up, none twice), its latest findings and device list. An access
    /// point the phone saw change channel picks its channel by itself.
    fn take_report(&mut self, phone: &str, watch: Option<Watch>, findings: Vec<Finding>, lan: Vec<LanDevice>) {
        if !phone.is_empty() {
            self.phone.phone = phone.to_string();
        }
        if let Some(w) = watch {
            let mine = self.phone.watch.get_or_insert_with(Watch::default);
            if mine.start.is_empty() || (!w.start.is_empty() && w.start < mine.start) {
                mine.start = w.start.clone();
            }
            mine.end = mine.end.clone().max(w.end.clone());
            mine.samples = mine.samples.max(w.samples);
            for e in w.events {
                if !mine.events.contains(&e) {
                    mine.events.push(e);
                }
            }
            mine.events.sort_by(|a, b| a.time.cmp(&b.time));
            // A day's worth is plenty; old events go first.
            let extra = mine.events.len().saturating_sub(2000);
            mine.events.drain(..extra);
        }
        if !findings.is_empty() {
            self.phone.findings = findings;
        }
        if !lan.is_empty() {
            self.phone.lan = lan;
        }
        let jumpers: Vec<u32> = self.phone.watch.iter().flat_map(|w| &w.events).filter(|e| e.kind == "channel").filter_map(|e| self.device_of_bssid(&e.ssid, &e.bssid)).collect();
        for id in jumpers {
            if let Some(d) = self.dev_mut(id) {
                d.auto_ch = true;
            }
        }
    }

    /// The plan's device a measured access point address belongs to, if it was matched.
    pub fn device_of_bssid(&self, ssid: &str, bssid: &str) -> Option<u32> {
        let key = key_of(&Seen { ssid: ssid.into(), bssid: bssid.into(), ..Default::default() });
        self.measured.iter().find(|m| m.0 == key).map(|m| m.1).filter(|d| self.dev(*d).is_some())
    }

    /// How often the phone saw this device's access point change channel by itself.
    pub fn channel_jumps(&self, id: u32) -> usize {
        self.phone.watch.iter().flat_map(|w| &w.events).filter(|e| e.kind == "channel" && self.device_of_bssid(&e.ssid, &e.bssid) == Some(id)).count()
    }

    /// Checks that need the phone's measurements: an extender's uplink as heard right next to it, and what the
    /// phone's own assistant found.
    pub fn measured_checks(&self, v: &mut Vec<crate::net::Issue>) {
        use crate::net::{Issue, Level};
        let push = |v: &mut Vec<Issue>, level, dev, text| v.push(Issue { level, dev, link: None, text, fix: None });
        for x in self.devices.iter().filter(|d| d.kind.radio() && !d.kind.foreign()) {
            let Some(up) = self.upstream(x) else { continue };
            let Some(p) = self.points.iter().filter(|p| p.placed && p.floor == x.floor).filter(|p| ((p.pos[0] - x.pos[0]).hypot(p.pos[1] - x.pos[1])) < 3.0 * PX_PER_M).min_by(|a, b| {
                let d = |p: &MeasurePoint| (p.pos[0] - x.pos[0]).hypot(p.pos[1] - x.pos[1]);
                d(a).total_cmp(&d(b))
            }) else {
                continue;
            };
            // Loudest reading at that point of each of your access points.
            let heard = |dev: u32| p.networks.iter().filter(|s| self.measured.iter().any(|m| m.0 == key_of(s) && m.1 == dev)).map(|s| s.rssi).fold(None, |m: Option<f32>, r| Some(m.map_or(r, |m| m.max(r))));
            let Some(uplink) = heard(up.id) else { continue };
            if uplink < -70.0 {
                push(v, Level::Warning, Some(x.id), crate::i18n::trf("Measured next to {} ('{}'): {}, the access point it repeats, came in at only {} dBm. Its link back is weak, so everyone on it is slow: move it closer to {}.", &[&x.name, &p.name, &up.name, &format!("{:.0}", uplink), &up.name]));
            }
            if let Some((o, r)) = self.devices.iter().filter(|o| o.kind.radio() && !o.kind.foreign() && o.id != x.id && o.id != up.id && !self.feeds_from(o, x.id)).filter_map(|o| heard(o.id).map(|r| (o, r))).max_by(|a, b| a.1.total_cmp(&b.1)) {
                if r >= uplink + 8.0 {
                    push(v, Level::Warning, Some(x.id), crate::i18n::trf("Measured next to {} ('{}'): {} is {} dBm there, {} only {} dBm. If {} really repeats {}, it uses the worse uplink: check it in its app.", &[&x.name, &p.name, &o.name, &format!("{:.0}", r), &up.name, &format!("{:.0}", uplink), &x.name, &up.name]));
                }
            }
        }
        // What the phone measured from your own access points against what the plan says.
        for m in self.measured_networks() {
            let Some(d) = m.dev.and_then(|id| self.dev(id)).filter(|d| d.kind.radio() && !d.kind.foreign()) else { continue };
            for (band, ch, _) in &m.radios {
                let planned = self.channel(d, *band);
                if *ch != planned && d.bands[band.i()] {
                    push(v, Level::Tip, Some(d.id), crate::i18n::trf("The plan has {} on {} channel {}, but the phone measured channel {}. It probably picked its channel by itself: set the plan to what's real, or fix the channel on the device.", &[&d.name, &(band.label()), &planned, ch]));
                }
            }
            let seen: Vec<&Seen> = self.points.iter().flat_map(|p| &p.networks).filter(|s| key_of(s) == m.key).collect();
            // Only files that report capabilities can tell (phone app 0.3 and newer).
            let reports = seen.iter().any(|s| s.streams.is_some() || s.k || s.v || s.r);
            if d.fast_roam && reports && !seen.iter().any(|s| s.k || s.v || s.r) {
                push(v, Level::Tip, Some(d.id), crate::i18n::trf("The plan has fast roaming on {}, but the phone heard no 802.11k, v or r from it: it's off on the real device. Turn it on there (often 'fast roaming', 'smart connect' or 'seamless roaming').", &[&d.name]));
            }
            if seen.iter().any(|s| s.wps) {
                push(v, Level::Tip, Some(d.id), crate::i18n::trf("{} has WPS switched on (the phone saw it announced). Its PIN method can be cracked; turn WPS off unless you need it right now.", &[&d.name]));
            }
        }
        for f in &self.phone.findings {
            let level = match f.level.as_str() {
                "error" => Level::Error,
                "warning" => Level::Warning,
                _ => Level::Tip,
            };
            push(v, level, self.device_of_bssid("", &f.bssid).or_else(|| self.measured_dev_by_bssid(&f.bssid)), crate::i18n::trf("From the phone: {}", &[&f.text]));
        }
    }

    /// The device a bssid's access point was matched to, whatever network name it was seen with.
    fn measured_dev_by_bssid(&self, bssid: &str) -> Option<u32> {
        if bssid.is_empty() {
            return None;
        }
        self.points.iter().flat_map(|p| &p.networks).filter(|s| s.bssid.eq_ignore_ascii_case(bssid)).find_map(|s| self.measured.iter().find(|m| m.0 == key_of(s)).map(|m| m.1))
    }

    /// Ties a measured network to a device, replacing an earlier match.
    fn match_measured(&mut self, key: &str, dev: u32) {
        self.measured.retain(|m| m.0 != key);
        self.measured.push((key.to_string(), dev));
    }

    /// Middle of a storey's walls (or devices), where new pins go.
    fn middle(&self, floor: i8) -> [f32; 2] {
        let pts: Vec<[f32; 2]> = self.walls.iter().filter(|w| w.floor == floor).flat_map(|w| [w.a, w.b]).chain(self.devices.iter().filter(|d| d.floor == floor).map(|d| d.pos)).collect();
        if pts.is_empty() {
            return [0.0, 0.0];
        }
        let n = pts.len() as f32;
        [pts.iter().map(|p| p[0]).sum::<f32>() / n, pts.iter().map(|p| p[1]).sum::<f32>() / n]
    }

    /// Your own access point that sends this name; with several (mesh, roaming), the one the model
    /// says should be strongest where the network was measured strongest.
    fn own_radio_for(&self, m: &Measured) -> Option<u32> {
        let at = self.points.iter().find(|p| p.id == m.best_point)?;
        self.devices
            .iter()
            .filter(|d| d.kind.radio() && !d.kind.foreign() && Network::names_of(d).iter().any(|n| n.0 == m.ssid))
            .max_by(|a, b| self.best_rssi(a, at.pos, at.floor).total_cmp(&self.best_rssi(b, at.pos, at.floor)))
            .map(|d| d.id)
    }

    /// Every access point heard at any point, strongest first.
    pub fn measured_networks(&self) -> Vec<Measured> {
        let mut out: Vec<Measured> = vec![];
        for p in &self.points {
            for s in &p.networks {
                let key = key_of(s);
                let i = match out.iter().position(|m| m.key == key) {
                    Some(i) => i,
                    None => {
                        let dev = self.measured.iter().find(|m| m.0 == key).map(|m| m.1).filter(|d| self.dev(*d).is_some());
                        out.push(Measured { key, ssid: s.ssid.clone(), best: -200.0, best_point: p.id, radios: vec![], utilization: None, security: s.security.clone(), standard: s.standard.clone(), dev });
                        out.len() - 1
                    }
                };
                let m = &mut out[i];
                if s.rssi > m.best {
                    m.best = s.rssi;
                    m.best_point = p.id;
                }
                let r = (band_of(s.freq), s.channel, s.width.max(20));
                if !m.radios.iter().any(|x| x.0 == r.0) {
                    m.radios.push(r);
                }
                if let Some(u) = s.utilization {
                    m.utilization = Some(m.utilization.map_or(u, |x: f32| x.max(u)));
                }
            }
        }
        out.sort_by(|a, b| b.best.total_cmp(&a.best));
        out
    }

    /// Readings of one measured access point at the pins that have been placed: (spot, storey, band, dBm).
    fn readings_of(&self, key: &str) -> Vec<([f32; 2], i8, Band, f32)> {
        self.points.iter().filter(|p| p.placed).flat_map(|p| p.networks.iter().filter(|s| key_of(s) == key).map(move |s| (p.pos, p.floor, band_of(s.freq), s.rssi))).collect()
    }

    /// Links a measured access point to a device in the plan (or unlinks it).
    pub fn link_measured(&mut self, key: &str, dev: Option<u32>) {
        self.measured.retain(|m| m.0 != key);
        if let Some(d) = dev {
            self.match_measured(key, d);
        }
    }

    /// Adds a measured network as a neighbour with exactly the channels, widths and busy time that
    /// were measured, then fits where it is and how loud.
    pub fn add_measured_neighbour(&mut self, key: &str) -> Option<u32> {
        let m = self.measured_networks().into_iter().find(|m| m.key == key)?;
        let at = self.points.iter().find(|p| p.id == m.best_point)?.clone();
        let id = self.add_device(Kind::Neighbor, [at.pos[0] + 4.0 * PX_PER_M, at.pos[1]]);
        let d = self.dev_mut(id).unwrap();
        d.name = if m.ssid.is_empty() { crate::i18n::tr("Hidden network").into() } else { m.ssid.clone() };
        d.ssid = m.ssid.clone();
        d.floor = at.floor;
        d.wifi = match m.standard.as_str() {
            s if s.contains('7') => WifiStd::Wifi7,
            s if s.contains('6') && m.radios.iter().any(|r| r.0 == Band::B6) => WifiStd::Wifi6E,
            s if s.contains('6') => WifiStd::Wifi6,
            _ => WifiStd::Wifi5,
        };
        d.bands = [false; 3];
        for (b, ch, w) in &m.radios {
            d.bands[b.i()] = true;
            d.ch[b.i()] = *ch;
            d.width[b.i()] = *w;
        }
        if let Some(u) = m.utilization {
            d.busy = u.clamp(0.0, 1.0);
        }
        d.security = match m.security.as_str() {
            "Open" | "Enhanced Open" => Security::Open,
            "WEP" => Security::Wep,
            "WPA3" => Security::Wpa3,
            "WPA Enterprise" => Security::Wpa3Ent,
            _ => Security::Wpa2,
        };
        d.notes = format!("Measured with the companion app ({} dBm at '{}').", m.best.round(), at.name);
        crate::wifi::normalize_radio(d);
        // normalize_radio may pick channels; keep the measured ones.
        let d = self.dev_mut(id).unwrap();
        for (b, ch, w) in &m.radios {
            d.ch[b.i()] = *ch;
            d.width[b.i()] = *w;
        }
        self.match_measured(key, id);
        self.fit_measured(id, true);
        Some(id)
    }

    /// Fits a measured device to its readings: its strength (always) and, if `place`, where it is.
    /// Returns the average error left (dB), or None without placed readings.
    pub fn fit_measured(&mut self, id: u32, place: bool) -> Option<f32> {
        let keys: Vec<String> = self.measured.iter().filter(|m| m.1 == id).map(|m| m.0.clone()).collect();
        let reads: Vec<_> = keys.iter().flat_map(|k| self.readings_of(k)).collect();
        if reads.is_empty() {
            return None;
        }
        // Wi-Fi RTT distances the phone measured to this access point (metres, good to 1–2 m).
        let dists: Vec<([f32; 2], i8, f32)> = self.points.iter().filter(|p| p.placed).flat_map(|p| p.networks.iter().filter(|s| keys.contains(&key_of(s))).filter_map(move |s| s.distance_m.filter(|d| d.is_finite() && *d > 0.0).map(|d| (p.pos, p.floor, d)))).collect();
        let mut probe: Device = self.dev(id)?.clone();
        probe.cal_db = 0.0;
        // Offset that makes the model match on average, and what's left over.
        let fit = |n: &Network, p: &Device| {
            let res: Vec<f32> = reads.iter().map(|(pos, fl, b, rssi)| rssi - n.rssi_at(p, *pos, *fl, *b)).collect();
            let k = res.iter().sum::<f32>() / res.len() as f32;
            (k, res.iter().map(|r| (r - k).powi(2)).sum::<f32>() / res.len() as f32)
        };
        if place {
            // Search around the pins: neighbours usually live outside your walls, and their routers
            // are rarely far louder or quieter than a typical one.
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (p, ..) in &reads {
                x0 = x0.min(p[0]);
                y0 = y0.min(p[1]);
                x1 = x1.max(p[0]);
                y1 = y1.max(p[1]);
            }
            let home: Vec<[f32; 2]> = self.walls.iter().filter(|w| w.floor == probe.floor).flat_map(|w| [w.a, w.b]).collect();
            let inside = |c: [f32; 2]| !home.is_empty() && home.iter().any(|h| h[0] <= c[0]) && home.iter().any(|h| h[0] >= c[0]) && home.iter().any(|h| h[1] <= c[1]) && home.iter().any(|h| h[1] >= c[1]);
            let pad = 20.0 * PX_PER_M;
            // About 1 m steps, but never more than ~120 × 120 tries however far apart the pins are.
            let (w, h) = (x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad);
            let step = PX_PER_M.max(w.max(h) / 120.0);
            let (nx, ny) = ((w / step) as usize, (h / step) as usize);
            let mut best = (f32::MAX, probe.pos);
            for j in 0..=ny {
                let y = y0 - pad + j as f32 * step;
                for i in 0..=nx {
                    let x = x0 - pad + i as f32 * step;
                    probe.pos = [x, y];
                    let (k, err) = fit(self, &probe);
                    let rtt: f32 = dists.iter().map(|(p, fl, d)| {
                        let flat = (p[0] - x).hypot(p[1] - y) / PX_PER_M;
                        let up = self.floor_z(*fl) - self.floor_z(probe.floor);
                        ((flat * flat + up * up).sqrt() - d).powi(2)
                    }).sum::<f32>() / dists.len().max(1) as f32;
                    let score = err + 0.05 * k * k + 4.0 * rtt + if inside([x, y]) { 40.0 } else { 0.0 };
                    if score < best.0 {
                        best = (score, [x, y]);
                    }
                }
            }
            probe.pos = best.1;
        }
        let (k, err) = fit(self, &probe);
        let d = self.dev_mut(id)?;
        d.pos = probe.pos;
        d.cal_db = k.clamp(-30.0, 30.0);
        Some(err.sqrt())
    }

    /// Fits every measured neighbour (position and strength) and calibrates your own access points.
    pub fn fit_all_measured(&mut self) -> String {
        let ids: Vec<u32> = self.measured.iter().map(|m| m.1).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let mut done = 0;
        for id in ids {
            let Some(d) = self.dev(id) else { continue };
            let place = d.kind == Kind::Neighbor;
            if self.fit_measured(id, place).is_some() {
                done += 1;
            }
        }
        match self.measured_error() {
            Some(e) => crate::i18n::trf("Fitted {} access points to the measurements — the model is now off by {} dB on average.", &[&done, &format!("{e:.1}")]),
            None => crate::i18n::tr("Place the measuring pins on the map first (drag them to where you stood).").into(),
        }
    }

    /// Average difference between model and measurement over all placed pins.
    pub fn measured_error(&self) -> Option<f32> {
        let e: Vec<f32> = self
            .measured
            .iter()
            .filter_map(|(k, d)| self.dev(*d).map(|d| (k, d)))
            .flat_map(|(k, d)| self.readings_of(k).into_iter().map(move |(pos, fl, b, rssi)| (self.rssi_at(d, pos, fl, b) - rssi).abs()))
            .collect();
        (!e.is_empty()).then(|| e.iter().sum::<f32>() / e.len() as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::template;

    /// Written by the companion app's own serializer (see companion-android, SurveyJsonTest).
    const SAMPLE: &str = include_str!("../companion-android/sample.netsurvey");

    #[test]
    fn a_companion_file_becomes_pins_and_neighbours_that_match_the_measurement() {
        let mut n = template("flat");
        n.devices.retain(|d| d.kind != Kind::Neighbor);
        let (pts, nets) = n.import_companion(SAMPLE).unwrap();
        assert!(pts >= 3 && nets >= 6, "{pts} {nets}");
        assert!(n.points.iter().all(|p| !p.placed), "positions come from the person, not the phone");
        let sp = n.points.iter().find_map(|p| p.speed.as_ref()).unwrap();
        assert!((sp.up_mbps - 48.5).abs() < 0.01 && (sp.jitter_ms - 2.1).abs() < 0.01, "upload and jitter from the phone");
        let found = n.measured_networks();
        // The flat's own router is recognised by its name; a neighbour is not.
        let own = found.iter().find(|m| m.ssid == "HomeNet").unwrap();
        assert_eq!(own.dev, n.devices.iter().find(|d| d.kind == Kind::HomeRouter).map(|d| d.id));
        let nb = found.iter().find(|m| m.ssid == "Flat-3B").unwrap().clone();
        assert!(nb.dev.is_none() && nb.utilization.is_some() && nb.radios.len() == 2);
        // Put the pins where they were taken (bedroom, living room, hall) and add the neighbour.
        for (p, at) in n.points.iter_mut().zip([[1.5, 1.5], [6.5, 3.0], [3.0, 5.0]]) {
            p.pos = [at[0] * PX_PER_M, at[1] * PX_PER_M];
            p.placed = true;
        }
        let id = n.add_measured_neighbour(&nb.key).unwrap();
        let d = n.dev(id).unwrap();
        assert_eq!((d.kind, d.ssid.as_str(), d.ch[0], d.ch[1], d.width[1]), (Kind::Neighbor, "Flat-3B", 6, 44, 80));
        assert!((d.busy - nb.utilization.unwrap()).abs() < 1e-3);
        // Its signal now matches what the phone measured, within a few dB, and it sits outside the flat.
        let err = n.fit_measured(id, false).unwrap();
        assert!(err < 4.0, "{err}");
        let p = n.dev(id).unwrap().pos;
        assert!(p[0] < 0.0 || p[0] > 9.0 * PX_PER_M || p[1] < 0.0 || p[1] > 6.5 * PX_PER_M, "{p:?}");
        assert!(n.fit_all_measured().contains("off by"));
        assert!(n.measured_error().unwrap() < 6.0);
    }

    #[test]
    fn the_same_points_twice_are_added_once_and_a_deleted_neighbour_lets_go_of_its_networks() {
        let mut n = template("flat");
        let (first, _) = n.import_companion(SAMPLE).unwrap();
        // Sent live, then imported from the shared file as well.
        assert_eq!(n.import_companion(SAMPLE).unwrap().0, 0);
        assert_eq!(n.points.len(), first);
        let nb = n.measured_networks().into_iter().find(|m| m.ssid == "Flat-3B").unwrap();
        let id = n.add_measured_neighbour(&nb.key).unwrap();
        n.remove_device(id);
        assert!(n.measured.iter().all(|m| m.1 != id));
        let again = n.add_measured_neighbour(&nb.key).unwrap();
        assert_eq!(n.measured.iter().filter(|m| m.0 == nb.key).count(), 1);
        assert_eq!(n.measured_networks().iter().find(|m| m.key == nb.key).unwrap().dev, Some(again));
    }

    #[test]
    fn grouping_and_bands() {
        let s = |b: &str| Seen { ssid: "X".into(), bssid: b.into(), ..Default::default() };
        assert_eq!(key_of(&s("AA:BB:CC:DD:EE:01")), key_of(&s("aa:bb:cc:dd:ee:05")));
        assert_ne!(key_of(&s("AA:BB:CC:DD:EF:01")), key_of(&s("AA:BB:CC:DD:EE:01")));
        assert_eq!((band_of(2437), band_of(5500), band_of(6115)), (Band::B24, Band::B5, Band::B6));
        assert!(Network::default().import_companion("{\"format\":\"other\",\"points\":[]}").is_err());
        assert!(Network::default().import_companion("not json").is_err());
        let huge = r#"{"format":"netsketch-survey","points":[{"name":"x","x":1e39,"y":0,"networks":[]}]}"#;
        assert!(Network::default().import_companion(huge).is_err(), "impossible positions are refused, not searched forever");
    }

    /// Router left, wired access point right, an extender next to the access point but repeating the router.
    fn far_extender() -> (Network, u32, u32, u32) {
        use crate::net::Medium;
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        let ap = n.add_device(Kind::AccessPoint, [800.0, 0.0]);
        let x = n.add_device(Kind::Extender, [740.0, 40.0]);
        for id in [r, ap, x] {
            n.dev_mut(id).unwrap().ssid = "Home".into();
        }
        n.connect(r, ap, Medium::Cat6).unwrap();
        n.connect(r, x, Medium::WiFi).unwrap();
        (n, r, ap, x)
    }

    #[test]
    fn an_extender_on_the_worse_uplink_is_found_and_moved() {
        let (mut n, _r, ap, x) = far_extender();
        let issues = n.check();
        let i = issues.iter().find(|i| i.text.contains("found first")).expect("the worse uplink is reported");
        n.apply_fix(i.fix.as_ref().unwrap());
        let up = n.upstream(n.dev(x).unwrap()).unwrap().id;
        assert_eq!(up, ap);
        assert!(!n.check().iter().any(|i| i.text.contains("found first")));
    }

    #[test]
    fn measured_next_to_an_extender_its_uplink_is_checked() {
        let (mut n, r, ap, x) = far_extender();
        let json = r#"{"format":"netsketch-survey","device":"Pixel","points":[{"name":"Hall","floor":0,"x":37.5,"y":2.0,"time":"t1","networks":[
            {"ssid":"Home","bssid":"aa:00:00:00:00:01","freq":5500,"channel":100,"rssi":-79},
            {"ssid":"Home","bssid":"bb:00:00:00:00:01","freq":5500,"channel":36,"rssi":-52}]}]}"#;
        n.import_companion(json).unwrap();
        n.link_measured("Home|aa:00:00:00:00", Some(r));
        n.link_measured("Home|bb:00:00:00:00", Some(ap));
        let all: Vec<String> = n.check().into_iter().filter(|i| i.dev == Some(x)).map(|i| i.text).collect();
        assert!(all.iter().any(|t| t.contains("came in at only -79") || t.contains("came in at only −79")), "{all:?}");
        assert!(all.iter().any(|t| t.contains("uses the worse uplink")), "{all:?}");
        // The router was measured on channel 100 with WPS on and no 802.11k/v/r, though the plan says otherwise.
        n.dev_mut(r).unwrap().fast_roam = true;
        n.points[0].networks[0].wps = true;
        n.points[0].networks[0].streams = Some(2);
        let mine: Vec<String> = n.check().into_iter().filter(|i| i.dev == Some(r)).map(|i| i.text).collect();
        assert!(mine.iter().any(|t| t.contains("measured channel 100")), "{mine:?}");
        assert!(mine.iter().any(|t| t.contains("no 802.11k, v or r")) && mine.iter().any(|t| t.contains("WPS")), "{mine:?}");
    }

    #[test]
    fn routers_that_both_pick_channels_by_themselves_are_flagged_and_the_phone_proves_it() {
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        let nb = n.add_device(Kind::Neighbor, [0.0, 200.0]);
        let tips = |n: &Network| n.check().into_iter().filter(|i| i.text.contains("by themselves")).collect::<Vec<_>>();
        let t = tips(&n);
        assert!(!t.is_empty() && t.iter().all(|i| i.level == crate::net::Level::Tip), "{t:?}");
        // The phone watched the neighbour's router change channel twice.
        let json = r#"{"format":"netsketch-survey","device":"Pixel","points":[{"name":"Desk","time":"t0","x":0,"y":5,"networks":[{"ssid":"Nachbar","bssid":"cc:00:00:00:00:02","freq":2437,"channel":6,"rssi":-60}]}],
            "watch":{"start":"2026-11-05T20:00:00Z","end":"2026-11-05T23:00:00Z","samples":10800,"events":[
              {"time":"2026-11-05T21:00:00Z","kind":"channel","bssid":"cc:00:00:00:00:02","ssid":"Nachbar","from":"1","to":"6"},
              {"time":"2026-11-05T22:30:00Z","kind":"channel","bssid":"cc:00:00:00:00:02","ssid":"Nachbar","from":"6","to":"11"},
              {"time":"2026-11-05T22:31:00Z","kind":"drop","value":4.5}]},
            "findings":[{"level":"warning","text":"Your router changed channel twice tonight.","key":"jump"}],
            "lan":[{"ip":"192.168.1.1","name":"Internet-Box","kind":"router","vendor":"Swisscom","model":"Internet-Box 5"}]}"#;
        n.import_companion(json).unwrap();
        n.link_measured("Nachbar|cc:00:00:00:00", Some(nb));
        // Sent again by the live link: nothing counted twice.
        n.import_companion(&json.replace("\"t0\"", "\"t1\"")).unwrap();
        assert_eq!(n.channel_jumps(nb), 2);
        assert_eq!(n.phone.lan[0].model, "Internet-Box 5");
        let t = tips(&n);
        assert!(t.iter().any(|i| i.level == crate::net::Level::Warning && i.text.contains("2 channel changes")), "{t:?}");
        assert!(n.check().iter().any(|i| i.text.contains("From the phone: Your router changed channel twice")));
        n.apply_fix(&crate::issues::Fix::FixChannels);
        assert!(!n.dev(r).unwrap().auto_ch && tips(&n).is_empty());
    }

    #[test]
    fn everything_the_phone_app_writes_is_read() {
        let mut n = template("flat");
        n.import_companion(SAMPLE).unwrap();
        let s = n.points.iter().flat_map(|p| &p.networks).find(|s| s.bssid == "d4:5d:64:aa:bb:c4").unwrap();
        assert!(s.k && s.v && s.r && s.ftm && s.streams == Some(2) && s.country == "CH" && s.rssi_min == Some(-61.0) && s.rssi_max == Some(-56.0));
        assert!(n.points.iter().flat_map(|p| &p.networks).any(|s| s.wps && s.distance_m.is_none()));
        assert!(n.points.iter().flat_map(|p| &p.networks).any(|s| s.distance_m.is_some()), "an RTT distance");
        assert!(n.points.iter().any(|p| p.ping.as_ref().is_some_and(|g| g.avg_ms > 0.0)), "a router ping");
        assert!(n.points.iter().filter_map(|p| p.connected.as_ref()).any(|c| c.tx_mbps.is_some() && !c.standard.is_empty()));
        let w = n.phone.watch.as_ref().unwrap();
        for k in ["channel", "roam", "drop", "spike", "loss", "dip"] {
            assert!(w.events.iter().any(|e| e.kind == k), "{k}");
        }
        assert!(!n.phone.findings.is_empty() && n.phone.lan.iter().any(|d| d.model == "FRITZ!Box 7590"));
        assert!(n.check().iter().any(|i| i.text.starts_with("From the phone:")));
    }
}
