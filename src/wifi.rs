//! The air: walls, signal strength, Wi-Fi speed, channels, neighbours, extenders, roaming, hardware presets,
//! and the Thread/Zigbee (802.15.4) gadgets that share 2.4 GHz with Wi-Fi.
//! shortcut: a multi-wall path-loss model over 3-D distance (COST-231 style: near free-space distance loss plus each wall
//! and floor crossed), plus one bend around a wall's free end or outside corner (knife-edge diffraction: doorways,
//! hall corners). Not a ray tracer: reflections are only described, and paths with two bends aren't followed.
//! Material, fading and receiver values come from published measurements; each constant says which.
use crate::net::{Device, Kind, Link, Medium, Network, WifiStd, PX_PER_M};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum WallKind {
    Drywall,
    Wood,
    Glass,
    LowE,
    Brick,
    Concrete,
    Metal,
    Aerated,
    Stone,
    InnerBrick,
}

impl WallKind {
    pub const ALL: [WallKind; 10] = [WallKind::Drywall, WallKind::Wood, WallKind::Glass, WallKind::LowE, WallKind::Aerated, WallKind::InnerBrick, WallKind::Brick, WallKind::Concrete, WallKind::Stone, WallKind::Metal];
    pub fn label(self) -> &'static str {
        crate::i18n::tr(self.label_en())
    }
    fn label_en(self) -> &'static str {
        match self {
            WallKind::Drywall => "Drywall / plasterboard",
            WallKind::Wood => "Wood / door",
            WallKind::Glass => "Plain glass",
            WallKind::LowE => "Coated (low-E) window",
            WallKind::Brick => "Brick (outer wall)",
            WallKind::InnerBrick => "Brick (thin inner wall)",
            WallKind::Concrete => "Reinforced concrete",
            WallKind::Metal => "Metal / mirror / fridge",
            WallKind::Aerated => "Aerated concrete (Ytong)",
            WallKind::Stone => "Thick stone wall (old house)",
        }
    }
    /// Signal loss (dB) crossing one wall straight on, at 2.4, 5 and 6 GHz, for a home's usual thickness. From
    /// ITU-R P.2040-4, 3GPP TR 38.901, NIST IR 6055 and measurements (Shakya et al. 2024 at 6.75 GHz; Rodriguez
    /// Larrad et al. 2014 for coated glass). Light walls include the clutter fitted per-wall models carry (TGax 5 dB).
    pub fn loss(self) -> [f32; 3] {
        match self {
            WallKind::Drywall => [3.0, 4.0, 5.0],
            WallKind::Wood => [4.0, 5.0, 6.0],
            WallKind::Glass => [2.0, 3.0, 3.5],
            WallKind::LowE => [25.0, 30.0, 32.0],
            // shortcut: no solid 5–6 GHz measurement of aerated concrete exists (4.5 dB dry / 8.3 wet at 2.4 GHz
            // through one block); 30–36 cm plastered walls assumed.
            WallKind::Aerated => [6.0, 10.0, 12.0],
            WallKind::InnerBrick => [5.0, 7.0, 8.0],
            WallKind::Brick => [9.0, 13.0, 15.0],
            WallKind::Concrete => [15.0, 23.0, 27.0],
            // shortcut: no measurement of half-metre stone walls found; a conservative guess above 50 cm marble.
            WallKind::Stone => [18.0, 28.0, 32.0],
            WallKind::Metal => [30.0, 35.0, 37.0],
        }
    }
    pub fn note(self) -> &'static str {
        match self {
            WallKind::LowE => "Energy-saving windows carry a metal coating: they block Wi-Fi almost like a wall of metal.",
            WallKind::Metal => "Metal reflects: a dead zone behind it, and echoes (multipath) in front that cost speed.",
            WallKind::Concrete => "Reinforced concrete and floor slabs are the main reason one router can't cover a house.",
            WallKind::Brick => "Only about 1/20 of the 5 GHz signal gets through a brick outer wall (−13 dB), more if it's crossed at a slant.",
            WallKind::InnerBrick => "A thin brick inner wall (11 cm) costs about 7 dB at 5 GHz: a fifth of the signal gets through.",
            WallKind::Aerated => "Aerated concrete blocks are full of air: lighter on Wi-Fi than brick, but a thick outer wall still costs 10 dB at 5 GHz.",
            WallKind::Stone => "Old stone walls are often half a metre thick: barely anything gets through at 5 GHz. Plan an access point per side, or a cable through.",
            _ => "Light material: costs a little signal per wall.",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wall {
    pub id: u32,
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub kind: WallKind,
    #[serde(default)]
    pub floor: i8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Band {
    B24 = 0,
    B5 = 1,
    B6 = 2,
}

impl Band {
    pub const ALL: [Band; 3] = [Band::B24, Band::B5, Band::B6];
    pub fn label(self) -> &'static str {
        match self {
            Band::B24 => "2.4 GHz",
            Band::B5 => "5 GHz",
            Band::B6 => "6 GHz",
        }
    }
    pub fn i(self) -> usize {
        self as usize
    }
    /// Legal transmit power limit (EIRP, dBm) in the EU, for warnings (the lowest one on the band).
    pub fn eu_limit(self) -> f32 {
        match self {
            Band::B24 => 20.0,
            Band::B5 => 23.0,
            Band::B6 => 23.0,
        }
    }
    /// EU/CH limit (EIRP, dBm) on one channel: 5 GHz channels 100–140 allow 30 dBm (with power control).
    pub fn eirp_limit(self, ch: u16) -> f32 {
        if self == Band::B5 && ch >= 100 { 30.0 } else { self.eu_limit() }
    }
    pub fn widths(self) -> &'static [u16] {
        match self {
            Band::B24 => &[20, 40],
            Band::B5 => &[20, 40, 80, 160],
            Band::B6 => &[20, 40, 80, 160, 320],
        }
    }
    /// Valid primary channels for a channel width (aligned to the width's blocks), as allowed in the EU and
    /// Switzerland: 5 GHz 36–64 and 100–140 (144 and 149–165 aren't Wi-Fi channels here), 6 GHz 1–93 (5945–6425 MHz).
    pub fn channels(self, width: u16) -> Vec<u16> {
        match self {
            Band::B24 => (1..=13).collect(),
            Band::B5 => {
                let all: Vec<u16> = (36..=64).step_by(4).chain((100..=140).step_by(4)).collect();
                all.into_iter().filter(|c| range(self, *c, width).0 == 5000.0 + 5.0 * *c as f32 - 10.0 && width_fits(self, *c, width)).collect()
            }
            Band::B6 => (1..=93).step_by(4).filter(|c| range(self, *c, width).0 == 5950.0 + 5.0 * *c as f32 - 10.0 && width_fits(self, *c, width)).collect(),
        }
    }
}

/// The 5 GHz band has a gap; a wide channel must stay inside one sub-band (and inside the EU's 6 GHz part).
fn width_fits(band: Band, c: u16, w: u16) -> bool {
    let (lo, hi) = range(band, c, w);
    match band {
        Band::B5 => [(5170.0, 5330.0), (5490.0, 5710.0)].iter().any(|(a, b)| lo >= *a && hi <= *b),
        _ => hi <= 6425.0,
    }
}

/// Radar (DFS) channel: anything touching 5250–5710 MHz must listen before sending and leave when radar shows up.
pub fn dfs(band: Band, c: u16, w: u16) -> bool {
    let (lo, hi) = range(band, c, w.max(20));
    band == Band::B5 && hi > 5250.0 && lo < 5710.0
}

/// How long (s) a radio must listen for radar before it may send on a channel: 10 minutes on the
/// weather-radar channels 120–128 (5590–5650 MHz), a minute on other DFS channels.
pub fn cac_s(band: Band, c: u16, w: u16) -> f64 {
    let (lo, hi) = range(band, c, w.max(20));
    if !dfs(band, c, w) { 0.0 } else if hi > 5590.0 && lo < 5650.0 { 600.0 } else { 60.0 }
}

/// Frequency range (MHz) a radio occupies.
fn range(band: Band, c: u16, w: u16) -> (f32, f32) {
    match band {
        Band::B24 => {
            // OFDM channels are 20 MHz wide: 1/5/9/13 sit side by side, like 1/6/11.
            let f = 2407.0 + 5.0 * c as f32;
            if w >= 40 { (f - 10.0, f + 30.0) } else { (f - 10.0, f + 10.0) }
        }
        Band::B5 => {
            let lo = 5000.0 + 5.0 * c as f32 - 10.0;
            let base = if c >= 149 { 5735.0 } else { 5170.0 };
            let s = base + ((lo - base) / w as f32).floor() * w as f32;
            (s, s + w as f32)
        }
        Band::B6 => {
            let lo = 5950.0 + 5.0 * c as f32 - 10.0;
            let s = 5945.0 + ((lo - 5945.0) / w as f32).floor() * w as f32;
            (s, s + w as f32)
        }
    }
}

/// 0 = no conflict, 1 = same channel (take turns), 1.5 = partly overlapping 2.4 GHz channels (worse: noise).
fn overlap(band: Band, a: (u16, u16), b: (u16, u16)) -> f32 {
    let (ra, rb) = (range(band, a.0, a.1), range(band, b.0, b.1));
    if ra.0 >= rb.1 || rb.0 >= ra.1 {
        return 0.0;
    }
    if band == Band::B24 && a.0 != b.0 { 1.5 } else { 1.0 }
}

/// How a Wi-Fi link is doing right now.
#[derive(Clone, Copy, Debug)]
pub struct Signal {
    /// Router → device (dBm), as the device shows it (beacons, no beamforming).
    pub rssi: f32,
    /// Device → router (dBm): phones transmit weaker than routers.
    pub up: f32,
    pub band: Band,
    pub width: u16,
    pub streams: u8,
    pub mbps: f32,
    /// Device → router throughput (Mbit/s): its own weaker transmitter.
    pub up_mbps: f32,
    /// Raw radio (PHY) rate down, before protocol overhead and shared airtime.
    pub phy: f32,
    pub walls: usize,
    /// 1 = the channel is all yours; 2 = you get half the airtime.
    pub contention: f32,
    /// Wi-Fi 7 Multi-Link: the second band carrying data at the same time (its speed is included above).
    pub also: Option<Band>,
}

impl Signal {
    pub fn alive(&self) -> bool {
        self.mbps > 0.0
    }
    pub fn quality(&self) -> &'static str {
        crate::i18n::tr(match self.rssi {
            r if r >= -55.0 => "excellent",
            r if r >= -67.0 => "good",
            r if r >= -75.0 => "fair",
            r if r >= -82.0 => "poor",
            _ => "unusable",
        })
    }
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Do segments p1-p2 and q1-q2 cross?
pub fn crosses(p1: [f32; 2], p2: [f32; 2], q1: [f32; 2], q2: [f32; 2]) -> bool {
    let (d1, d2) = (cross(q1, q2, p1), cross(q1, q2, p2));
    let (d3, d4) = (cross(p1, p2, q1), cross(p1, p2, q2));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Weakest signal any Wi-Fi link works at: the slowest rate on a 20 MHz channel (beacons go out on 20 MHz).
pub fn floor(_band: Band) -> f32 {
    sens(20)
}

/// Weakest signal a channel width still works at: −90 dBm at 20 MHz (the slowest rate, MCS 0: the standard's
/// −82 dBm minimum less the ~8 dB a typical device does better), 3 dB more per doubling (more noise).
pub fn sens(width: u16) -> f32 {
    -90.0 + 3.0 * (width.max(20) as f32 / 20.0).log2()
}

/// Signal each rate needs on 80 MHz (dBm) and that rate as a share of Wi-Fi 6's top (1024-QAM, MCS 11): the
/// 802.11ac/ax minimum-sensitivity table (MCS 0–11; 12–13 for Wi-Fi 7) less ~8 dB for a typical device. In between,
/// rate adaptation mixes the neighbouring rates, hence the straight lines.
const MCS_80: [(f32, f32); 14] = [(-49.0, 1.2), (-51.0, 1.1), (-54.0, 1.0), (-56.0, 0.9), (-59.0, 0.8), (-61.0, 0.72), (-66.0, 0.6), (-67.0, 0.54), (-68.0, 0.48), (-72.0, 0.36), (-76.0, 0.24), (-79.0, 0.18), (-81.0, 0.12), (-84.0, 0.06)];

/// The top rate of a standard on a band as a share of Wi-Fi 6's (what `stream20` gives): 4096-QAM on Wi-Fi 7,
/// 256-QAM on Wi-Fi 5 (64-QAM on its 2.4 GHz side, which is Wi-Fi 4).
fn top_share(std: WifiStd, band: usize) -> f32 {
    match (std, band) {
        (WifiStd::Wifi7, _) => 1.2,
        (WifiStd::Wifi5, 0) => 0.6,
        (WifiStd::Wifi5, _) => 0.8,
        _ => 1.0,
    }
}

/// Share of the top rate left at a signal strength. Thresholds are for 80 MHz; wider channels
/// collect more noise (−3 dB per doubling), narrower ones less.
fn rate_fraction(rssi: f32, width: u16, std: WifiStd, band: usize) -> f32 {
    let eff = rssi - 3.0 * (width as f32 / 80.0).log2();
    let top = top_share(std, band);
    let share = if eff >= MCS_80[0].0 {
        MCS_80[0].1
    } else if eff < MCS_80[13].0 {
        0.0
    } else {
        MCS_80.windows(2).find(|w| eff <= w[0].0 && eff >= w[1].0).map_or(0.0, |w| w[1].1 + (w[0].1 - w[1].1) * (eff - w[1].0) / (w[0].0 - w[1].0))
    };
    share.min(top) / top
}

pub fn width_factor(w: u16) -> f32 {
    match w {
        0..=20 => 1.0,
        21..=40 => 2.0,
        41..=80 => 4.19,
        81..=160 => 8.38,
        _ => 16.76,
    }
}

/// 2.4 GHz channels that don't overlap: the classic 1/6/11 plan first, then the European 1/5/9/13 plan.
pub const CH24: [u16; 6] = [1, 6, 11, 5, 9, 13];

/// Height (m) above the floor of a spot on the Wi-Fi map: a phone in the hand, a laptop on a desk.
pub const SPOT_M: f32 = 1.0;

/// Spatial streams a link can really use at a signal: extra streams need a clean signal.
fn usable_streams(rssi: f32, n: u8) -> u8 {
    n.min(if rssi < -75.0 { 1 } else if rssi < -67.0 { 2 } else { 4 })
}

/// Share of the radio rate left as data after preambles, acknowledgements and gaps: small at low rates,
/// better at high ones where frames are packed together (aggregation).
pub fn mac_eff(phy: f32) -> f32 {
    (0.45 + 0.125 * (phy.max(1.0) / 10.0).log10()).clamp(0.45, 0.7)
}

/// Signal at which a client starts looking for a better access point. Each device has its own
/// (some are much stickier); 802.11k/v lets the access point suggest moving earlier.
pub fn roam_threshold(ap: &Device, cl: &Device) -> f32 {
    if ap.fast_roam { (cl.roam_at + 7).max(-70) as f32 } else { cl.roam_at as f32 }
}

fn band_loss(k: WallKind, band: Band) -> f32 {
    k.loss()[band.i()]
}

/// A floor slab's loss for the first storey crossed: P.1238 measured 20 dB at 5.2 GHz through a plain
/// reinforced-concrete floor, TGax and COST-231 use 18.3 dB; other materials as walls.
fn slab_loss(k: WallKind, band: Band) -> f32 {
    if k == WallKind::Concrete { [18.0, 22.0, 26.0][band.i()] } else { band_loss(k, band) }
}

/// A slanted crossing is longer inside the wall, but far less than 1/cos in air suggests: the wave bends
/// towards the wall's normal (Snell; ε′ ≈ 5 for brick and concrete), so at most ~1.12× (ITU-R P.2040 slab model).
fn slant_path(cos_in: f32) -> f32 {
    let sin2 = 1.0 - cos_in * cos_in;
    1.0 / (1.0 - 0.19 * sin2).sqrt()
}

/// The rest of the slant's cost is reflection off the wall's faces (dB): nothing below 45°, then +1 at 60°,
/// +2.5 at 70°, +5 at 80°, capped at +6 (fitted to P.2040's results for concrete and plasterboard).
fn slant_reflect(cos_in: f32) -> f32 {
    let deg = cos_in.clamp(0.0, 1.0).acos().to_degrees();
    let pts = [(45.0, 0.0), (60.0, 1.0), (70.0, 2.5), (80.0, 5.0), (90.0, 6.0)];
    if deg <= 45.0 {
        return 0.0;
    }
    pts.windows(2).find(|w| deg <= w[1].0).map_or(6.0, |w| w[0].1 + (w[1].1 - w[0].1) * (deg - w[0].0) / (w[1].0 - w[0].0))
}

/// Centre frequency (MHz) the path loss is worked out at.
fn mhz(band: Band) -> f32 {
    match band {
        Band::B24 => 2437.0,
        Band::B5 => 5500.0,
        Band::B6 => 6100.0,
    }
}

/// Distance loss (dB) over `d` metres indoors: free space with a slightly steeper slope (furniture, people).
fn dist_loss(band: Band, d: f32) -> f32 {
    20.0 * mhz(band).log10() + 22.0 * d.max(1.0).log10() - 28.0
}

/// Extra loss (dB) bending around an edge (knife-edge diffraction, ITU-R P.526), from the Fresnel parameter ν.
fn knife_edge(v: f32) -> f32 {
    if v <= -0.78 { 0.0 } else { 6.9 + 20.0 * (((v - 0.1).powi(2) + 1.0).sqrt() + v - 0.1).log10() }
}

// How signals vary over time on a link that stays put (see `Network::fading`).
/// Slow change over time (people walking through or standing in the way, doors), sd in dB, with and without a
/// line of sight. ITU-R P.1238 §8: a person in a line-of-sight path costs 6–8 dB; behind walls people barely matter.
pub const SHADOW_SD_LOS: f32 = 3.0;
pub const SHADOW_SD_NLOS: f32 = 1.5;
/// A device that's carried around: shadowing by position (TGn models B–D after the breakpoint: 4–5 dB).
pub const SHADOW_SD_CARRIED: f32 = 4.0;
/// How long such a change takes (s).
pub const SHADOW_S: f64 = 5.0;
/// Rician K-factor (linear) of a line-of-sight link: 0 dB, as in the TGn residential models B and C.
pub const LOS_K: f32 = 1.0;
/// Width (MHz) over which a home's echoes fade together (coherence bandwidth ≈ 1/(5 × 15–30 ns delay spread)).
pub const COHERENCE_MHZ: f32 = 10.0;
/// Speed of what moves in a home (m/s, 1.2 km/h as in TGn), which sets how fast multipath fading changes.
pub const MOVE_MPS: f64 = 0.33;
/// Window rate adaptation and the live view average over (s).
pub const HALF_S: f64 = 0.5;
/// How long a device keeps trying before it gives up on the access point (s: Linux waits 7 beacons, then probes:
/// 1–3 s), and how long joining again takes (s: scan, authentication, keys, address; 1.5 s typical, 5 s worst).
pub const BEACON_LOSS_S: f64 = 2.0;
pub const REJOIN_S: f64 = 1.5;

/// How steady a Wi-Fi link is over time.
#[derive(Clone, Copy, Debug)]
pub struct Stability {
    /// dB the average signal sits above the edge where the link gives up.
    pub margin: f32,
    /// Share of frames sent again, and share lost even after all retries.
    pub retry: f32,
    pub loss: f32,
    /// How often an hour the device loses the access point and must join again, and the share of time it's off.
    pub drops_h: f32,
    pub down: f32,
}

/// Standard normal distribution function.
fn phi(z: f32) -> f32 {
    // Abramowitz & Stegun 7.1.26 (error < 1.5e-7).
    let x = z.abs() / std::f32::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.3275911 * x);
    let erf = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    if z >= 0.0 { 0.5 * (1.0 + erf) } else { 0.5 * (1.0 - erf) }
}

/// Fading power (mean 1) of a Nakagami-m channel — Gamma(m, 1/m) — at the standard-normal value z
/// (Wilson–Hilferty: accurate to a few % for m ≥ 1).
pub fn gamma_quantile(m: f32, z: f32) -> f32 {
    let c = 1.0 / (9.0 * m.max(0.5));
    ((1.0 - c) + c.sqrt() * z).max(0.05).powi(3)
}

/// Chance a Nakagami-m fading power (mean 1) is below x.
pub fn gamma_below(m: f32, x: f32) -> f32 {
    let c = 1.0 / (9.0 * m.max(0.5));
    phi((x.max(1e-9).cbrt() - (1.0 - c)) / c.sqrt())
}

/// Diffraction behind a corner is capped: walls next to it reflect signal into the shadow, so pure knife-edge
/// (27–37 dB 1–4 m into the shadow at 5 GHz) is far too pessimistic. ITU-R P.1411 uses 20 dB for a street corner.
/// shortcut: a cap instead of computing those reflections; no measured indoor value exists.
const BEND_CAP_DB: f32 = 20.0;

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Where a signal can bend around a wall: the corner itself, a spot 0.3 m out into the open side, and the storey.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub at: [f32; 2],
    pub open: [f32; 2],
    pub floor: i8,
}

/// Spots where a signal can bend around a wall: free ends (door frames, openings) and outside corners.
/// A point where walls meet straight on, or in a T, is no edge.
pub fn wall_edges(walls: &[Wall]) -> Vec<Edge> {
    let near = 0.2 * PX_PER_M;
    let mut seen: Vec<([f32; 2], i8)> = vec![];
    let mut out = vec![];
    for w in walls {
        for end in [w.a, w.b] {
            if seen.iter().any(|s| s.1 == w.floor && dist(s.0, end) < near) {
                continue;
            }
            seen.push((end, w.floor));
            let mut ang: Vec<f32> = vec![];
            for o in walls.iter().filter(|o| o.floor == w.floor) {
                let toward = |p: [f32; 2]| (p[1] - end[1]).atan2(p[0] - end[0]);
                if dist(o.a, end) < near {
                    ang.push(toward(o.b));
                } else if dist(o.b, end) < near {
                    ang.push(toward(o.a));
                } else if seg_dist(end, o.a, o.b) < near {
                    ang.push(toward(o.a));
                    ang.push(toward(o.b));
                }
            }
            ang.sort_by(f32::total_cmp);
            let tau = std::f32::consts::TAU;
            let (mut gap, mut mid) = (0.0, 0.0);
            for (i, a) in ang.iter().enumerate() {
                let next = if i + 1 < ang.len() { ang[i + 1] } else { ang[0] + tau };
                if next - a > gap {
                    gap = next - a;
                    mid = a + gap / 2.0;
                }
            }
            if gap > 200f32.to_radians() {
                out.push(Edge { at: end, open: [end[0] + 0.3 * PX_PER_M * mid.cos(), end[1] + 0.3 * PX_PER_M * mid.sin()], floor: w.floor });
            }
        }
    }
    out
}

/// Results the UI asks for many times in one frame (every link's signal, airtime, steadiness, spanning tree).
/// Only used while `on`: the app turns it on and empties it at the start of every frame and after every edit,
/// so model code and tests always compute afresh.
#[derive(Default, Debug)]
pub struct Memo {
    pub on: bool,
    signal: std::collections::HashMap<(u32, bool, bool), Option<Signal>>,
    airtime: std::collections::HashMap<(u32, usize), f32>,
    stability: std::collections::HashMap<u32, Option<Stability>>,
    pub(crate) stp: Option<crate::stp::Stp>,
}

/// A copy of a plan (undo steps, tests) starts with an empty memo, switched off.
impl Clone for Memo {
    fn clone(&self) -> Self {
        Memo::default()
    }
}

impl Memo {
    /// Forgets everything (and turns the memo on or off).
    pub fn reset(&mut self, on: bool) {
        *self = Memo { on, ..Default::default() };
    }

    /// Forgets what depends on the air's busy time (the traffic sharing changes it between its two passes).
    pub fn forget_air(&mut self) {
        self.signal.clear();
        self.airtime.clear();
        self.stability.clear();
    }
}

/// Path losses already worked out, valid while the geometry stamp stays the same.
#[derive(Default, Debug)]
pub struct PathCache {
    stamp: u64,
    map: std::collections::HashMap<(u32, u32, u32, u32, u32, i8, i8), [f32; 3]>,
}

/// A copy of a plan (each undo step is one) starts with an empty cache instead of copying megabytes.
impl Clone for PathCache {
    fn clone(&self) -> Self {
        PathCache::default()
    }
}

/// The wall edges, worked out again only when the walls change.
#[derive(Clone, Default, Debug)]
pub struct EdgeCache {
    stamp: u64,
    edges: Vec<Edge>,
    /// Wall loss from a spot (a transmitter, usually) to each edge: the first half of every detour from there.
    legs: std::collections::HashMap<(u32, u32, i8), std::rc::Rc<Vec<[f32; 3]>>>,
}

/// A cheap fingerprint of the walls (FNV-1a over their numbers): checked on every path, so it must be fast.
fn walls_stamp(walls: &[Wall]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| h = (h ^ x).wrapping_mul(0x0100_0000_01b3);
    eat(walls.len() as u64);
    for w in walls {
        for v in [w.a[0], w.a[1], w.b[0], w.b[1]] {
            eat(v.to_bits() as u64);
        }
        eat(((w.floor as u8 as u64) << 8) | w.kind as u64);
    }
    h
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (abx, aby) = (b[0] - a[0], b[1] - a[1]);
    let t = (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / (abx * abx + aby * aby).max(1e-6)).clamp(0.0, 1.0);
    ((a[0] + abx * t - p[0]).powi(2) + (a[1] + aby * t - p[1]).powi(2)).sqrt()
}

/// A piece of Wi-Fi hardware: what the radio can do.
pub struct Preset {
    pub name: &'static str,
    pub std: WifiStd,
    pub streams: u8,
    pub bands: [bool; 3],
    pub width: [u16; 3],
    pub tx: [f32; 3],
    pub backhaul: bool,
    pub price: f32,
    /// Antenna beam width (degrees, 0 = all around).
    pub beam: f32,
}

impl Preset {
    pub fn apply(&self, d: &mut Device) {
        d.beam = self.beam;
        d.model = self.name.into();
        d.wifi = self.std;
        d.streams = self.streams;
        d.bands = self.bands;
        d.width = self.width;
        d.tx = self.tx;
        d.backhaul = self.backhaul;
        if self.price > 0.0 {
            d.price = self.price;
        }
        normalize_radio(d);
    }
}

/// Keeps a radio's settings sensible after any change: bands the standard can't use go off, and a band
/// that's on always gets a real width, power and channel (presets leave unused bands at zero).
pub fn normalize_radio(d: &mut Device) {
    if d.kind == Kind::Interference || !(d.kind.transmits() || d.kind.wifi_client()) {
        return;
    }
    let client = !d.kind.transmits();
    d.streams = d.streams.clamp(1, 4);
    for b in Band::ALL {
        let i = b.i();
        let max = d.wifi.max_width(i);
        if max == 0 {
            d.bands[i] = false;
            continue;
        }
        if d.width[i] == 0 || d.width[i] > max {
            // Access points: the usual widths (20 / 80 / 160 MHz); devices: as wide as their standard allows.
            let usual = match b {
                Band::B24 => 20,
                Band::B5 => if client { 160 } else { 80 },
                Band::B6 => 160,
            };
            d.width[i] = if d.width[i] > max { max } else { usual.min(max) };
        }
        if d.tx[i] <= 0.0 {
            // Typical transmit power: access points at the EU limit, phones and gadgets well below.
            let base = if !client {
                b.eu_limit()
            } else {
                match d.kind {
                    Kind::Phone => 15.0,
                    Kind::SmartHome | Kind::Camera => 12.0,
                    _ => 17.0,
                }
            };
            d.tx[i] = if client && b == Band::B6 { base - 2.0 } else { base };
        }
        let chans = b.channels(d.width[i]);
        if !chans.contains(&d.ch[i]) {
            d.ch[i] = chans.first().copied().unwrap_or(d.ch[i]);
        }
        // Hardware sold in Europe won't send above the channel's legal limit.
        d.tx[i] = d.tx[i].min(b.eirp_limit(d.ch[i]));
    }
}

const fn p(name: &'static str, std: WifiStd, streams: u8, bands: [bool; 3], width: [u16; 3], tx: [f32; 3], backhaul: bool, price: f32) -> Preset {
    Preset { name, std, streams, bands, width, tx, backhaul, price, beam: 0.0 }
}

/// Extra signal (dB) a directional antenna gives towards `to`: strong inside the beam, weak outside.
pub fn dir_gain(d: &Device, to: [f32; 2]) -> f32 {
    if d.beam <= 0.0 {
        return 0.0;
    }
    let ang = (to[1] - d.pos[1]).atan2(to[0] - d.pos[0]).to_degrees();
    let off = ((ang - d.aim + 540.0).rem_euclid(360.0) - 180.0).abs();
    // A beam `beam`° wide in both directions covers beam²/41253 of the sphere: that's its gain (dBi).
    let peak = (10.0 * (41253.0 / (d.beam * d.beam)).log10()).min(30.0);
    if off <= d.beam / 2.0 { peak } else if off <= d.beam { peak - 12.0 } else { -12.0 }
}

use WifiStd::*;
pub const RADIO_PRESETS: &[Preset] = &[
    p("ISP basic router — Wi-Fi 5, 2×2", Wifi5, 2, [true, true, false], [20, 80, 0], [18.0, 20.0, 0.0], false, 0.0),
    p("Wi-Fi 6 dual-band — 4×4", Wifi6, 4, [true, true, false], [20, 80, 0], [20.0, 23.0, 0.0], false, 160.0),
    p("Wi-Fi 6E tri-band — 4×4", Wifi6E, 4, [true, true, true], [20, 80, 160], [20.0, 23.0, 23.0], false, 280.0),
    p("Wi-Fi 7 flagship — 4×4, 320 MHz", Wifi7, 4, [true, true, true], [40, 160, 320], [20.0, 23.0, 23.0], false, 450.0),
    p("Enterprise ceiling AP — Wi-Fi 6E, PoE", Wifi6E, 4, [true, true, true], [20, 40, 80], [14.0, 17.0, 17.0], false, 600.0),
    p("Mesh node — tri-band, own backhaul", Wifi6, 2, [true, true, false], [20, 80, 0], [20.0, 23.0, 0.0], true, 200.0),
    p("Cheap plug-in extender — Wi-Fi 5", Wifi5, 2, [true, true, false], [20, 80, 0], [17.0, 20.0, 0.0], false, 40.0),
    p("Cross-band extender — Wi-Fi 6", Wifi6, 2, [true, true, false], [20, 80, 0], [20.0, 23.0, 0.0], true, 90.0),
    p("Multi-room speaker — 2.4 GHz mesh radio, 2×2", Wifi5, 2, [true, false, false], [20, 0, 0], [17.0, 0.0, 0.0], false, 0.0),
    Preset { name: "Outdoor point-to-point bridge — 5 GHz, 15° dish", std: Wifi6, streams: 2, bands: [false, true, false], width: [0, 80, 0], tx: [0.0, 20.0, 0.0], backhaul: true, price: 180.0, beam: 15.0 },
];
pub const CLIENT_PRESETS: &[Preset] = &[
    p("Phone — Wi-Fi 6E, 2×2", Wifi6E, 2, [true, true, true], [20, 160, 160], [15.0, 15.0, 13.0], false, 0.0),
    p("Older phone — Wi-Fi 5, 1×1", Wifi5, 1, [true, true, false], [20, 80, 0], [14.0, 14.0, 0.0], false, 0.0),
    p("Laptop — Wi-Fi 7, 2×2", Wifi7, 2, [true, true, true], [40, 160, 320], [18.0, 18.0, 16.0], false, 0.0),
    p("Laptop — Wi-Fi 6, 2×2", Wifi6, 2, [true, true, false], [20, 160, 0], [17.0, 17.0, 0.0], false, 0.0),
    p("TV / streaming box — Wi-Fi 5, 2×2", Wifi5, 2, [true, true, false], [20, 80, 0], [17.0, 17.0, 0.0], false, 0.0),
    p("Smart-home gadget — 2.4 GHz only, 1×1", Wifi5, 1, [true, false, false], [20, 0, 0], [12.0, 0.0, 0.0], false, 0.0),
    p("USB Wi-Fi stick — Wi-Fi 6, 2×2", Wifi6, 2, [true, true, false], [20, 80, 0], [16.0, 16.0, 0.0], false, 0.0),
];

/// Household things that send on 2.4 GHz without being Wi-Fi: (name, channel, width, share of time, power dBm).
pub const DISTURBERS: &[(&str, u16, u16, f32, f32)] = &[
    // A magnetron oven: ≈ +14 dBm (−50 dBm at 15 m, Nassar et al. 2011), on for half of each mains cycle,
    // centred near 2.455 GHz and sweeping over channels 6–13 (worst 8–11).
    ("Microwave oven (while cooking)", 9, 40, 0.5, 14.0),
    ("Analog baby monitor / video sender", 6, 20, 0.9, 3.0),
    ("Bluetooth speaker or headphones", 6, 40, 0.06, 0.0),
    ("Zigbee / Thread smart-home hub", 11, 20, 0.04, 5.0),
];

pub fn apply_disturber(d: &mut Device, i: usize) {
    let (name, ch, w, busy, tx) = DISTURBERS[i];
    d.model = name.into();
    d.bands = [true, false, false];
    d.ch[0] = ch;
    d.width[0] = w;
    d.busy = busy;
    d.tx[0] = tx;
    d.streams = 1;
}

pub fn default_preset(k: Kind) -> Option<&'static Preset> {
    let r = |i: usize| Some(&RADIO_PRESETS[i]);
    let c = |i: usize| Some(&CLIENT_PRESETS[i]);
    match k {
        Kind::HomeRouter => r(1),
        Kind::AccessPoint => r(2),
        Kind::Extender => r(6),
        Kind::Neighbor => r(0),
        Kind::Speaker => r(8),
        Kind::Phone => c(0),
        Kind::Laptop => c(3),
        Kind::Tv | Kind::Console => c(4),
        Kind::SmartHome | Kind::Camera => c(5),
        Kind::Desktop | Kind::Printer => c(6),
        _ => None,
    }
}

impl Network {
    /// Walls crossed between two points (only walls on the storeys of either end count): loss per band, and how
    /// many. A wall crossed at a slant costs more (see `slant_path`, `slant_reflect`).
    pub fn wall_losses(&self, a: [f32; 2], fa: i8, b: [f32; 2], fb: i8) -> ([f32; 3], usize) {
        let mut loss = [0.0; 3];
        let mut n = 0;
        let mut seen: Vec<WallKind> = vec![];
        let p = [b[0] - a[0], b[1] - a[1]];
        let (lo, hi) = ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]);
        for w in self.walls.iter().filter(|w| w.floor == fa || w.floor == fb) {
            // Quick reject: the wall's box doesn't touch the path's box.
            if w.a[0].max(w.b[0]) < lo[0] || w.a[0].min(w.b[0]) > hi[0] || w.a[1].max(w.b[1]) < lo[1] || w.a[1].min(w.b[1]) > hi[1] {
                continue;
            }
            if crosses(a, b, w.a, w.b) {
                let v = [w.b[0] - w.a[0], w.b[1] - w.a[1]];
                // |sin| of the angle between path and wall = cos of the angle to the wall's normal.
                let cos = ((p[0] * v[1] - p[1] * v[0]).abs() / ((p[0].hypot(p[1]) * v[0].hypot(v[1])).max(1e-6))).min(1.0);
                // A second, third… wall of the same kind costs less (Lott & Forkel: 16 → 14 dB, 29 → 24 dB).
                let again = if seen.contains(&w.kind) { 0.85 } else { 1.0 };
                seen.push(w.kind);
                for (l, k) in loss.iter_mut().zip(w.kind.loss()) {
                    *l += (k * slant_path(cos) + slant_reflect(cos)) * again;
                }
                n += 1;
            }
        }
        (loss, n)
    }

    pub fn wall_loss(&self, a: [f32; 2], fa: i8, b: [f32; 2], fb: i8, band: Band) -> (f32, usize) {
        let (l, n) = self.wall_losses(a, fa, b, fb);
        (l[band.i()], n)
    }

    fn edges(&self) -> std::cell::Ref<'_, Vec<Edge>> {
        let stamp = walls_stamp(&self.walls);
        if self.edge_cache.borrow().stamp != stamp || (stamp == 0 && !self.walls.is_empty()) {
            *self.edge_cache.borrow_mut() = EdgeCache { stamp, edges: wall_edges(&self.walls), legs: Default::default() };
        }
        std::cell::Ref::map(self.edge_cache.borrow(), |c| &c.edges)
    }

    /// Wall losses from `a` to every edge (in `edges()` order), worked out once per spot: a transmitter stays put
    /// while thousands of spots around it are worked out.
    fn legs_from(&self, a: [f32; 2], fa: i8) -> std::rc::Rc<Vec<[f32; 3]>> {
        // Brings the edges (and with them this cache) up to date with the walls first.
        drop(self.edges());
        let key = (a[0].to_bits(), a[1].to_bits(), fa);
        if let Some(v) = self.edge_cache.borrow().legs.get(&key) {
            return v.clone();
        }
        let v: std::rc::Rc<Vec<[f32; 3]>> = std::rc::Rc::new(self.edge_cache.borrow().edges.iter().map(|e| if e.floor == fa { self.wall_losses(a, fa, e.open, fa).0 } else { [0.0; 3] }).collect());
        let mut c = self.edge_cache.borrow_mut();
        if c.legs.len() > 5000 {
            c.legs.clear();
        }
        c.legs.insert(key, v.clone());
        v
    }

    /// Path loss (dB) between two spots, multi-wall style: near free-space loss over the real 3-D distance
    /// (storey heights, plus `dh` = how much higher b's antenna sits above its floor than a's), plus every
    /// wall crossed, plus the floor slabs (each further slab costs a little less).
    pub fn path_loss(&self, a: [f32; 2], fa: i8, b: [f32; 2], fb: i8, dh: f32, band: Band) -> f32 {
        self.path_losses(a, fa, b, fb, dh)[band.i()]
    }

    /// `path_loss` for all three bands at once (the geometry is the same). When walls block the straight way,
    /// the signal also bends around the best nearby wall end or corner (a doorway, the corner of a hall);
    /// both ways add up as power.
    pub fn path_losses(&self, a: [f32; 2], fa: i8, b: [f32; 2], fb: i8, dh: f32) -> [f32; 3] {
        // The same pairs come up again and again (every link, every frame): remember them until the walls change.
        let stamp = self.geometry_stamp();
        let key = (a[0].to_bits(), a[1].to_bits(), b[0].to_bits(), b[1].to_bits(), dh.to_bits(), fa, fb);
        {
            let c = self.path_cache.borrow();
            if c.stamp == stamp {
                if let Some(v) = c.map.get(&key) {
                    return *v;
                }
            }
        }
        let v = self.path_losses_raw(a, fa, b, fb, dh);
        let mut c = self.path_cache.borrow_mut();
        if c.stamp != stamp || c.map.len() > 200_000 {
            c.map.clear();
            c.stamp = stamp;
        }
        c.map.insert(key, v);
        v
    }

    /// Everything path loss depends on besides the two ends: walls, the floor slabs and storey heights.
    fn geometry_stamp(&self) -> u64 {
        let h = self.storey_heights.iter().fold(walls_stamp(&self.walls), |h, (f, m)| crate::sim::hash(h, *f as u64, m.to_bits() as u64));
        crate::sim::hash(h, self.slab as u64, 0x5_1AB)
    }

    /// `path_losses` without the cache: for the Wi-Fi map, whose thousands of spots would only crowd it out.
    pub fn path_losses_raw(&self, a: [f32; 2], fa: i8, b: [f32; 2], fb: i8, dh: f32) -> [f32; 3] {
        let storeys = (fa as f32 - fb as f32).abs();
        let flat = dist(a, b) / PX_PER_M;
        let up = self.floor_z(fb) - self.floor_z(fa) + dh;
        let d = (flat * flat + up * up).sqrt().max(1.0);
        let (walls, _) = self.wall_losses(a, fa, b, fb);
        let mut out = [0.0; 3];
        for band in Band::ALL {
            // COST-231 multi-wall: k slabs cost Lf·k^((k+2)/(k+1) − 0.46).
            let slabs = if storeys > 0.0 { slab_loss(self.slab, band) * storeys.powf((storeys + 2.0) / (storeys + 1.0) - 0.46) } else { 0.0 };
            out[band.i()] = dist_loss(band, d) + walls[band.i()] + slabs;
        }
        if fa != fb {
            return out;
        }
        let ab = [b[0] - a[0], b[1] - a[1]];
        let ab_len = ab[0].hypot(ab[1]).max(1e-6);
        // How far a point sits off the straight line (m), and where along it (0 = a, 1 = b).
        let off = |p: [f32; 2]| ((p[0] - a[0]) * ab[1] - (p[1] - a[1]) * ab[0]).abs() / ab_len / PX_PER_M;
        let along = |p: [f32; 2]| ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / (ab_len * ab_len);
        // Fresnel parameter of an edge h metres off the way, d1 and d2 metres from the ends.
        let nu = |h: f32, d1: f32, d2: f32, band: Band| h * (2.0 * (d1 + d2) / (300.0 / mhz(band) * d1 * d2)).sqrt();
        let worst = walls.iter().fold(0.0f32, |m, x| m.max(*x));
        if worst == 0.0 {
            // A clear way that passes close to a wall's end still loses some signal (up to 6 dB right at the edge):
            // the edge cuts into the zone the radio wave fills around the straight line.
            let mut lit = [0.0f32; 3];
            for e in self.edges().iter().filter(|e| e.floor == fa) {
                let t = along(e.at);
                if !(0.02..0.98).contains(&t) {
                    continue;
                }
                let (d1, d2) = ((dist(a, e.at) / PX_PER_M).max(0.5), (dist(e.at, b) / PX_PER_M).max(0.5));
                let h = off(e.at);
                // 2.4 GHz has the widest zone: if the edge is clear of it there, it's clear on every band.
                if nu(h, d1, d2, Band::B24) >= 0.78 {
                    continue;
                }
                for band in Band::ALL {
                    lit[band.i()] = lit[band.i()].max(knife_edge(-nu(h, d1, d2, band)));
                }
            }
            for i in 0..3 {
                out[i] += lit[i];
            }
            return out;
        }
        if worst < 6.0 {
            return out;
        }
        // Each corner's detour costs its distance, its bend, the walls from here to the corner (known once per
        // transmitter) and the walls from the corner on. Try the cheapest first and stop once no corner left can beat
        // the best detour found: the same result with far less work.
        let first = self.legs_from(a, fa);
        let edges = self.edges();
        let mut cands: Vec<([f32; 3], [f32; 2])> = vec![];
        for (e, leg) in edges.iter().zip(first.iter()).filter(|(e, _)| e.floor == fa) {
            let (d1, d2) = ((dist(a, e.open) / PX_PER_M).max(0.5), (dist(e.open, b) / PX_PER_M).max(0.5));
            let len = ((d1 + d2).powi(2) + up * up).sqrt().max(1.0);
            // Even grazing an edge costs 6 dB: skip detours that can't beat the straight way.
            if 22.0 * (len / d).log10() + 6.0 >= worst {
                continue;
            }
            // The deeper the shadow (how far the corner sits off the straight line), the more bending costs.
            let h = off(e.at);
            let mut lb = [0.0; 3];
            for band in Band::ALL {
                lb[band.i()] = dist_loss(band, len) + knife_edge(nu(h, d1, d2, band)).min(BEND_CAP_DB) + leg[band.i()];
            }
            cands.push((lb, e.open));
        }
        cands.sort_by(|x, y| x.0[1].total_cmp(&y.0[1]));
        let mut bent = [f32::INFINITY; 3];
        for (lb, open) in cands {
            if (0..3).all(|i| lb[i] >= bent[i]) {
                continue;
            }
            let (rest, _) = self.wall_losses(open, fa, b, fa);
            for i in 0..3 {
                bent[i] = bent[i].min(lb[i] + rest[i]);
            }
        }
        for i in 0..3 {
            if bent[i].is_finite() {
                out[i] = -10.0 * (10f32.powf(-out[i] / 10.0) + 10f32.powf(-bent[i] / 10.0)).log10();
            }
        }
        out
    }

    /// Power (dBm EIRP) a radio sends towards a spot: a dish's gain counts, but never above the legal limit.
    fn eirp(&self, t: &Device, to: [f32; 2], band: Band) -> f32 {
        let p = t.tx[band.i()] + dir_gain(t, to);
        if t.beam > 0.0 { p.min(band.eirp_limit(self.chan(t, band).0)) } else { p }
    }

    /// Average signal (dBm) from `ap` received at a spot: transmit power, antenna gain, distance, walls, floors.
    pub fn rssi_at(&self, ap: &Device, pos: [f32; 2], floor: i8, band: Band) -> f32 {
        if !ap.bands[band.i()] {
            return -120.0;
        }
        self.rssi_from(ap, pos, band, self.path_loss(ap.pos, ap.floor, pos, floor, SPOT_M - ap.mount, band))
    }

    fn rssi_from(&self, ap: &Device, pos: [f32; 2], band: Band, loss: f32) -> f32 {
        let gain = if ap.streams >= 4 { 2.0 } else { 0.0 };
        self.eirp(ap, pos, band) + gain + ap.cal_db - loss
    }

    /// Best signal over the bands a radio has on (2.4 GHz counted 6 dB lower: it's the slow band).
    pub fn best_rssi(&self, ap: &Device, pos: [f32; 2], floor: i8) -> f32 {
        if !ap.bands.iter().any(|b| *b) {
            return -120.0;
        }
        self.best_of(ap, pos, self.path_losses(ap.pos, ap.floor, pos, floor, SPOT_M - ap.mount))
    }

    /// `best_rssi` without the path-loss cache, for maps and searches over thousands of one-off spots.
    pub fn best_rssi_raw(&self, ap: &Device, pos: [f32; 2], floor: i8) -> f32 {
        if !ap.bands.iter().any(|b| *b) {
            return -120.0;
        }
        self.best_of(ap, pos, self.path_losses_raw(ap.pos, ap.floor, pos, floor, SPOT_M - ap.mount))
    }

    /// `rssi_at` without the path-loss cache.
    pub fn rssi_at_raw(&self, ap: &Device, pos: [f32; 2], floor: i8, band: Band) -> f32 {
        if !ap.bands[band.i()] {
            return -120.0;
        }
        self.rssi_from(ap, pos, band, self.path_losses_raw(ap.pos, ap.floor, pos, floor, SPOT_M - ap.mount)[band.i()])
    }

    /// The strongest signal any of `radios` gives at a spot, on one band or (None) the best band: what the Wi-Fi
    /// map and the report draw. A radio is only worked out in full while it could still beat the best so far: with
    /// no walls at all it would be at most free-space distance loss less 3 dB (a bent path adds at most that).
    /// Radios that couldn't reach `at_least` dBm even with no walls are skipped (the map draws nothing that weak).
    pub fn strongest_at(&self, radios: &[&Device], pos: [f32; 2], floor: i8, band: Option<Band>, at_least: f32) -> f32 {
        let ceiling = |r: &Device| -> f32 {
            let flat = dist(r.pos, pos) / PX_PER_M;
            let up = self.floor_z(floor) - self.floor_z(r.floor) + SPOT_M - r.mount;
            let d = (flat * flat + up * up).sqrt().max(1.0);
            let bands: Vec<Band> = match band {
                Some(b) => vec![b],
                None => Band::ALL.to_vec(),
            };
            bands.iter().filter(|b| r.bands[b.i()]).map(|b| self.rssi_from(r, pos, *b, dist_loss(*b, d) - 3.02) - if band.is_none() && *b == Band::B24 { 6.0 } else { 0.0 }).fold(-120.0, f32::max)
        };
        let mut cands: Vec<(f32, &Device)> = radios.iter().map(|r| (ceiling(r), *r)).collect();
        cands.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut best = -120.0f32;
        for (top, r) in cands {
            if top <= best || top < at_least {
                break;
            }
            best = best.max(match band {
                Some(b) => self.rssi_at_raw(r, pos, floor, b),
                None => self.best_rssi_raw(r, pos, floor),
            });
        }
        best
    }

    fn best_of(&self, ap: &Device, pos: [f32; 2], loss: [f32; 3]) -> f32 {
        Band::ALL.iter().filter(|b| ap.bands[b.i()]).map(|b| self.rssi_from(ap, pos, *b, loss[b.i()]) - if *b == Band::B24 { 6.0 } else { 0.0 }).fold(-120.0, f32::max)
    }

    /// Reflecting surfaces (metal, coated glass, plain glass) within 4 m of a spot: they cause multipath fading.
    pub fn reflectors(&self, pos: [f32; 2], floor: i8) -> usize {
        self.walls
            .iter()
            .filter(|w| w.floor == floor && matches!(w.kind, WallKind::Metal | WallKind::LowE | WallKind::Glass))
            .filter(|w| seg_dist(pos, w.a, w.b) < 4.0 * PX_PER_M)
            .count()
    }

    /// How a link's signal varies over time: (slow shadowing sd in dB, fast-fading Nakagami m per frame,
    /// coherence time of the fast fading in s). See `fading` constants.
    pub fn fading(&self, ap: &Device, cl: &Device, band: Band, width: u16) -> (f32, f32, f64) {
        let los = ap.floor == cl.floor && self.wall_loss(ap.pos, ap.floor, cl.pos, cl.floor, band).1 == 0;
        let refl = (self.reflectors(cl.pos, cl.floor) + self.reflectors(ap.pos, ap.floor)).min(4) as f32;
        // Rician K: a line of sight gives a steady direct ray; reflecting surfaces around add echoes that weaken it.
        let k = if los { LOS_K / (1.0 + 0.5 * refl) } else { 0.0 };
        let m_k = (k + 1.0).powi(2) / (2.0 * k + 1.0);
        // Diversity: every receive antenna, and every slice of a wide channel that fades on its own, averages fades out.
        let antennas = ap.streams.min(cl.streams).clamp(1, 4) as f32;
        let slices = (width.max(20) as f32 / COHERENCE_MHZ).clamp(1.0, 16.0);
        let m = (m_k * antennas * slices).min(64.0);
        // Phones and laptops get carried around: their signal also changes with where they are (TGn/TGax: 4–5 dB).
        let carried = matches!(cl.kind, Kind::Phone | Kind::Laptop);
        let sd = if carried { SHADOW_SD_CARRIED } else if los { SHADOW_SD_LOS } else { SHADOW_SD_NLOS };
        let lambda = 300.0 / mhz(band) as f64;
        (sd, m, 0.423 * lambda / MOVE_MPS)
    }

    /// Live signal variation (dB) on a link: slow shadowing (people walking through, doors), plus fast multipath
    /// fading averaged over half a second (rate adaptation follows that, not single frames). Zero when live
    /// variation is off (planning uses averages).
    fn fade(&self, key: u64, band: Band, ap: &Device, cl: &Device, width: u16) -> f32 {
        if !self.live {
            return 0.0;
        }
        let (sd, m, tc) = self.fading(ap, cl, band, width);
        let slow = sd * crate::sim::wander(self.seed, key * 8 + band as u64, self.clock, SHADOW_S);
        let z = crate::sim::wander(self.seed, key * 8 + 4 + band as u64, self.clock, HALF_S);
        slow + 10.0 * gamma_quantile(m * (HALF_S / tc).max(1.0) as f32, z).log10()
    }

    /// How steady a Wi-Fi link is over time, from its average signal and fading (see `Stability`).
    pub fn stability(&self, l: &Link) -> Option<Stability> {
        if self.memo.borrow().on {
            if let Some(s) = self.memo.borrow().stability.get(&l.id) {
                return *s;
            }
        }
        let s = self.stability_fresh(l);
        let mut m = self.memo.borrow_mut();
        if m.on {
            m.stability.insert(l.id, s);
        }
        s
    }

    fn stability_fresh(&self, l: &Link) -> Option<Stability> {
        let s = self.signal(l, false)?;
        let (ap, cl) = match l.medium {
            Medium::WiFi => self.wifi_ends(l)?,
            Medium::Mesh => (self.dev(l.a)?, self.dev(l.b)?),
            _ => return None,
        };
        if !s.alive() {
            return Some(Stability { margin: 0.0, retry: 1.0, loss: 1.0, drops_h: 0.0, down: 1.0 });
        }
        let (sd, m, tc) = self.fading(ap, cl, s.band, s.width);
        // Both directions must get through; the access point hears a little better (more antennas).
        let margin = s.rssi.min(s.up + 3.0) - sens(20);
        let collide = 0.03 * self.hidden(l).len() as f32 + 0.08 * (s.contention - 1.0);
        // Rate adaptation probes faster rates and leaves ~3 dB to spare: a frame caught in a deeper fade, or in a
        // collision, is sent again. Real homes resend about a quarter of frames (median 25%, WiSe, 30 homes).
        let retry = (0.1 + 0.5 * gamma_below(m, 0.5) + collide).min(0.95);
        // Lost even after 7 tries: retries come within milliseconds, inside one fade, so they fail together.
        // Averaged over the slow shadowing (Gauss–Hermite, 7 points).
        const GH: [(f32, f32); 7] = [(-2.651961, 0.000971781), (-1.673552, 0.054515583), (-0.816288, 0.42560725), (0.0, 0.81026462), (0.816288, 0.42560725), (1.673552, 0.054515583), (2.651961, 0.000971781)];
        let outage: f32 = GH.iter().map(|(x, w)| w * gamma_below(m, 10f32.powf((-margin - std::f32::consts::SQRT_2 * sd * x) / 10.0))).sum::<f32>() / std::f32::consts::PI.sqrt();
        let loss = (outage + collide.min(1.0).powi(4)).min(1.0);
        // Dropouts: the half-second signal sinks below the edge for longer than the beacons last. Rice's formula
        // for how often a smooth Gaussian process crosses a level (u sd below its mean).
        let fast_10 = -10.0 * gamma_quantile(m * (HALF_S / tc).max(1.0) as f32, -1.2816).log10();
        let u = (margin - fast_10) / sd;
        let per_s = 0.25 / SHADOW_S as f32 * (-u.powi(2) / 2.0).exp();
        // Only dips that outlast the beacons count; they last Φ(−u)/rate on average, taken as exponential.
        let lasts = phi(-u) / per_s.max(1e-12);
        let drops_h = (3600.0 * per_s * (-(BEACON_LOSS_S as f32) / lasts.max(1e-6)).exp()).min(3600.0 / (REJOIN_S + BEACON_LOSS_S) as f32);
        let down = (drops_h * (REJOIN_S + BEACON_LOSS_S) as f32 / 3600.0 + outage).min(1.0);
        Some(Stability { margin, retry, loss, drops_h, down })
    }

    /// Share of airtime a transmitter uses right now. Neighbours' traffic comes and goes (streaming, downloads).
    fn busy_now(&self, t: &Device, band: Band, live: bool) -> f32 {
        let base = if t.kind.foreign() { t.busy } else { self.demand_airtime(t, band) };
        // Your own radios' real traffic already comes and goes (see extras::want); only neighbours' is guessed.
        if !live || (!t.kind.foreign() && self.memory.borrow().air.contains_key(&(self.chan_src(t).id, band.i()))) {
            return base;
        }
        (base * (1.0 + 0.6 * crate::sim::wander(self.seed, t.id as u64 + 0x5EED, self.clock, 7.0))).clamp(0.0, 0.95)
    }

    /// Airtime your own radio needs on a band, plus ~2% for beacons: each device's apps at this hour over the rate
    /// its own signal allows, so a far, slow device takes more of the air than a near one with the same traffic.
    pub(crate) fn demand_airtime(&self, ap: &Device, band: Band) -> f32 {
        let key = (ap.id, band.i());
        if self.memo.borrow().on {
            if let Some(a) = self.memo.borrow().airtime.get(&key) {
                return *a;
            }
        }
        let a = self.demand_airtime_fresh(ap, band);
        let mut m = self.memo.borrow_mut();
        if m.on {
            m.airtime.insert(key, a);
        }
        a
    }

    fn demand_airtime_fresh(&self, ap: &Device, band: Band) -> f32 {
        // What the last sharing of the traffic really sent on this channel, once there is one.
        if let Some(a) = self.memory.borrow().air.get(&(self.chan_src(ap).id, band.i())) {
            return (0.02 + a).min(0.95);
        }
        let w = ap.width[band.i()].max(20);
        let used: f32 = self
            .links
            .iter()
            .filter(|l| l.medium == Medium::WiFi && (l.a == ap.id || l.b == ap.id))
            .filter_map(|l| self.dev(l.other(ap.id).0))
            .filter(|c| !c.off && !c.kind.radio())
            .map(|c| {
                let demand: f32 = self.demands(c).iter().map(|x| x.2 + x.3).sum();
                let rssi = self.rssi_at(ap, c.pos, c.floor, band);
                let streams = usable_streams(rssi, ap.streams.min(c.streams).max(1));
                let rate = (ap.wifi.stream20(band.i()) * width_factor(w) * streams as f32 * rate_fraction(rssi, w, ap.wifi, band.i())).max(1.0);
                demand / (rate * mac_eff(rate))
            })
            .sum();
        (0.02 + used).min(0.8)
    }

    /// The radio whose channel a device really uses: single-radio repeaters follow their upstream.
    pub(crate) fn chan_src<'a>(&'a self, d: &'a Device) -> &'a Device {
        if d.kind.radio() && !d.backhaul && d.kind != Kind::HomeRouter { self.upstream(d).unwrap_or(d) } else { d }
    }

    /// The radio an extender (or wireless mesh point) repeats.
    pub fn upstream(&self, d: &Device) -> Option<&Device> {
        if !matches!(d.kind, Kind::Extender | Kind::AccessPoint) {
            return None;
        }
        self.links.iter().filter(|l| l.medium == Medium::WiFi && (l.a == d.id || l.b == d.id)).find_map(|l| {
            let o = self.dev(l.other(d.id).0)?;
            (o.kind.radio() && (d.kind != Kind::AccessPoint || o.kind != Kind::Extender) && !(o.kind == Kind::Extender && d.kind == Kind::Extender && o.id > d.id)).then_some(o)
        })
    }

    /// The (channel, width) a radio really uses: single-radio repeaters must follow their upstream, and a radio
    /// kept off its radar channel sends on channel 36 (at most 80 MHz, all radar-free) meanwhile.
    pub fn chan(&self, d: &Device, band: Band) -> (u16, u16) {
        let src = self.chan_src(d);
        let (ch, w) = (src.ch[band.i()], src.width[band.i()].max(20));
        if band == Band::B5 && self.dfs_wait(src).is_some() { (36, w.min(80)) } else { (ch, w) }
    }

    pub fn channel(&self, d: &Device, band: Band) -> u16 {
        self.chan(d, band).0
    }

    /// Airtime sharing on `ap`'s channel: (divisor, who shares it).
    pub fn contention(&self, ap: &Device, band: Band) -> (f32, Vec<u32>) {
        self.contention_at(ap, band, self.live)
    }

    pub fn contention_at(&self, ap: &Device, band: Band, live: bool) -> (f32, Vec<u32>) {
        if !ap.bands[band.i()] {
            return (1.0, vec![]);
        }
        let v = self.heard_load(ap, ap.id, ap.pos, ap.floor, band, live);
        (1.0 + v.iter().map(|x| x.1).sum::<f32>(), v.into_iter().map(|x| x.0).collect())
    }

    /// Others on `ap`'s channel heard at a spot (the access point's, or that of its device `me`): who, and
    /// the share of airtime each takes.
    fn heard_load(&self, ap: &Device, me: u32, pos: [f32; 2], floor: i8, band: Band, live: bool) -> Vec<(u32, f32)> {
        let mine = self.chan(ap, band);
        let mut v = vec![];
        for t in self.devices.iter().filter(|t| t.id != ap.id && t.id != me && !t.off && t.kind.transmits() && t.bands[band.i()]) {
            // An extender and its upstream are one conversation; that's counted as halving.
            if self.upstream(ap).map(|u| u.id) == Some(t.id) || self.upstream(t).map(|u| u.id) == Some(ap.id) {
                continue;
            }
            let theirs = self.chan(t, band);
            let ov = overlap(band, mine, theirs);
            if ov == 0.0 {
                continue;
            }
            // Wi-Fi waits for anything it can decode on its main 20 MHz (from −82 dBm); traffic that only
            // touches the extra width of a wide channel counts once it's loud (energy detect, −62 dBm).
            // Non-Wi-Fi noise (a microwave oven) can't be decoded: only energy detection, from −62 dBm.
            let heard_from = if t.kind != Kind::Interference && overlap(band, (mine.0, 20), theirs) > 0.0 { -82.0 } else { -62.0 };
            if self.rssi_at(t, pos, floor, band) >= heard_from {
                v.push((t.id, ov * self.busy_now(t, band, live)));
            }
        }
        v
    }

    /// Neighbours' radios `ap` hears on a band (from −82 dBm, where Wi-Fi waits for them) on a channel that passes `which`.
    pub fn heard_on(&self, ap: &Device, band: Band, which: impl Fn(u16) -> bool) -> Vec<&Device> {
        self.devices.iter().filter(|t| t.kind == Kind::Neighbor && !t.off && t.bands[band.i()] && which(self.channel(t, band)) && self.rssi_at(t, ap.pos, ap.floor, band) >= -82.0).collect()
    }

    /// Which end of a Wi-Fi link is the access point.
    pub fn wifi_ends<'a>(&'a self, l: &Link) -> Option<(&'a Device, &'a Device)> {
        let (a, b) = (self.dev(l.a)?, self.dev(l.b)?);
        Some(match (a.kind.radio(), b.kind.radio()) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            (true, true) => {
                if self.upstream(a).map(|u| u.id) == Some(b.id) { (b, a) } else { (a, b) }
            }
            _ => return None,
        })
    }

    /// Signal and speed of a Wi-Fi (or speaker-mesh) link right now, honouring both sides' hardware.
    pub fn link_signal(&self, l: &Link) -> Option<Signal> {
        self.signal(l, self.live)
    }

    /// Like `link_signal`, but `live = false` gives the long-run average (what spanning tree and plans use).
    pub fn signal(&self, l: &Link, live: bool) -> Option<Signal> {
        let key = (l.id, live, self.live);
        if self.memo.borrow().on {
            if let Some(s) = self.memo.borrow().signal.get(&key) {
                return *s;
            }
        }
        let s = self.signal_fresh(l, live);
        let mut m = self.memo.borrow_mut();
        if m.on {
            m.signal.insert(key, s);
        }
        s
    }

    fn signal_fresh(&self, l: &Link, live: bool) -> Option<Signal> {
        let (ap, cl) = match l.medium {
            Medium::WiFi => self.wifi_ends(l)?,
            Medium::Mesh => (self.dev(l.a)?, self.dev(l.b)?),
            _ => return None,
        };
        let walls = self.wall_loss(ap.pos, ap.floor, cl.pos, cl.floor, Band::B5).1;
        let std = ap.wifi.min(cl.wifi);
        let streams = ap.streams.min(cl.streams).max(1);
        let rx_gain = if ap.streams >= 4 { 2.0 } else { 0.0 };
        // Echoes from metal and coated glass cost a little speed even on average.
        let multipath = 1.0 - 0.04 * (self.reflectors(cl.pos, cl.floor) + self.reflectors(ap.pos, ap.floor)).min(4) as f32;
        let mut best: Option<Signal> = None;
        let mut alive: Vec<Signal> = vec![];
        for band in Band::ALL {
            let width = ap.width[band.i()].min(cl.width[band.i()]).min(std.max_width(band.i())).min(self.chan(ap, band).1);
            if !ap.bands[band.i()] || !cl.bands[band.i()] || width == 0 {
                continue;
            }
            let loss = self.path_loss(ap.pos, ap.floor, cl.pos, cl.floor, cl.mount - ap.mount, band);
            let fade = if live { self.fade(l.id as u64, band, ap, cl, width) } else { 0.0 };
            // Directional antennas help (or hurt) at both ends of the link; sending is capped at the legal limit.
            let rssi = self.eirp(ap, cl.pos, band) + dir_gain(cl, ap.pos) + rx_gain + ap.cal_db - loss + fade;
            let up = self.eirp(cl, ap.pos, band) + dir_gain(ap, cl.pos) + rx_gain + ap.cal_db - loss + fade;
            // Multi-antenna access points aim their data at the device (beamforming; not on Wi-Fi 4-era 2.4 GHz).
            let bf = if streams >= 2 && (band != Band::B24 || std >= WifiStd::Wifi6) { 3.0 } else { 0.0 };
            // Others the access point hears make it wait; ones only the device hears drown out what it receives.
            let at_ap = self.heard_load(ap, cl.id, ap.pos, ap.floor, band, live);
            let near_me: f32 = self.heard_load(ap, cl.id, cl.pos, cl.floor, band, live).iter().filter(|x| !at_ap.iter().any(|a| a.0 == x.0)).map(|x| x.1).sum();
            let contention = 1.0 + at_ap.iter().map(|x| x.1).sum::<f32>() + near_me;
            // Radio rate at a signal: a weak link drops to fewer streams and to a narrower part of the channel.
            let phy_at = |sig: f32| -> (f32, u16) {
                let mut best = (0.0, width);
                let mut w = 20;
                while w <= width {
                    let r = std.stream20(band.i()) * width_factor(w) * usable_streams(sig, streams) as f32 * rate_fraction(sig, w, std, band.i());
                    if r > best.0 {
                        best = (r, w);
                    }
                    w *= 2;
                }
                best
            };
            let ((phy, used), (phy_up, _)) = (phy_at(rssi + bf), phy_at(up));
            // Data one way needs acknowledgements the other way: both directions must get through.
            let usable = phy > 0.0 && phy_up > 0.0;
            let rate = |p: f32| if usable { p * mac_eff(p) * multipath / contention } else { 0.0 };
            let s = Signal { rssi, up, band, width: used, streams: usable_streams(rssi + bf, streams), mbps: rate(phy), up_mbps: rate(phy_up), phy, walls, contention, also: None };
            if s.alive() {
                alive.push(s);
            }
            // With band steering the device lands on the fastest band; without, on the loudest.
            let better = match best {
                None => true,
                Some(b) if ap.band_steer => s.mbps > b.mbps || (b.mbps == 0.0 && s.rssi > b.rssi),
                Some(b) => (s.alive() && !b.alive()) || (s.alive() == b.alive() && s.rssi > b.rssi),
            };
            if better {
                best = Some(s);
            }
        }
        // Multi-Link (both ends Wi-Fi 7 with MLO on): the two best bands carry data together, and when one
        // drops the other simply carries on — no hand-over, no gap.
        if ap.mlo && cl.mlo && std >= WifiStd::Wifi7 && alive.len() >= 2 {
            alive.sort_by(|a, b| b.mbps.total_cmp(&a.mbps));
            let (a, b) = (alive[0], alive[1]);
            best = Some(Signal { mbps: a.mbps + b.mbps, up_mbps: a.up_mbps + b.up_mbps, phy: a.phy + b.phy, also: Some(b.band), ..a });
        }
        if live && l.medium == Medium::WiFi {
            best = self.hold_drop(l.id, best);
        }
        best
    }

    /// A live link whose signal stays below the edge longer than the beacons last is dropped: the device must
    /// find the access point again, authenticate and renew its address before anything flows.
    fn hold_drop(&self, id: u32, best: Option<Signal>) -> Option<Signal> {
        let alive = best.is_some_and(|s| s.alive());
        let t = self.clock;
        let mut m = self.link_down.borrow_mut();
        let e = m.entry(id).or_insert((f64::NAN, f64::NEG_INFINITY));
        // The clock went back (Simulate restarted): forget.
        if e.1 - t > REJOIN_S + BEACON_LOSS_S || e.0 > t {
            *e = (f64::NAN, f64::NEG_INFINITY);
        }
        if t < e.1 {
            return best.map(|s| Signal { mbps: 0.0, up_mbps: 0.0, ..s });
        }
        if alive {
            e.0 = f64::NAN;
        } else if e.0.is_nan() {
            e.0 = t;
        } else if t - e.0 >= BEACON_LOSS_S {
            *e = (f64::NAN, t + REJOIN_S);
        }
        best
    }

    /// How steady a link is, in a few words: dropouts, then packets lost, then frames resent.
    pub fn steadiness(&self, l: &Link) -> Option<String> {
        use crate::i18n::trf;
        let st = self.stability(l)?;
        if st.down >= 1.0 {
            return None;
        }
        let resent = format!("{:.0}", st.retry * 100.0);
        Some(if st.drops_h >= 1.0 {
            trf("drops about {} times an hour · {}% of frames resent", &[&format!("{:.0}", st.drops_h), &resent])
        } else if st.drops_h >= 0.04 {
            trf("drops about once every {} hours · {}% of frames resent", &[&format!("{:.0}", 1.0 / st.drops_h), &resent])
        } else if st.loss >= 0.002 {
            trf("stays connected, but {}% of packets get lost", &[&format!("{:.1}", st.loss * 100.0)])
        } else {
            trf("steady · {}% of frames resent", &[&resent])
        })
    }

    /// Whether a live Wi-Fi link is joining again after a dropout.
    pub fn rejoining(&self, id: u32) -> bool {
        self.live && self.link_down.borrow().get(&id).is_some_and(|e| self.clock < e.1)
    }

    /// What the cable itself carries (Mbit/s), whatever it's plugged into.
    pub fn cable_mbps(&self, l: &Link) -> u32 {
        match l.medium {
            // Old Cat 5 still carries gigabit (1000BASE-T was made for it), but nothing faster.
            Medium::Cat5 => 1000,
            Medium::Cat5e => 2500,
            Medium::Cat6 if self.link_len(l) <= 55.0 => 10000,
            Medium::Cat6 => 5000,
            Medium::Cat6a | Medium::Fiber => 10000,
            _ => u32::MAX,
        }
    }

    /// The speed two Ethernet ends agree on (Mbit/s): the slower of the two ports, limited by what the cable carries.
    pub fn negotiated(&self, l: &Link) -> u32 {
        self.cable_mbps(l).min(self.end_mbps(l, l.a)).min(self.end_mbps(l, l.b))
    }

    /// What one connection gets over a link: LACP hashes each connection onto a single cable of a bundle,
    /// so one download can't go faster than one cable even when the bundle carries more in total.
    pub fn flow_rate(&self, l: &Link) -> f32 {
        let b = self.bundle(l);
        let all: u32 = b.iter().map(|o| self.negotiated(o)).sum();
        if b.len() > 1 && all > 0 { self.link_rate(l) * self.negotiated(l) as f32 / all as f32 } else { self.link_rate(l) }
    }

    /// Real-world throughput of a link in Mbit/s (all connections together).
    pub fn link_rate(&self, l: &Link) -> f32 {
        let base = match l.medium {
            Medium::WiFi | Medium::Mesh => self.link_signal(l).map(|s| s.mbps).unwrap_or(0.0),
            // A 5G home router: ~300 Mbit/s down (50 up), depending on the mast and how many people use it.
            Medium::Cellular => 300.0 * self.cell_now(l),
            Medium::Powerline => {
                // Old wiring, distance and other appliances: every house is different (seeded per link).
                let wiring = 0.35 + 0.75 * crate::sim::unit(self.seed, l.id as u64, 0x9A);
                let now = if self.live { (1.0 + 0.2 * crate::sim::wander(self.seed, l.id as u64 + 0x9B, self.clock, 4.0)) * self.appliance_noise(l) } else { 1.0 };
                let close = crate::catalog::find_kit(&l.kit).map_or(300.0, |k| k.real_mbps);
                (close * (-self.wiring_m(l) / 70.0).exp() * wiring * now * self.phase_factor(l)).min(self.negotiated(l) as f32 * 0.94)
            }
            // Ethernet carries about 94% of the line rate as data; a bundle (LACP) adds its cables up for all
            // connections together (one connection alone gets one cable: see flow_rate).
            _ => self.bundle(l).iter().map(|o| self.negotiated(o)).sum::<u32>() as f32 * 0.94,
        };
        let isp = [l.a, l.b].iter().filter_map(|x| self.dev(*x)).find(|d| d.kind == Kind::Internet);
        let rate = isp.map(|i| base.min(i.plan_mbps)).unwrap_or(base);
        // Smart queueing shapes the line a little below its speed so the queue stays in the router.
        if self.sqm_on(l) { rate * 0.92 } else { rate }
    }

    /// A noisy appliance on the wiring (vacuum cleaner, cheap charger, dimmer, washing machine motor) switched on
    /// for a few minutes now and then: powerline loses most of its speed while it runs. Live only.
    /// shortcut: a seeded on/off pattern (on about 15% of the minutes), not a model of the house's appliances.
    fn appliance_noise(&self, l: &Link) -> f32 {
        let minute = (self.clock / 60.0).floor() as u64;
        if crate::sim::unit(self.seed, l.id as u64 + 0x9C, minute) < 0.15 { 0.4 } else { 1.0 }
    }

    /// Live swing of a mobile line (1 = its average).
    fn cell_now(&self, l: &Link) -> f32 {
        if self.live { 1.0 + 0.3 * crate::sim::wander(self.seed, l.id as u64 + 0x5C, self.clock, 5.0) } else { 1.0 }
    }

    /// Length of house wiring a powerline link runs over (m): the wires wander, ~1.5× the straight distance
    /// plus 5 m per storey (unless the length was measured).
    pub fn wiring_m(&self, l: &Link) -> f32 {
        match (l.length_m, self.dev(l.a), self.dev(l.b)) {
            (Some(m), _, _) => m,
            (None, Some(a), Some(b)) => (a.pos[0] - b.pos[0]).hypot(a.pos[1] - b.pos[1]) / PX_PER_M * 1.5 + 5.0 * (a.floor as f32 - b.floor as f32).abs(),
            _ => 0.0,
        }
    }

    /// Powerline across storeys usually means another circuit or phase: the signal has to cross in the fuse box.
    fn phase_factor(&self, l: &Link) -> f32 {
        let floor = |id: u32| self.dev(id).map_or(0, |d| d.floor);
        if floor(l.a) != floor(l.b) { 0.6 } else { 1.0 }
    }

    /// Throughput from the first device of a path to the last (an upload): radio hops in the direction they're
    /// crossed (devices send weaker than routers), the plan's upload speed, the mobile line's slower uplink, and
    /// same-channel radio hops taking turns as in `path_rate`.
    pub fn upload_rate(&self, path: &[u32]) -> f32 {
        let mut worst = f32::MAX;
        let mut shared: Vec<((Band, u16), f32)> = vec![];
        for w in path.windows(2) {
            let Some(l) = self.link_between(w[0], w[1]) else { continue };
            let r = match (l.medium, self.link_signal(l)) {
                (Medium::Cellular, _) => self.link_rate(l).min(50.0 * self.cell_now(l)),
                (_, Some(s)) => {
                    let ap = self.wifi_ends(l).map_or(l.a, |e| e.0.id);
                    let r = if ap == w[0] { s.mbps } else { s.up_mbps };
                    let key = (s.band, self.dev(ap).map_or(0, |d| self.chan(d, s.band).0));
                    match shared.iter_mut().find(|g| g.0 == key) {
                        Some(g) => g.1 += 1.0 / r.max(0.01),
                        None => shared.push((key, 1.0 / r.max(0.01))),
                    }
                    r
                }
                _ => {
                    let up = [l.a, l.b].iter().filter_map(|x| self.dev(*x)).find(|d| d.kind == Kind::Internet).map(|i| if i.plan_up_mbps > 0.0 { i.plan_up_mbps } else { i.plan_mbps });
                    self.link_rate(l).min(up.unwrap_or(f32::MAX))
                }
            };
            worst = worst.min(r);
        }
        shared.iter().fold(worst, |m, g| m.min(1.0 / g.1))
    }

    /// The internet link of a home router with smart queueing on.
    pub fn sqm_on(&self, l: &Link) -> bool {
        let (a, b) = (self.dev(l.a), self.dev(l.b));
        let has = |x: Option<&Device>, k: Kind| x.is_some_and(|d| d.kind == k);
        (has(a, Kind::Internet) && b.is_some_and(|d| d.kind == Kind::HomeRouter && d.sqm)) || (has(b, Kind::Internet) && a.is_some_and(|d| d.kind == Kind::HomeRouter && d.sqm))
    }

    /// Average-condition liveness (spanning tree doesn't flap with every fade).
    pub fn link_up(&self, l: &Link) -> bool {
        !l.medium.wireless() || self.signal(l, false).is_some_and(|s| s.alive())
    }

    /// Radio hops along a path: chance a packet is lost after retries, mean extra delay waiting for
    /// airtime and resending (ms), and the weakest signal on the way.
    pub fn path_air(&self, path: &[u32]) -> (f32, f32, Option<f32>) {
        let mut ok = 1.0f32;
        let mut jitter = 0.0;
        let mut worst: Option<f32> = None;
        for w in path.windows(2) {
            let Some(l) = self.link_between(w[0], w[1]) else { continue };
            let Some(s) = self.link_signal(l) else { continue };
            let Some(st) = self.stability(l) else { continue };
            ok *= 1.0 - st.loss;
            // Each retry waits a little longer (back-off doubles); shared airtime makes everyone wait their turn.
            jitter += 0.6 + 2.5 * (s.contention - 1.0) + 6.0 * st.retry;
            worst = Some(worst.map_or(s.rssi, |w: f32| w.min(s.rssi)));
        }
        (1.0 - ok, jitter, worst)
    }

    /// When radar last hit a radio's DFS channel (live clock, s), if that was under 30 minutes (plus the
    /// listening time to come back) ago. Radar shows up now and then: about once in 10 half-hours (seeded).
    fn radar_at(&self, d: &Device) -> Option<f64> {
        let (c, w) = (d.ch[1], d.width[1]);
        if !self.live || !d.bands[1] || !dfs(Band::B5, c, w) {
            return None;
        }
        let slot = (self.clock / 1800.0).floor();
        (0..3).map(|k| slot - k as f64).filter(|s| *s >= 0.0).find_map(|s| {
            let k = d.id as u64 + 0xDF5;
            let at = (s + crate::sim::unit(self.seed, k + 1, s as u64) as f64) * 1800.0;
            (crate::sim::unit(self.seed, k, s as u64) < 0.1 && self.clock >= at && self.clock < at + 1800.0 + cac_s(Band::B5, c, w)).then_some(at)
        })
    }

    /// Seconds a radio still stays off its radar (DFS) channel (live only): it listens for a minute (10 on the
    /// weather-radar channels) after power-on, and must leave for 30 minutes when radar shows up.
    pub fn dfs_wait(&self, d: &Device) -> Option<f64> {
        let cac = cac_s(Band::B5, d.ch[1], d.width[1]);
        let left = match self.radar_at(d) {
            Some(at) => at + 1800.0 + cac - self.clock,
            None if self.live && d.bands[1] => cac - self.clock,
            None => 0.0,
        };
        (left > 0.0).then_some(left)
    }

    /// Radar was heard on the radio's DFS channel in the last 30 minutes (live only): it moved to channel 36.
    pub fn radar(&self, ap: &Device) -> bool {
        self.radar_at(self.chan_src(ap)).is_some()
    }

    /// Other devices on the same access point that this one can't hear ("hidden stations"):
    /// they talk at the same time and collide at the access point.
    pub fn hidden(&self, l: &Link) -> Vec<u32> {
        let Some((ap, me)) = self.wifi_ends(l) else { return vec![] };
        if me.kind.radio() {
            return vec![];
        }
        self.links
            .iter()
            .filter(|o| o.id != l.id && o.medium == Medium::WiFi)
            .filter_map(|o| self.wifi_ends(o).filter(|(a, c)| a.id == ap.id && !c.kind.radio() && !c.off).map(|(_, c)| c))
            .filter(|c| {
                // Can I hear them? Their transmit power over the path between us, on 5 GHz.
                let heard = c.tx[1].max(c.tx[0]) - self.path_loss(c.pos, c.floor, me.pos, me.floor, me.mount - c.mount, Band::B5);
                heard < -82.0
            })
            .map(|c| c.id)
            .collect()
    }

    /// Throughput along a path of devices and the weakest link, described.
    pub fn path_rate(&self, path: &[u32]) -> (f32, String) {
        let mut best = (f32::MAX, String::from("no links"));
        // Radio hops on the same band and channel take turns in the air: a packet that crosses it
        // three times needs three slots, so together they reach 1 / (1/r1 + 1/r2 + 1/r3).
        let mut shared: Vec<((Band, u16), f32, Vec<u32>)> = vec![];
        for w in path.windows(2) {
            let Some(l) = self.link_between(w[0], w[1]) else { continue };
            let r = self.flow_rate(l);
            if let (Some(s), Some((ap, _))) = (self.link_signal(l), self.wifi_ends(l).or_else(|| (l.medium == Medium::Mesh).then(|| (self.dev(l.a), self.dev(l.b))).and_then(|(a, b)| Some((a?, b?))))) {
                let key = (s.band, self.chan(ap, s.band).0);
                match shared.iter_mut().find(|g| g.0 == key) {
                    Some(g) => {
                        g.1 += 1.0 / r.max(0.01);
                        g.2.push(l.id);
                    }
                    None => shared.push((key, 1.0 / r.max(0.01), vec![l.id])),
                }
            }
            if r < best.0 {
                let mut why = format!("{} {} ↔ {}", l.medium.label(), self.name(l.a), self.name(l.b));
                if !l.medium.wireless() && l.medium != Medium::Powerline {
                    let slow = [l.a, l.b].iter().map(|x| (*x, self.end_mbps(l, *x))).min_by_key(|x| x.1).filter(|x| x.1 == self.negotiated(l));
                    if let Some((d, mbps)) = slow {
                        why += &format!(" — {}'s Ethernet port runs at {mbps} Mbit/s", self.name(d));
                    }
                }
                if let Some(s) = self.link_signal(l) {
                    why += &format!(
                        " ({:.0} dBm on {} {} MHz, {}×{} streams, {} wall(s){})",
                        s.rssi,
                        s.band.label(),
                        s.width,
                        s.streams,
                        s.streams,
                        s.walls,
                        if s.contention > 1.05 { format!(", airtime shared ×{:.1}", s.contention) } else { String::new() }
                    );
                }
                if l.medium == Medium::Mesh {
                    why += " — speaker mesh carrying everyone's traffic";
                }
                if let Some(i) = [l.a, l.b].iter().filter_map(|x| self.dev(*x)).find(|d| d.kind == Kind::Internet) {
                    if r >= i.plan_mbps - 0.5 {
                        why = format!("your internet plan ({:.0} Mbit/s)", i.plan_mbps);
                    }
                }
                best = (r, why);
            }
        }
        for ((band, ch), inv, hops) in shared.into_iter().filter(|g| g.2.len() > 1) {
            let r = 1.0 / inv;
            if r < best.0 {
                let names: Vec<String> = hops
                    .iter()
                    .filter_map(|id| self.links.iter().find(|l| l.id == *id))
                    .map(|l| format!("{} ↔ {}", self.name(l.a), self.name(l.b)))
                    .collect();
                best = (r, format!("{} radio hops on {} channel {ch} take turns in the air ({}): every packet is sent {} times", hops.len(), band.label(), names.join(", "), hops.len()));
            }
        }
        best
    }

    /// Speed a device really gets to the internet: the whole way, not just its own link.
    /// None when it has no way out (or there's no internet in the plan).
    pub fn internet_rate(&self, id: u32, stp: &crate::stp::Stp) -> Option<(f32, String)> {
        let inet = self.devices.iter().find(|d| d.kind == Kind::Internet)?.id;
        // Shortest working path, around links spanning tree keeps on standby.
        let mut prev: std::collections::HashMap<u32, u32> = std::collections::HashMap::from([(id, id)]);
        let mut q = std::collections::VecDeque::from([id]);
        while let Some(c) = q.pop_front() {
            if c == inet {
                break;
            }
            // Only the start, switches, access points and routers pass traffic on.
            if c != id && !self.dev(c).is_some_and(|d| d.kind.bridges() || d.kind.routes() || d.kind.radio()) {
                continue;
            }
            for l in self.links.iter().filter(|l| (l.a == c || l.b == c) && self.link_ok(l) && !stp.blocked.contains(&l.id) && self.link_up(l)) {
                let o = l.other(c).0;
                if let std::collections::hash_map::Entry::Vacant(e) = prev.entry(o) {
                    e.insert(c);
                    q.push_back(o);
                }
            }
        }
        prev.get(&inet)?;
        let mut path = vec![inet];
        while *path.last().unwrap() != id {
            path.push(prev[path.last().unwrap()]);
        }
        path.reverse();
        Some(self.path_rate(&path))
    }

    /// A wireless client's link to its access point.
    pub fn wifi_link_of(&self, id: u32) -> Option<&Link> {
        let d = self.dev(id)?;
        self.links.iter().find(|l| {
            l.medium == Medium::WiFi && (l.a == id || l.b == id) && self.wifi_ends(l).is_some_and(|(ap, cl)| cl.id == id && ap.id != id) && !(d.kind.radio() && self.upstream(d).is_none())
        })
    }

    /// Access points a client could roam to: same network name, strongest first.
    pub fn roam_candidates(&self, client: &Device, ssid: &str) -> Vec<(u32, f32)> {
        let mut v: Vec<(u32, f32)> = self
            .devices
            .iter()
            .filter(|a| a.kind.radio() && !a.off && Network::names_of(a).iter().any(|n| n.0 == ssid) && a.id != client.id && self.upstream(client).map(|u| u.id) != Some(a.id))
            // An extender only counts when it reaches the router, and not through this client (two extenders feeding each other).
            .filter(|a| a.kind != Kind::Extender || (self.upstream(a).is_some() && !self.feeds_from(a, client.id)))
            .map(|a| (a.id, self.best_rssi(a, client.pos, client.floor)))
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }

    /// Whether `dev` gets its Wi-Fi through `via` (directly or further up a chain of extenders).
    pub(crate) fn feeds_from(&self, dev: &Device, via: u32) -> bool {
        let mut cur = self.upstream(dev);
        for _ in 0..8 {
            match cur {
                Some(u) if u.id == via => return true,
                Some(u) => cur = self.upstream(u),
                None => return false,
            }
        }
        false
    }

    /// One step of a client's roaming logic. Returns what happened when it switched.
    pub fn roam_step(&mut self, client_id: u32) -> Option<Roam> {
        let link = self.wifi_link_of(client_id)?.clone();
        let (ap, cl) = self.wifi_ends(&link)?;
        let (ap, cl) = (ap.clone(), cl.clone());
        let now = self.best_rssi(&ap, cl.pos, cl.floor);
        let name = Network::joined(&cl, &ap).to_string();
        let best = self.roam_candidates(&cl, &name).into_iter().find(|c| c.0 != ap.id);
        let kicked = ap.min_rssi != 0 && now < ap.min_rssi as f32;
        let lost = now < -86.0;
        // Clients hang on until the signal is bad (sticky); 802.11k/v nudges them earlier.
        let threshold = roam_threshold(&ap, &cl);
        let (to, rssi) = best?;
        if !(kicked || lost || (now < threshold && rssi > now + 8.0)) || rssi < -86.0 {
            return None;
        }
        let new_ap = self.dev(to)?.clone();
        let fast = ap.fast_roam && new_ap.fast_roam;
        let segs = self.segments();
        let same_lan = segs.get(&(ap.id, 0)) == segs.get(&(new_ap.id, 0));
        let l = self.links.iter_mut().find(|x| x.id == link.id)?;
        if l.a == ap.id {
            (l.a, l.a_if) = (to, 0);
        } else {
            (l.b, l.b_if) = (to, 0);
        }
        let on = self.memo.borrow().on;
        self.memo.borrow_mut().reset(on);
        Some(Roam { from: ap.id, to, before: now, after: rssi, ms: if fast { 40.0 } else if lost { 2500.0 } else { 350.0 }, kicked, lost, same_lan, fast })
    }

    /// Picks non-overlapping channels for your own radios (at their widths), avoiding the neighbours.
    pub fn auto_channels(&mut self) -> String {
        let ids: Vec<u32> = self.devices.iter().filter(|d| matches!(d.kind, Kind::HomeRouter | Kind::AccessPoint) || d.kind == Kind::Extender && d.backhaul).map(|d| d.id).collect();
        let mut done: Vec<u32> = vec![];
        let mut out = vec![];
        for id in ids {
            let me = self.dev(id).unwrap().clone();
            let mut new = me.ch;
            for band in Band::ALL.into_iter().filter(|b| me.bands[b.i()]) {
                let w = me.width[band.i()];
                // 2.4 GHz: the 1/6/11 and 1/5/9/13 plans' channels; whichever overlaps least wins (1/6/11 on a tie).
                let cands: Vec<u16> = if band == Band::B24 { CH24.to_vec() } else { band.channels(w) };
                let cost = |ch: u16| -> f32 {
                    self.devices
                        .iter()
                        .filter(|t| t.id != id && t.bands[band.i()] && (t.kind.foreign() || done.contains(&t.id)))
                        .map(|t| {
                            let heard = self.rssi_at(t, me.pos, me.floor, band);
                            if heard < -85.0 { 0.0 } else { overlap(band, (ch, w), self.chan(t, band)) * if t.kind.foreign() { t.busy } else { 0.6 } * (1.0 + (heard + 85.0) / 30.0) }
                        })
                        .sum::<f32>()
                        // Radar channels cost a minute's silence at power-on (10 on weather-radar ones) and a
                        // forced move whenever radar shows up.
                        + if !dfs(band, ch, w) { 0.0 } else if cac_s(band, ch, w) > 60.0 { 0.25 } else { 0.08 }
                };
                if let Some(c) = cands.into_iter().min_by(|a, b| cost(*a).total_cmp(&cost(*b))) {
                    new[band.i()] = c;
                }
            }
            let d = self.dev_mut(id).unwrap();
            if d.ch != new {
                out.push(format!("{}: {:?} → {:?}", d.name, d.ch, new));
            }
            d.ch = new;
            done.push(id);
        }
        if out.is_empty() { "Channels were already the best choice.".into() } else { format!("New channels (2.4 / 5 / 6 GHz) — {}", out.join("; ")) }
    }
}

#[derive(Clone, Debug)]
pub struct Roam {
    pub from: u32,
    pub to: u32,
    pub before: f32,
    pub after: f32,
    pub ms: f32,
    pub kicked: bool,
    pub lost: bool,
    pub same_lan: bool,
    pub fast: bool,
}

/// Thread and Zigbee radios: transmit power (dBm) and the weakest signal they still decode.
const ZB_TX: f32 = 3.0;
const ZB_FLOOR: f32 = -92.0;

/// The 2 MHz an 802.15.4 channel (11–26) uses: 2405 MHz for 11, then every 5 MHz.
fn zb_range(ch: u8) -> (f32, f32) {
    let f = 2405.0 + 5.0 * (ch as f32 - 11.0);
    (f - 1.0, f + 1.0)
}

impl Network {
    /// The hub a Thread or Zigbee gadget belongs to (Thread border router and Zigbee coordinator in one), with
    /// the signal (dBm) of its own link into the mesh.
    pub fn smart_hub(&self, d: &Device) -> Option<(u32, f32)> {
        self.smart_route(d).map(|(hub, _, dbm, _)| (hub, dbm))
    }

    /// How a Thread or Zigbee gadget joins: (hub, the device it talks to, that link's signal, hops to the hub).
    /// Mains-powered gadgets (plugs, bulbs: they draw at least 0.5 W) repeat for others like real Thread routers and
    /// Zigbee routers, so a gadget out of the hub's reach still joins through its neighbours, up to 8 hops;
    /// battery sensors only talk to one parent. Fewer hops win, then the stronger link.
    pub fn smart_route(&self, d: &Device) -> Option<(u32, u32, f32, u8)> {
        if !d.on_802154() {
            return None;
        }
        let sig = |a: &Device, b: &Device| ZB_TX - self.path_loss(a.pos, a.floor, b.pos, b.floor, b.mount - a.mount, Band::B24);
        // Everything already in the mesh: (device, its hub, hops from the hub).
        let mut mesh: Vec<(&Device, u32, u8)> = self.devices.iter().filter(|h| h.id != d.id && h.hub && !h.off && matches!(h.kind, Kind::HomeRouter | Kind::Speaker | Kind::SmartHome)).map(|h| (h, h.id, 0)).collect();
        let relays: Vec<&Device> = self.devices.iter().filter(|r| r.id != d.id && r.on_802154() && !r.off && !r.hub && r.watts >= 0.5).collect();
        for hop in 1..=8 {
            let grown: Vec<(&Device, u32, u8)> = relays
                .iter()
                .filter(|r| !mesh.iter().any(|m| m.0.id == r.id))
                .filter_map(|r| mesh.iter().filter(|m| sig(m.0, r) >= ZB_FLOOR).max_by(|a, b| sig(a.0, r).total_cmp(&sig(b.0, r))).map(|m| (*r, m.1, hop)))
                .collect();
            if grown.is_empty() {
                break;
            }
            mesh.extend(grown);
        }
        mesh.iter()
            .map(|m| (m, sig(m.0, d)))
            .filter(|x| x.1 >= ZB_FLOOR)
            .min_by(|a, b| a.0 .2.cmp(&b.0 .2).then(b.1.total_cmp(&a.1)))
            .map(|(m, dbm)| (m.1, m.0.id, dbm, m.2 + 1))
    }

    /// Wi-Fi transmitters a hub hears (above −82 dBm, its own radio too) whose 2.4 GHz channel covers its 802.15.4 channel `ch`.
    pub fn zb_clash(&self, hub: &Device, ch: u8) -> Vec<(u32, u16)> {
        let z = zb_range(ch);
        self.devices
            .iter()
            .filter(|t| !t.off && t.kind.transmits() && t.kind != Kind::Interference && t.bands[0])
            .filter(|t| self.rssi_at(t, hub.pos, hub.floor, Band::B24) >= -82.0)
            .filter_map(|t| {
                let (c, w) = self.chan(t, Band::B24);
                let r = range(Band::B24, c, w);
                (z.1 > r.0 && z.0 < r.1).then_some((t.id, c))
            })
            .collect()
    }

    /// A Thread/Zigbee channel that no Wi-Fi around the hub uses, if there is one (15, 20, 25 and 26 sit between 1, 6 and 11).
    pub fn free_zb_channel(&self, hub: &Device) -> Option<u8> {
        [15, 20, 25, 26].into_iter().chain(11..=24).find(|c| self.zb_clash(hub, *c).is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::template;

    fn pair(n: &mut Network, d: f32) -> Link {
        let ap = n.add_device(Kind::AccessPoint, [0.0, 0.0]);
        let ph = n.add_device(Kind::Phone, [d, 0.0]);
        let l = n.connect(ap, ph, Medium::WiFi).unwrap();
        n.links.iter().find(|x| x.id == l).unwrap().clone()
    }

    #[test]
    fn heights_count_in_three_dimensions() {
        let mut n = Network::default();
        let link = pair(&mut n, 10.0);
        let at = |n: &Network| n.signal(&link, false).unwrap().rssi;
        let desk = at(&n);
        // On the ceiling right above the phone: 1.5 m further away.
        n.dev_mut(link.a).unwrap().mount = 2.5;
        assert!(at(&n) < desk - 3.0, "{desk} {}", at(&n));
        // Upstairs: a tall storey is further than a low one.
        n.dev_mut(link.a).unwrap().mount = 1.0;
        n.dev_mut(link.b).unwrap().floor = 1;
        let low = at(&n);
        n.storey_heights.push((0, 4.0));
        assert!(at(&n) < low - 1.0 && (n.floor_z(1) - 4.0).abs() < 1e-6 && (n.floor_z(-1) + 2.8).abs() < 1e-6);
    }

    #[test]
    fn multi_link_adds_two_bands_and_survives_one_dropping() {
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        for d in n.devices.iter_mut() {
            d.wifi = WifiStd::Wifi7;
            d.bands = [true, true, true];
            normalize_radio(d);
        }
        let single = n.signal(&link, false).unwrap();
        for d in n.devices.iter_mut() {
            d.mlo = true;
        }
        let both = n.signal(&link, false).unwrap();
        assert!(both.also.is_some() && both.mbps > single.mbps * 1.2, "{} {}", single.mbps, both.mbps);
        // One band goes away: the link keeps running on the others at once.
        n.dev_mut(link.a).unwrap().bands[both.band.i()] = false;
        assert!(n.signal(&link, false).unwrap().alive());
    }

    #[test]
    fn zigbee_gadgets_join_a_hub_and_dodge_busy_wifi() {
        let mut n = template("home");
        let r = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().clone();
        let plug = n.add_device(Kind::SmartHome, [r.pos[0] + 60.0, r.pos[1]]);
        n.dev_mut(plug).unwrap().smart = crate::net::SmartLink::Zigbee;
        let plug_d = n.dev(plug).unwrap().clone();
        assert!(n.smart_hub(&plug_d).is_none());
        assert!(n.check().iter().any(|i| i.dev == Some(plug) && i.text.contains("no smart-home hub")));
        n.dev_mut(r.id).unwrap().hub = true;
        assert_eq!(n.smart_hub(&plug_d).map(|h| h.0), Some(r.id));
        // Its own Wi-Fi on 2.4 GHz channel 6 (2427–2447 MHz) covers Zigbee 17 (2435 MHz) but not 25 (2475 MHz).
        n.dev_mut(r.id).unwrap().ch[0] = 6;
        n.dev_mut(r.id).unwrap().zb_ch = 17;
        let hub = n.dev(r.id).unwrap().clone();
        assert!(n.zb_clash(&hub, 17).iter().any(|c| c.0 == r.id) && n.zb_clash(&hub, 25).iter().all(|c| c.0 != r.id));
        let fix = n.check().into_iter().find_map(|i| i.fix.filter(|f| matches!(f, crate::issues::Fix::ZigbeeChannel(..)))).expect("channel tip");
        n.apply_fix(&fix);
        let hub = n.dev(r.id).unwrap().clone();
        assert!(n.zb_clash(&hub, hub.zb_ch).is_empty(), "{}", hub.zb_ch);
    }

    #[test]
    fn a_far_device_uses_more_airtime_than_a_near_one_with_the_same_traffic() {
        let mut n = Network::default();
        let ap = n.add_device(Kind::AccessPoint, [0.0, 0.0]);
        let ph = n.add_device(Kind::Phone, [2.0 * PX_PER_M, 0.0]);
        n.connect(ap, ph, Medium::WiFi).unwrap();
        n.dev_mut(ph).unwrap().uses = vec![crate::extras::Use::Stream4k];
        n.hour = 20.5;
        let near = n.demand_airtime(n.dev(ap).unwrap(), Band::B5);
        n.dev_mut(ph).unwrap().pos = [100.0 * PX_PER_M, 0.0];
        let far = n.demand_airtime(n.dev(ap).unwrap(), Band::B5);
        // Beacons (2%) aside, the same stream takes about twice the air from far away.
        assert!(far - 0.02 > 1.5 * (near - 0.02), "{near} {far}");
    }

    #[test]
    fn a_far_sensor_joins_the_hub_through_a_plug_that_relays() {
        let mut n = Network::default();
        let hub = n.add_device(Kind::SmartHome, [0.0, 0.0]);
        n.dev_mut(hub).unwrap().hub = true;
        // 250 m apart each, in the open: a sensor at 500 m is out of the hub's reach (about 300 m).
        let plug = n.add_device(Kind::SmartHome, [250.0 * PX_PER_M, 0.0]);
        let sensor = n.add_device(Kind::SmartHome, [500.0 * PX_PER_M, 0.0]);
        for (id, w) in [(plug, 1.0), (sensor, 0.0)] {
            let d = n.dev_mut(id).unwrap();
            d.smart = crate::net::SmartLink::Zigbee;
            d.watts = w;
        }
        let s = n.dev(sensor).unwrap().clone();
        assert_eq!(n.smart_route(&s).map(|r| (r.0, r.1, r.3)), Some((hub, plug, 2)), "through the mains-powered plug");
        // A battery sensor doesn't relay: with the plug on batteries, the far one is cut off.
        n.dev_mut(plug).unwrap().watts = 0.0;
        assert!(n.smart_route(&s).is_none());
    }

    #[test]
    fn devices_that_cannot_hear_each_other_collide() {
        let mut n = Network::default();
        let ap = n.add_device(Kind::AccessPoint, [0.0, 250.0]);
        let a = n.add_device(Kind::Phone, [400.0, 0.0]);
        let b = n.add_device(Kind::Laptop, [-400.0, 0.0]);
        let la = n.connect(ap, a, Medium::WiFi).unwrap();
        n.connect(ap, b, Medium::WiFi).unwrap();
        let la = n.links.iter().find(|x| x.id == la).unwrap().clone();
        let before = n.path_air(&[a, ap]).0;
        assert!(n.hidden(&la).is_empty());
        // A wall of metal and concrete between the two clients, but not in the way to the access point.
        for (i, k) in [WallKind::Metal, WallKind::Concrete, WallKind::Concrete].into_iter().enumerate() {
            let x = i as f32 * 10.0;
            n.walls.push(Wall { id: 90 + i as u32, a: [x, -200.0], b: [x, 180.0], kind: k, floor: 0 });
        }
        assert_eq!(n.hidden(&la), vec![b]);
        assert!(n.path_air(&[a, ap]).0 >= before);
    }

    #[test]
    fn a_phone_behind_two_extenders_gets_the_whole_paths_speed() {
        let mut n = Network::default();
        let inet = n.add_device(Kind::Internet, [-200.0, 0.0]);
        n.dev_mut(inet).unwrap().plan_mbps = 10000.0;
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        let e1 = n.add_device(Kind::Extender, [300.0, 0.0]);
        let e2 = n.add_device(Kind::Extender, [650.0, 0.0]);
        let ph = n.add_device(Kind::Phone, [690.0, 0.0]);
        // A concrete wall between the two extenders makes that hop the slow one.
        n.walls.push(Wall { id: 1, a: [450.0, -2000.0], b: [450.0, 2000.0], kind: WallKind::Concrete, floor: 0 });
        n.connect(inet, r, Medium::Fiber).unwrap();
        let ids: Vec<u32> = [(r, e1), (e1, e2), (e2, ph)].iter().map(|(a, b)| n.connect(*a, *b, Medium::WiFi).unwrap()).collect();
        let rates: Vec<f32> = ids.iter().map(|id| n.link_rate(n.links.iter().find(|l| l.id == *id).unwrap())).collect();
        let (e2e, why) = n.internet_rate(ph, &n.stp()).unwrap();
        // All three hops share one channel: slower than the slowest hop, and the phone's own link is fast.
        let shared = 1.0 / rates.iter().map(|r| 1.0 / r).sum::<f32>();
        assert!((e2e - shared).abs() < 1.0 && e2e < rates[1] * 0.9 && rates[2] > rates[1], "{rates:?} → {e2e} ({why})");
        assert!(why.contains("3 radio hops"), "{why}");
    }

    #[test]
    fn walls_cost_signal_and_speed() {
        let mut n = Network::default();
        let link = pair(&mut n, 300.0);
        let open = n.link_signal(&link).unwrap();
        // A wall across the whole house: nothing to bend around.
        n.walls.push(Wall { id: 99, a: [150.0, -2000.0], b: [150.0, 2000.0], kind: WallKind::Concrete, floor: 0 });
        let walled = n.link_signal(&link).unwrap();
        assert!(walled.mbps < open.mbps / 2.0, "{open:?} {walled:?}");
        assert_eq!(walled.walls, 1);
        let (ap, ph) = (n.dev(link.a).unwrap(), n.dev(link.b).unwrap());
        assert!(n.rssi_at(ap, ph.pos, ph.floor, Band::B5) < open.rssi - 20.0);
    }

    #[test]
    fn hardware_settings_change_the_speed() {
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        let base = n.link_signal(&link).unwrap().mbps;
        let ap = link.a;
        // Fewer antennas and a narrower channel: slower.
        n.dev_mut(ap).unwrap().streams = 1;
        n.dev_mut(ap).unwrap().width = [20, 20, 20];
        assert!(n.link_signal(&link).unwrap().mbps < base / 4.0);
        // A 2.4 GHz-only gadget can't use 5/6 GHz.
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        CLIENT_PRESETS[5].apply(n.dev_mut(link.b).unwrap());
        assert_eq!(n.link_signal(&link).unwrap().band, Band::B24);
        // Turning the router's power down shrinks its reach.
        let mut n = Network::default();
        let link = pair(&mut n, 500.0);
        let at = |n: &Network| n.rssi_at(n.dev(link.a).unwrap(), n.dev(link.b).unwrap().pos, 0, Band::B5);
        let before = at(&n);
        n.dev_mut(link.a).unwrap().tx = [5.0, 5.0, 5.0];
        assert!(at(&n) < before - 10.0);
    }

    #[test]
    fn neighbours_on_my_channel_share_airtime_and_auto_channels_fix_it() {
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        let nb = n.add_device(Kind::Neighbor, [200.0, 0.0]);
        n.dev_mut(nb).unwrap().ch = [6, 36, 37];
        n.dev_mut(nb).unwrap().busy = 0.6;
        n.dev_mut(r).unwrap().ch = [6, 36, 37];
        let (c, who) = n.contention(n.dev(r).unwrap(), Band::B5);
        assert!(c > 1.5 && who == [nb]);
        n.auto_channels();
        assert!(n.contention(n.dev(r).unwrap(), Band::B5).0 < 1.01);
        assert!(n.contention(n.dev(r).unwrap(), Band::B24).0 < 1.01);
    }

    /// Live variation must be weather, not mood: centred on the planned average and bounded.
    #[test]
    fn live_variation_is_centred_and_bounded() {
        let mut n = Network::default();
        let link = pair(&mut n, 300.0);
        let avg = n.signal(&link, false).unwrap().rssi;
        n.live = true;
        let xs: Vec<f32> = (0..3000).map(|i| {
            n.clock = i as f64 * 0.37;
            n.link_signal(&link).unwrap().rssi
        }).filter(|r| *r > -110.0).collect();
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        let worst = xs.iter().fold(0.0f32, |m, x| m.max((x - avg).abs()));
        assert!((mean - avg).abs() < 1.0, "mean {mean} vs average {avg}");
        assert!(worst < 12.0, "swing {worst} dB");
        // A strong link loses (almost) nothing.
        let (p, _, _) = n.path_air(&[link.a, link.b]);
        assert!(p < 0.01, "{p}");
    }

    #[test]
    fn floors_and_slow_ports_matter() {
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        let same = n.signal(&link, false).unwrap().rssi;
        n.dev_mut(link.b).unwrap().floor = 1;
        let above = n.signal(&link, false).unwrap().rssi;
        assert!(above < same - 15.0, "{same} {above}");
        let sw = n.add_device(Kind::Switch, [0.0, 0.0]);
        let tv = n.add_device(Kind::Tv, [40.0, 0.0]);
        let l = n.connect(sw, tv, Medium::Cat6a).unwrap();
        let l = n.links.iter().find(|x| x.id == l).unwrap().clone();
        assert_eq!(n.negotiated(&l), 100);
        assert!(n.link_rate(&l) < 100.0);
    }

    #[test]
    fn switching_standard_and_bands_gives_sensible_values() {
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        // The reported case: Wi-Fi 6 router → Wi-Fi 7, then switch 6 GHz on.
        let d = n.dev_mut(r).unwrap();
        assert_eq!(d.wifi, WifiStd::Wifi6);
        d.wifi = WifiStd::Wifi7;
        d.bands[2] = true;
        normalize_radio(d);
        assert!(d.width[2] >= 160 && d.tx[2] > 15.0 && Band::B6.channels(d.width[2]).contains(&d.ch[2]), "{:?} {:?} {:?}", d.width, d.tx, d.ch);
        // Back down to Wi-Fi 6: 6 GHz goes off, widths stay legal.
        d.wifi = WifiStd::Wifi6E;
        d.width[2] = 320;
        normalize_radio(d);
        assert_eq!(d.width[2], 160);
        d.wifi = WifiStd::Wifi6;
        normalize_radio(d);
        assert!(!d.bands[2]);
        // Every preset, every band switched on: never zero width or power.
        for p in RADIO_PRESETS.iter().chain(CLIENT_PRESETS) {
            let mut x = n.dev(r).unwrap().clone();
            p.apply(&mut x);
            x.wifi = WifiStd::Wifi7;
            x.bands = [true, true, true];
            normalize_radio(&mut x);
            for i in 0..3 {
                assert!(x.width[i] > 0 && x.tx[i] > 0.0, "{}: {:?} {:?}", p.name, x.width, x.tx);
            }
        }
        // A phone's adapter switched to 6 GHz on a standard without it stays off.
        let ph = n.add_device(Kind::Phone, [10.0, 0.0]);
        let d = n.dev_mut(ph).unwrap();
        d.wifi = WifiStd::Wifi6;
        d.bands[2] = true;
        normalize_radio(d);
        assert!(!d.bands[2]);
        // A second router and a second access point don't clash with the first.
        let r2 = n.add_device(Kind::HomeRouter, [0.0, 100.0]);
        assert_ne!(n.dev(r).unwrap().ifaces[0].ip, n.dev(r2).unwrap().ifaces[0].ip);
        let a1 = n.add_device(Kind::AccessPoint, [100.0, 0.0]);
        let a2 = n.add_device(Kind::AccessPoint, [200.0, 0.0]);
        assert!(n.dev(a1).unwrap().ch[0] != n.dev(a2).unwrap().ch[0] || n.dev(a1).unwrap().ch[1] != n.dev(a2).unwrap().ch[1]);
    }

    #[test]
    fn channel_lists_respect_widths() {
        // EU/CH: no 144 or 149–165 on 5 GHz; 6 GHz only up to 6425 MHz (one 320 MHz channel, 5945–6265).
        assert_eq!(Band::B5.channels(160), vec![36, 100]);
        assert_eq!(Band::B5.channels(80), vec![36, 52, 100, 116]);
        assert_eq!(*Band::B5.channels(20).last().unwrap(), 140);
        assert_eq!(Band::B6.channels(320), vec![1]);
        assert_eq!((Band::B6.channels(160).len(), *Band::B6.channels(20).last().unwrap()), (3, 93));
        assert_eq!((Band::B5.eirp_limit(52), Band::B5.eirp_limit(100)), (23.0, 30.0));
        assert!(dfs(Band::B5, 36, 160) && !dfs(Band::B5, 36, 80) && cac_s(Band::B5, 116, 80) == 600.0 && cac_s(Band::B5, 100, 20) == 60.0);
        assert_eq!(overlap(Band::B5, (36, 80), (44, 20)), 1.0);
        assert_eq!(overlap(Band::B24, (1, 20), (6, 20)), 0.0);
        assert_eq!(overlap(Band::B24, (1, 20), (3, 20)), 1.5);
    }

    #[test]
    fn slanted_walls_and_extra_floors() {
        let mut n = Network::default();
        n.walls.push(Wall { id: 1, a: [100.0, -400.0], b: [100.0, 400.0], kind: WallKind::Brick, floor: 0 });
        let straight = n.wall_loss([0.0, 0.0], 0, [200.0, 0.0], 0, Band::B5).0;
        let slant = n.wall_loss([0.0, 0.0], 0, [200.0, 400.0], 0, Band::B5).0;
        // At a slant the wave bends towards the wall's normal inside it: a little longer, plus some reflection.
        assert!(straight == 13.0 && slant > 14.0 && slant < 18.0, "{straight} {slant}");
        // Every further concrete slab costs less than the first one.
        let pl = |f: i8| n.path_loss([0.0, 0.0], 0, [0.0, 0.0], f, 0.0, Band::B5);
        assert!(pl(2) - pl(1) < pl(1) - pl(0) - 5.0, "{} {} {}", pl(0), pl(1), pl(2));
    }

    #[test]
    fn weak_links_drop_streams_and_width_and_send_back_slower() {
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        n.dev_mut(link.b).unwrap().bands = [false, true, false];
        let near = n.signal(&link, false).unwrap();
        assert!(near.streams == 2 && near.up_mbps < near.mbps * 1.01, "{near:?}");
        // A phone far away: one stream, a narrow part of the channel, and its weaker radio uploads slower.
        n.dev_mut(link.b).unwrap().pos = [800.0, 0.0];
        n.walls.push(Wall { id: 50, a: [400.0, -2000.0], b: [400.0, 2000.0], kind: WallKind::Concrete, floor: 0 });
        let far = n.signal(&link, false).unwrap();
        assert!(far.alive() && far.streams == 1 && far.up_mbps < far.mbps / 2.0, "{far:?}");
        assert!(mac_eff(10.0) < 0.5 && mac_eff(1200.0) == 0.7);
    }

    #[test]
    fn wide_channels_and_interference_near_the_device() {
        let mut n = Network::default();
        let link = pair(&mut n, 100.0);
        n.dev_mut(link.b).unwrap().bands = [false, true, false];
        let base = n.signal(&link, false).unwrap().contention;
        // A neighbour on the second half of our 80 MHz channel only counts once it's loud (−62 dBm).
        let nb = n.add_device(Kind::Neighbor, [0.0, 1200.0]);
        n.dev_mut(nb).unwrap().ch = [6, 52, 37];
        n.dev_mut(nb).unwrap().width[1] = 20;
        n.dev_mut(nb).unwrap().busy = 0.5;
        n.dev_mut(link.a).unwrap().ch[1] = 36;
        n.dev_mut(link.a).unwrap().width[1] = 160;
        let ap = n.dev(link.a).unwrap().clone();
        assert!(n.rssi_at(n.dev(nb).unwrap(), ap.pos, 0, Band::B5) > -82.0);
        assert!(n.contention(&ap, Band::B5).0 < 1.01);
        n.dev_mut(nb).unwrap().pos = [0.0, 200.0];
        assert!(n.contention(n.dev(link.a).unwrap(), Band::B5).0 > 1.1);
        // A neighbour behind the phone that the access point can't hear still slows the phone down.
        n.dev_mut(nb).unwrap().ch[1] = 36;
        n.dev_mut(nb).unwrap().pos = [1500.0, 0.0];
        n.dev_mut(nb).unwrap().tx[1] = 5.0;
        n.dev_mut(link.b).unwrap().pos = [500.0, 0.0];
        let s = n.signal(&link, false).unwrap();
        assert!(n.contention(n.dev(link.a).unwrap(), Band::B5).0 < 1.01 && s.contention > base + 0.1, "{s:?}");
    }

    #[test]
    fn own_access_points_are_as_busy_as_their_apps() {
        let mut n = template("home");
        let r = n.devices.iter().find(|d| d.kind == Kind::HomeRouter).unwrap().clone();
        let evening = n.demand_airtime(&r, Band::B5);
        n.hour = 4.0;
        assert!(n.demand_airtime(&r, Band::B5) < evening && evening < 0.5, "{evening}");
    }

    #[test]
    fn radar_and_power_on_keep_the_radio_off_dfs_channels() {
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        n.dev_mut(r).unwrap().ch[1] = 100;
        n.live = true;
        n.clock = 10.0;
        let d = n.dev(r).unwrap().clone();
        assert_eq!(n.chan(&d, Band::B5), (36, 80));
        n.clock = 70.0;
        assert_eq!(n.chan(&d, Band::B5).0, 100);
        // Over a long day radar shows up now and then: each time the radio moves for 30 minutes.
        let hits = (0..2000).filter(|i| {
            n.clock = 61.0 + *i as f64 * 60.0;
            n.radar(&d)
        }).count();
        assert!((10..800).contains(&hits), "{hits}");
        n.live = false;
        assert_eq!(n.chan(&d, Band::B5).0, 100);
    }

    #[test]
    fn auto_channels_can_use_the_1_5_9_13_plan() {
        let mut n = Network::default();
        let r = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        for (i, c) in [1, 5, 9].into_iter().enumerate() {
            let nb = n.add_device(Kind::Neighbor, [100.0 + 50.0 * i as f32, 0.0]);
            n.dev_mut(nb).unwrap().ch[0] = c;
            n.dev_mut(nb).unwrap().busy = 0.5;
        }
        n.auto_channels();
        assert_eq!(n.dev(r).unwrap().ch[0], 13);
    }

    #[test]
    fn powerline_between_storeys_is_slower() {
        let mut n = Network::default();
        let a = n.add_device(Kind::Switch, [0.0, 0.0]);
        let b = n.add_device(Kind::Desktop, [200.0, 0.0]);
        let l = n.connect(a, b, Medium::Powerline).unwrap();
        let l = n.links.iter().find(|x| x.id == l).unwrap().clone();
        let same = n.link_rate(&l);
        n.dev_mut(b).unwrap().floor = 1;
        assert!(n.link_rate(&l) < same * 0.6 && (n.wiring_m(&l) - 20.0).abs() < 0.1, "{} {}", same, n.wiring_m(&l));
    }

    #[test]
    fn phone_roams_when_walking() {
        let mut n = template("roaming");
        let phone = n.devices.iter().find(|d| d.kind == Kind::Phone).unwrap().id;
        let start_ap = n.wifi_ends(n.wifi_link_of(phone).unwrap()).unwrap().0.id;
        let far = n.devices.iter().find(|d| d.kind == Kind::AccessPoint).unwrap().pos;
        // Right next to the far access point. The router still reaches −68 dBm through the opening beside the
        // concrete wall, so a typical phone hangs on; with 802.11k/v it's nudged across.
        n.dev_mut(phone).unwrap().pos = [far[0] - 20.0, far[1]];
        n.dev_mut(phone).unwrap().roam_at = -72;
        assert!(n.roam_step(phone).is_none(), "sticky without 802.11k/v");
        n.devices.iter_mut().filter(|d| d.kind.radio()).for_each(|d| d.fast_roam = true);
        let r = n.roam_step(phone).expect("should hand over");
        assert_ne!(r.to, start_ap);
    }

    #[test]
    fn signals_bend_round_wall_ends_and_corners_but_not_through_joints() {
        let w = |a: [f32; 2], b: [f32; 2]| Wall { id: 0, a, b, kind: WallKind::Brick, floor: 0 };
        // A wall drawn as two pieces in a straight line: its ends bend, the joint doesn't.
        assert_eq!(wall_edges(&[w([0.0, 0.0], [100.0, 0.0]), w([100.0, 0.0], [200.0, 0.0])]).len(), 2);
        // An L: two free ends and the outside of the corner.
        let l = wall_edges(&[w([0.0, 0.0], [100.0, 0.0]), w([100.0, 0.0], [100.0, 100.0])]);
        assert_eq!(l.len(), 3);
        let c = l.iter().find(|e| e.at == [100.0, 0.0]).unwrap();
        assert!(c.open[0] > 100.0 && c.open[1] < 0.0, "outside the corner: {c:?}");
        // A T: only the three free ends.
        assert_eq!(wall_edges(&[w([0.0, 0.0], [200.0, 0.0]), w([100.0, 0.0], [100.0, 100.0])]).len(), 3);
    }

    #[test]
    fn a_doorway_lets_signal_round_the_corner_smoothly() {
        // Router in a room; a concrete wall with a 1 m door gap; a phone behind the wall, off to the side.
        let mut n = Network::default();
        let r = [0.0, 0.0];
        let wall = |n: &mut Network, a, b| n.walls.push(Wall { id: n.walls.len() as u32, a, b, kind: WallKind::Concrete, floor: 0 });
        wall(&mut n, [100.0, -2000.0], [100.0, 0.0]);
        wall(&mut n, [100.0, 20.0], [100.0, 2000.0]);
        let behind = [200.0, 100.0];
        let through = 22.0 * (dist(r, behind) / PX_PER_M).log10() + 20.0 * 5500f32.log10() - 28.0 + WallKind::Concrete.loss()[1];
        let got = n.path_loss(r, 0, behind, 0, 0.0, Band::B5);
        assert!(got < through - 3.0, "round the door frame beats 23 dB of concrete: {got} vs {through}");
        assert!(got > dist_loss(Band::B5, dist(r, behind) / PX_PER_M) + 6.0, "but it's no open door: {got}");
        // Walking past the door: no sudden jumps.
        let mut last = None;
        for i in 0..80 {
            let p = [300.0, -100.0 + i as f32 * 5.0];
            let l = n.path_loss(r, 0, p, 0, 0.0, Band::B5);
            if let Some(o) = last {
                assert!((l - o as f32).abs() < 3.5, "jump at {p:?}: {o} → {l}");
            }
            last = Some(l);
        }
        // Lower frequencies bend more easily.
        let lit = |b| n.path_loss(r, 0, behind, 0, 0.0, b) - dist_loss(b, dist(r, behind) / PX_PER_M);
        assert!(lit(Band::B24) < lit(Band::B5));
    }

    #[test]
    fn far_links_drop_now_and_then_and_live_ones_rejoin_after_a_pause() {
        let mut n = Network::default();
        let ap = n.add_device(Kind::HomeRouter, [0.0, 0.0]);
        let near = n.add_device(Kind::Phone, [100.0, 0.0]);
        let far = n.add_device(Kind::Phone, [3600.0, 0.0]);
        n.walls.push(Wall { id: 1, a: [50.0, -3000.0], b: [50.0, 3000.0], kind: WallKind::Brick, floor: 0 });
        let link = |n: &mut Network, d| {
            let id = n.connect(ap, d, Medium::WiFi).unwrap();
            n.links.iter().find(|l| l.id == id).unwrap().clone()
        };
        let (ln, lf) = (link(&mut n, near), link(&mut n, far));
        let (sn, sf) = (n.stability(&ln).unwrap(), n.stability(&lf).unwrap());
        // A phone in the next room: steady, and resending about as much as a healthy home does. One at the far end
        // of the garden drops now and then.
        assert!(sn.drops_h < 0.01 && sn.loss < 0.001 && (0.08..0.35).contains(&sn.retry), "{sn:?}");
        assert!(sf.drops_h >= 1.0 && sf.retry > sn.retry && sf.loss > sn.loss, "{sf:?}");
        // An hour of Simulate: the far phone drops about as often as predicted, and each time stays off while it
        // joins again.
        n.live = true;
        let (mut drops, mut was, mut since) = (0, false, 0.0);
        let step = 0.25;
        for t in 0..(3600.0 / step) as usize {
            n.clock = t as f64 * step;
            let _ = n.signal(&lf, true);
            let now = n.rejoining(lf.id);
            if now && !was {
                drops += 1;
                since = n.clock;
            }
            if !now && was {
                assert!(n.clock - since >= REJOIN_S - step, "rejoined too fast");
            }
            was = now;
        }
        let want = sf.drops_h;
        assert!((drops as f32) > want / 4.0 && (drops as f32) < want * 4.0 + 2.0, "{drops} drops in an hour, predicted {want}");
        assert!(!n.rejoining(ln.id));
    }
}

#[cfg(test)]
mod speed {
    use super::*;
    use std::time::Instant;

    fn ms(t: Instant) -> f64 {
        t.elapsed().as_secs_f64() * 1000.0
    }

    #[test]
    fn caches_never_change_a_result() {
        let mut n = crate::net::template_seeded("campus", 7);
        n.live = true;
        n.clock = 30.0;
        // The problem check shares out the traffic, which updates the airtime memory: do it first.
        let issues: Vec<String> = n.check().into_iter().map(|i| i.text).collect();
        let fresh: Vec<_> = n.links.iter().map(|l| (n.signal(l, true), n.signal(l, false), n.stability(l))).collect();
        n.memo.borrow_mut().reset(true);
        for _ in 0..2 {
            let again: Vec<_> = n.links.iter().map(|l| (n.signal(l, true), n.signal(l, false), n.stability(l))).collect();
            assert_eq!(format!("{fresh:?}"), format!("{again:?}"));
        }
        assert_eq!(issues, n.check().into_iter().map(|i| i.text).collect::<Vec<_>>());
        let (a, b) = (n.devices[5].pos, n.devices[40].pos);
        assert_eq!(n.path_losses(a, 0, b, 0, 0.3), n.path_losses_raw(a, 0, b, 0, 0.3));
        // Moving a wall forgets the cached geometry.
        let before = n.path_losses(a, 0, b, 0, 0.3);
        n.walls.push(Wall { id: 999, a: [(a[0] + b[0]) / 2.0, -5000.0], b: [(a[0] + b[0]) / 2.0 + 1.0, 5000.0], kind: WallKind::Metal, floor: 0 });
        assert!(n.path_losses(a, 0, b, 0, 0.3)[1] > before[1] + 20.0);
        // A copy (an undo step) starts with empty caches.
        assert!(n.clone().path_cache.borrow().map.is_empty() && !n.clone().memo.borrow().on);
    }

    #[test]
    fn the_map_shortcut_finds_the_same_strongest_signal() {
        let n = crate::net::template_seeded("campus", 7);
        let radios: Vec<&Device> = n.devices.iter().filter(|d| d.kind.radio() && d.bands.iter().any(|b| *b)).collect();
        for y in (0..1700).step_by(37) {
            for x in (0..1500).step_by(41) {
                let p = [x as f32, y as f32];
                for band in [None, Some(Band::B24), Some(Band::B5), Some(Band::B6)] {
                    let brute = radios.iter().map(|r| match band {
                        Some(b) => n.rssi_at_raw(r, p, 0, b),
                        None => n.best_rssi_raw(r, p, 0),
                    }).fold(-120.0, f32::max);
                    assert_eq!(n.strongest_at(&radios, p, 0, band, -200.0), brute, "{p:?} {band:?}");
                    let floor = n.strongest_at(&radios, p, 0, band, -90.0);
                    assert!(if brute >= -90.0 { floor == brute } else { floor < -90.0 }, "{p:?} {band:?}");
                }
            }
        }
    }

    /// The work one UI frame and one refresh do on the campus, with budgets that keep it smooth (release build):
    /// `cargo test --release campus_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn campus_speed() {
        let mut n = crate::net::template_seeded("campus", 7);
        println!("{} devices, {} links, {} walls", n.devices.len(), n.links.len(), n.walls.len());
        let memo = std::env::var("NO_MEMO").is_err();
        for live in [false, true] {
            n.live = live;
            n.clock = 12.0;
            for round in 0..2 {
                n.memo.borrow_mut().reset(memo);
                let t = Instant::now();
                for l in &n.links {
                    let _ = n.link_signal(l);
                }
                for d in &n.devices {
                    let _ = n.wifi_link_of(d.id).and_then(|l| n.link_signal(l));
                }
                let f = ms(t);
                println!("frame (live {live}, round {round}): {f:.1} ms");
                assert!(f < 8.0, "a frame's model work must stay well under 16 ms");
            }
        }
        n.live = true;
        n.memo.borrow_mut().reset(memo);
        let refresh = Instant::now();
        let t = Instant::now();
        let _ = n.check();
        println!("check: {:.1} ms", ms(t));
        let t = Instant::now();
        let _ = n.loads_now();
        println!("loads_now: {:.1} ms", ms(t));
        n.live = false;
        let t = Instant::now();
        let _ = n.loads();
        println!("loads: {:.1} ms", ms(t));
        let t = Instant::now();
        let stp = n.stp();
        println!("stp: {:.1} ms", ms(t));
        let t = Instant::now();
        let hosts: Vec<u32> = n.devices.iter().filter(|d| d.kind.is_host() && !d.off).map(|d| d.id).collect();
        for h in hosts {
            let _ = n.internet_rate(h, &stp);
        }
        println!("internet_rate for every host: {:.1} ms", ms(t));
        let r = ms(refresh);
        println!("whole refresh: {r:.1} ms");
        assert!(r < 60.0, "the refresh runs every 0.3 s on the UI thread");
        let radios: Vec<&Device> = n.devices.iter().filter(|d| d.kind.radio() && d.bands.iter().any(|b| *b)).collect();
        let cells: Vec<[f32; 2]> = (0..90).flat_map(|y| (0..100).map(move |x| [x as f32 * 19.0 - 200.0, y as f32 * 21.0 - 200.0])).collect();
        let t = Instant::now();
        let mut exact = 0;
        for p in &cells {
            let _ = n.strongest_at(&radios, *p, 0, None, n.target - 25.0);
            exact += 1;
        }
        let m = ms(t);
        println!("Wi-Fi map, {} cells x {} radios: {m:.1} ms (spread over frames, 6 ms each)", exact, radios.len());
        assert!(m < 400.0, "the map should fill in within a fraction of a second");
        let t = Instant::now();
        n.assign_ports();
        println!("assign_ports: {:.2} ms", ms(t));
    }
}
