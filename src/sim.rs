//! Randomness that behaves like the real world: one seed per plan (so a plan is reproducible),
//! smooth noise over time for fading and traffic, and hardware addresses with real vendor prefixes.
use crate::net::Kind;

fn mix(mut z: u64) -> u64 {
    // splitmix64
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn hash(seed: u64, a: u64, b: u64) -> u64 {
    mix(seed ^ mix(a.wrapping_add(0x51_7CC1_B727_220A)) ^ mix(b).rotate_left(17))
}

/// Uniform in [0, 1).
pub fn unit(seed: u64, a: u64, b: u64) -> f32 {
    (hash(seed, a, b) >> 40) as f32 / (1u64 << 24) as f32
}

/// Standard normal (mean 0, sd 1).
pub fn gauss(seed: u64, a: u64, b: u64) -> f32 {
    let u1 = unit(seed, a, b.wrapping_mul(2)).max(1e-6);
    let u2 = unit(seed, a, b.wrapping_mul(2) + 1);
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

/// Smooth random walk over time (mean 0, sd ≈ 1) that changes on the scale of `period` seconds.
pub fn wander(seed: u64, key: u64, t: f64, period: f64) -> f32 {
    let x = t / period;
    let i = x.floor();
    let f = (x - i) as f32;
    let s = f * f * (3.0 - 2.0 * f);
    let (a, b) = (gauss(seed, key, i as i64 as u64), gauss(seed, key, (i as i64 + 1) as u64));
    a + (b - a) * s
}

pub fn pick<'a>(seed: u64, a: u64, b: u64, from: &[&'a str]) -> &'a str {
    from[(hash(seed, a, b) % from.len() as u64) as usize]
}

/// A fresh seed for a new plan.
pub fn new_seed() -> u64 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    mix(t)
}

/// Vendor prefixes (OUIs) typical for each kind of device. Spanning tree breaks ties on the full
/// MAC address, so who wins depends on these numbers — just like in a real home.
fn ouis(kind: Kind) -> &'static [&'static str] {
    match kind {
        // Multi-room speaker makers' registered prefixes.
        Kind::Speaker => &["00:0e:58", "34:7e:5c", "48:a6:b8", "54:2a:1b", "5c:aa:fd", "78:28:ca", "94:9f:3e", "b8:e9:37", "f0:f6:c1"],
        Kind::Switch => &["00:09:5b", "00:1a:a1", "24:5a:4c", "28:80:88", "50:c7:bf", "78:8a:20", "98:da:c4", "a0:40:a0", "b0:be:76"],
        Kind::HomeRouter | Kind::Router | Kind::AccessPoint | Kind::Extender => &["04:d4:c4", "24:5a:4c", "3c:a6:2f", "50:c7:bf", "60:31:97", "78:8a:20", "c8:0e:14", "e0:28:6d"],
        _ => &["00:1b:63", "3c:22:fb", "70:85:c2", "a4:83:e7", "b8:27:eb", "d8:3a:dd", "f4:5c:89"],
    }
}

/// A MAC address for port `port` of device `id`. Phones and laptops use randomised private addresses,
/// as current systems do on Wi-Fi.
pub fn mac(seed: u64, kind: Kind, id: u32, port: u32) -> String {
    let h = hash(seed, id as u64, 0xAC);
    let tail = |h: u64| format!("{:02x}:{:02x}:{:02x}", (h >> 16) & 0xff, (h >> 8) & 0xff, (h.wrapping_add(port as u64)) & 0xff);
    if matches!(kind, Kind::Phone | Kind::Laptop) {
        // Locally administered bit set: x2, x6, xa or xe in the first byte.
        let first = (((h >> 40) & 0xfc) | 0x02) as u8;
        return format!("{first:02x}:{:02x}:{:02x}:{}", (h >> 32) & 0xff, (h >> 24) & 0xff, tail(h));
    }
    format!("{}:{}", pick(seed, id as u64, 0x0B, ouis(kind)), tail(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_reproducible_smooth_and_roughly_normal() {
        assert_eq!(wander(7, 1, 3.3, 1.0), wander(7, 1, 3.3, 1.0));
        assert!((wander(7, 1, 3.30, 1.0) - wander(7, 1, 3.31, 1.0)).abs() < 0.2);
        let xs: Vec<f32> = (0..4000).map(|i| gauss(3, 9, i)).collect();
        let mean = xs.iter().sum::<f32>() / xs.len() as f32;
        let sd = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / xs.len() as f32).sqrt();
        assert!(mean.abs() < 0.08 && (sd - 1.0).abs() < 0.08, "{mean} {sd}");
        let m = mac(5, Kind::Phone, 3, 0);
        assert!(u8::from_str_radix(&m[..2], 16).unwrap() & 0x02 == 0x02, "{m}");
    }
}
