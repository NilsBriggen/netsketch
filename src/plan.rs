//! Planning help: how much of each storey has good Wi-Fi, where one more access point would help
//! most, and applying every one-click fix in one go.
use crate::net::{Device, Kind, Network, PX_PER_M};

/// Signal levels the coverage score counts: good enough for video calls, and still usable.
pub const GOOD: f32 = -67.0;
pub const USABLE: f32 = -75.0;

pub struct Coverage {
    pub good: f32,
    pub usable: f32,
    pub area_m2: f32,
}

impl Network {
    /// Sample spots (cell centres) inside the walls of a storey, about 1 m apart (coarser for big buildings).
    /// A spot counts as indoors when walls surround it in all four directions.
    // shortcut: a courtyard closed on all sides counts as indoors; fine for a score.
    fn spots(&self, floor: i8) -> (Vec<[f32; 2]>, f32) {
        let walls: Vec<_> = self.walls.iter().filter(|w| w.floor == floor).collect();
        if walls.len() < 3 {
            return (vec![], 0.0);
        }
        let xs = walls.iter().flat_map(|w| [w.a[0], w.b[0]]);
        let ys = walls.iter().flat_map(|w| [w.a[1], w.b[1]]);
        let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max));
        let (y0, y1) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
        let cell = PX_PER_M.max(((x1 - x0) * (y1 - y0) / 2500.0).sqrt());
        let mut v = vec![];
        let mut y = y0 + cell / 2.0;
        while y < y1 {
            let mut x = x0 + cell / 2.0;
            while x < x1 {
                if [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]].iter().all(|d| walls.iter().any(|w| ray_hits([x, y], *d, w.a, w.b))) {
                    v.push([x, y]);
                }
                x += cell;
            }
            y += cell;
        }
        (v, cell)
    }

    fn radios(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.kind.radio() && !d.kind.foreign() && !d.off && d.bands.iter().any(|b| *b)).collect()
    }

    fn best_at(&self, radios: &[&Device], p: [f32; 2], floor: i8) -> f32 {
        radios.iter().map(|r| self.best_rssi(r, p, floor)).fold(-120.0, f32::max)
    }

    /// Share of a storey with good and with usable Wi-Fi. None until the storey has walls.
    pub fn coverage(&self, floor: i8) -> Option<Coverage> {
        let (spots, cell) = self.spots(floor);
        if spots.is_empty() {
            return None;
        }
        let radios = self.radios();
        let s: Vec<f32> = spots.iter().map(|p| self.best_at(&radios, *p, floor)).collect();
        let share = |t: f32| s.iter().filter(|x| **x >= t).count() as f32 / s.len() as f32;
        Some(Coverage { good: share(GOOD), usable: share(USABLE), area_m2: s.len() as f32 * (cell / PX_PER_M).powi(2) })
    }

    /// The spot on a storey where one more ordinary access point would give the most extra area good Wi-Fi.
    /// Returns the spot and the share of good coverage before and after.
    pub fn suggest_ap(&self, floor: i8) -> Option<([f32; 2], f32, f32)> {
        let (spots, _) = self.spots(floor);
        if spots.is_empty() {
            return None;
        }
        let radios = self.radios();
        let now: Vec<f32> = spots.iter().map(|p| self.best_at(&radios, *p, floor)).collect();
        let weak: Vec<[f32; 2]> = spots.iter().zip(&now).filter(|(_, s)| **s < GOOD).map(|(p, _)| *p).collect();
        let good_now = (spots.len() - weak.len()) as f32 / spots.len() as f32;
        if weak.is_empty() {
            return None;
        }
        // A typical ceiling access point, as the shelf would add it.
        let mut probe = Network::default();
        let id = probe.add_device(Kind::AccessPoint, [0.0, 0.0]);
        let mut ap = probe.dev(id).unwrap().clone();
        ap.floor = floor;
        // Candidates: every 4th spot (keeps it to a few hundred tries).
        let mut best: Option<([f32; 2], usize)> = None;
        for c in spots.iter().step_by(4) {
            ap.pos = *c;
            let gained = weak.iter().filter(|p| self.best_rssi(&ap, **p, floor) >= GOOD).count();
            if best.is_none_or(|b| gained > b.1) {
                best = Some((*c, gained));
            }
        }
        let (pos, gained) = best?;
        (gained > 0).then(|| (pos, good_now, good_now + gained as f32 / spots.len() as f32))
    }

    /// Apply every one-click fix the checks offer (fixes can reveal new ones, so a few passes), then
    /// hand out addresses again. Returns what was done.
    pub fn fix_all(&mut self) -> Vec<String> {
        let mut done = vec![];
        for _ in 0..3 {
            let mut fixes: Vec<_> = self.check().into_iter().filter_map(|i| i.fix).collect();
            // Several problems can offer the same fix (one PoE switch for three cameras): apply it once.
            let mut seen = vec![];
            fixes.retain(|f| {
                let k = format!("{f:?}");
                let new = !seen.contains(&k);
                seen.push(k);
                new
            });
            if fixes.is_empty() {
                break;
            }
            for f in fixes {
                done.push(self.apply_fix(&f));
            }
        }
        self.run_dhcp();
        done
    }
}

/// Does the ray from `p` in direction `d` cross the segment a–b?
fn ray_hits(p: [f32; 2], d: [f32; 2], a: [f32; 2], b: [f32; 2]) -> bool {
    let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
    let den = d[0] * ey - d[1] * ex;
    if den.abs() < 1e-6 {
        return false;
    }
    let (wx, wy) = (a[0] - p[0], a[1] - p[1]);
    let t = (wx * ey - wy * ex) / den;
    let u = (wx * d[1] - wy * d[0]) / den;
    t > 0.0 && (0.0..=1.0).contains(&u)
}

#[cfg(test)]
mod tests {
    use crate::net::template;

    #[test]
    fn coverage_and_a_new_access_point_where_it_helps() {
        let mut n = template("roaming");
        n.devices.retain(|d| !d.kind.radio() || d.kind.foreign() || d.kind == crate::net::Kind::HomeRouter);
        let before = n.coverage(0).expect("walls");
        assert!(before.good < 1.0 && before.area_m2 > 20.0);
        let (pos, was, after) = n.suggest_ap(0).expect("a spot");
        assert!((was - before.good).abs() < 1e-3 && after > was, "{was} → {after}");
        n.add_device(crate::net::Kind::AccessPoint, pos);
        assert!(n.coverage(0).unwrap().good >= after - 0.01);
    }

    #[test]
    fn outside_the_walls_does_not_count() {
        let mut n = crate::net::Network::default();
        // Two 10 m rooms 30 m apart: the gap between them is outdoors.
        for x0 in [0.0, 800.0] {
            let (x1, y1) = (x0 + 200.0, 200.0);
            for (a, b) in [([x0, 0.0], [x1, 0.0]), ([x1, 0.0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, 0.0])] {
                n.walls.push(crate::wifi::Wall { id: n.walls.len() as u32, a, b, kind: crate::wifi::WallKind::Brick, floor: 0 });
            }
        }
        let c = n.coverage(0).unwrap();
        assert!((c.area_m2 - 200.0).abs() < 10.0, "{}", c.area_m2);
    }

    #[test]
    fn fix_all_repairs_the_nightmare_storm() {
        let mut n = template("nightmare");
        assert!(!n.fix_all().is_empty());
        assert!(!n.check().iter().any(|i| i.text.contains("broadcast storm")));
    }
}
