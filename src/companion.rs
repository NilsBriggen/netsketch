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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Connected {
    pub ssid: String,
    pub bssid: String,
    pub rssi: f32,
    pub link_mbps: f32,
    pub freq: u32,
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
        let n_points = f.points.len();
        // Positions must be real numbers on a plausible scale (within 10 km).
        if f.points.iter().any(|p| [p.x, p.y].iter().flatten().any(|v| !v.is_finite() || v.abs() > 10_000.0) || p.networks.iter().any(|s| !s.rssi.is_finite())) {
            return Err("the file has impossible positions or readings".into());
        }
        for (i, mut p) in f.points.into_iter().enumerate() {
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
            self.points.push(MeasurePoint { id: self.next_id, name: p.name, pos, floor: p.floor, placed, time: p.time, phone: f.device.clone(), connected: p.connected, speed: p.speed, networks: p.networks });
        }
        // Networks named like one of your access points belong to it.
        for m in self.measured_networks() {
            if m.dev.is_none() {
                if let Some(d) = self.own_radio_for(&m) {
                    self.measured.push((m.key.clone(), d));
                }
            }
        }
        Ok((n_points, nets))
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
            self.measured.push((key.to_string(), d));
        }
    }

    /// Adds a measured network as a neighbour with exactly the channels, widths and busy time that
    /// were measured, then fits where it is and how loud.
    pub fn add_measured_neighbour(&mut self, key: &str) -> Option<u32> {
        let m = self.measured_networks().into_iter().find(|m| m.key == key)?;
        let at = self.points.iter().find(|p| p.id == m.best_point)?.clone();
        let id = self.add_device(Kind::Neighbor, [at.pos[0] + 4.0 * PX_PER_M, at.pos[1]]);
        let d = self.dev_mut(id).unwrap();
        d.name = if m.ssid.is_empty() { "Hidden network".into() } else { m.ssid.clone() };
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
        self.measured.push((key.to_string(), id));
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
                    let score = err + 0.05 * k * k + if inside([x, y]) { 40.0 } else { 0.0 };
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
            Some(e) => format!("Fitted {done} access points to the measurements — the model is now off by {e:.1} dB on average."),
            None => "Place the measuring pins on the map first (drag them to where you stood).".into(),
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
}
