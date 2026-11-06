//! Planning help: how much of each storey (and each garden or terrace) has good Wi-Fi, where one more
//! access point would help most, what to buy to cover the home on a budget, and applying every one-click fix in one go.
use crate::catalog::Role;
use crate::net::{Device, Kind, Network, PX_PER_M};

/// How far below the plan's target (`Network::target`) a signal still counts as usable.
pub const USABLE_BELOW: f32 = 8.0;

/// What "good Wi-Fi" can mean, for the picker: (dBm, name).
pub const TARGETS: [(f32, &str); 3] = [(-72.0, "Browsing and music"), (-67.0, "Video calls (recommended)"), (-60.0, "4K streaming and gaming")];

/// Share of good Wi-Fi every storey must reach for the budget planner to call the home covered.
pub const COVERED: f32 = 0.9;

/// What the budget planner proposes: which product, how many and where, the price and the worst storey's good share.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub product: &'static crate::catalog::Product,
    pub kind: Kind,
    pub spots: Vec<(i8, [f32; 2])>,
    pub cost: f32,
    pub good: f32,
}

/// A rectangle outside the building (two opposite corners, map pixels) where Wi-Fi should reach.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Outdoor {
    pub id: u32,
    pub name: String,
    pub floor: i8,
    pub a: [f32; 2],
    pub b: [f32; 2],
}

#[derive(Clone)]
pub struct Coverage {
    pub good: f32,
    pub usable: f32,
    pub area_m2: f32,
}

/// Indoors, as the coverage score counts it: walls in all four directions around the spot.
pub fn surrounded(walls: &[&crate::wifi::Wall], p: [f32; 2]) -> bool {
    [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]].iter().all(|d| walls.iter().any(|w| ray_hits(p, *d, w.a, w.b)))
}

impl Network {
    /// Sample spots (cell centres) inside the walls of a storey, about 1 m apart (coarser for big buildings).
    /// A spot counts as indoors when walls surround it in all four directions.
    /// A courtyard closed on all sides looks indoors to that test: marking it as an outdoor area takes it out.
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
                if !self.in_outdoor(floor, [x, y]) && surrounded(&walls, [x, y]) {
                    v.push([x, y]);
                }
                x += cell;
            }
            y += cell;
        }
        (v, cell)
    }

    pub(crate) fn radios(&self) -> Vec<&Device> {
        self.devices.iter().filter(|d| d.kind.radio() && !d.kind.foreign() && !d.off && d.bands.iter().any(|b| *b)).collect()
    }

    fn best_at(&self, radios: &[&Device], p: [f32; 2], floor: i8) -> f32 {
        radios.iter().map(|r| self.best_rssi(r, p, floor)).fold(-120.0, f32::max)
    }

    /// Share of a storey with good and with usable Wi-Fi. None until the storey has walls. It scores quality, not raw
    /// signal: 2.4 GHz counts 6 dB less (slower and more crowded), as devices weigh it when picking a band.
    pub fn coverage(&self, floor: i8) -> Option<Coverage> {
        let (spots, cell) = self.spots(floor);
        if spots.is_empty() {
            return None;
        }
        let radios = self.radios();
        let s: Vec<f32> = spots.iter().map(|p| self.best_at(&radios, *p, floor)).collect();
        let share = |t: f32| s.iter().filter(|x| **x >= t).count() as f32 / s.len() as f32;
        Some(Coverage { good: share(self.target), usable: share(self.target - USABLE_BELOW), area_m2: s.len() as f32 * (cell / PX_PER_M).powi(2) })
    }

    /// Whether a spot lies in one of the storey's outdoor areas (garden, terrace).
    pub fn in_outdoor(&self, floor: i8, p: [f32; 2]) -> bool {
        self.outdoor.iter().any(|o| o.floor == floor && (o.a[0].min(o.b[0])..o.a[0].max(o.b[0])).contains(&p[0]) && (o.a[1].min(o.b[1])..o.a[1].max(o.b[1])).contains(&p[1]))
    }

    /// Good and usable share of an outdoor area, sampled about every metre (walls of the house in between count).
    pub fn outdoor_coverage(&self, o: &Outdoor) -> Coverage {
        let (x0, x1, y0, y1) = (o.a[0].min(o.b[0]), o.a[0].max(o.b[0]), o.a[1].min(o.b[1]), o.a[1].max(o.b[1]));
        let cell = PX_PER_M.max(((x1 - x0) * (y1 - y0) / 2500.0).sqrt());
        let radios = self.radios();
        let mut s = vec![];
        let mut y = y0 + cell / 2.0;
        while y < y1 {
            let mut x = x0 + cell / 2.0;
            while x < x1 {
                s.push(self.best_at(&radios, [x, y], o.floor));
                x += cell;
            }
            y += cell;
        }
        let share = |t: f32| if s.is_empty() { 0.0 } else { s.iter().filter(|x| **x >= t).count() as f32 / s.len() as f32 };
        Coverage { good: share(self.target), usable: share(self.target - USABLE_BELOW), area_m2: (x1 - x0) * (y1 - y0) / (PX_PER_M * PX_PER_M) }
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
        let weak: Vec<[f32; 2]> = spots.iter().zip(&now).filter(|(_, s)| **s < self.target).map(|(p, _)| *p).collect();
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
            let gained = weak.iter().filter(|p| self.best_rssi_raw(&ap, **p, floor) >= self.target).count();
            if best.is_none_or(|b| gained > b.1) {
                best = Some((*c, gained));
            }
        }
        let (pos, gained) = best?;
        (gained > 0).then(|| (pos, good_now, good_now + gained as f32 / spots.len() as f32))
    }

    /// What to buy to cover the home: the cheapest 1–4 catalog units (mesh nodes without cables, or wired access
    /// points) up to `budget` that give every storey with walls `COVERED` good Wi-Fi, else the best worst storey
    /// within the budget. The spots follow `suggest_ap`, one at a time on the storey that needs it most; the three
    /// cheapest products of each Wi-Fi generation are tried there. None if nothing fits the budget or helps.
    pub fn plan_budget(&self, budget: f32, mesh: bool) -> Option<Proposal> {
        let mut floors: Vec<i8> = self.walls.iter().map(|w| w.floor).collect();
        floors.sort();
        floors.dedup();
        // A light copy with just what coverage depends on (no pictures, no cables).
        let mut probe = Network {
            walls: self.walls.clone(),
            storey_heights: self.storey_heights.clone(),
            slab: self.slab,
            wall_fit: self.wall_fit.clone(),
            outdoor: self.outdoor.clone(),
            target: self.target,
            devices: self.radios().into_iter().cloned().collect(),
            next_id: self.next_id,
            ..Default::default()
        };
        let worst = |n: &Network| floors.iter().filter_map(|f| n.coverage(*f)).map(|c| c.good).fold(1.0f32, f32::min);
        let mut spots: Vec<(i8, [f32; 2])> = vec![];
        for _ in 0..4 {
            let need = floors.iter().filter_map(|f| probe.coverage(*f).map(|c| (*f, c.good))).min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((f, _)) = need.filter(|x| x.1 < COVERED) else { break };
            let Some((pos, ..)) = probe.suggest_ap(f) else { break };
            let id = probe.add_device(Kind::AccessPoint, pos);
            probe.dev_mut(id).unwrap().floor = f;
            spots.push((f, pos));
        }
        if spots.is_empty() {
            return None;
        }
        let kind = if mesh { Kind::Extender } else { Kind::AccessPoint };
        let fits = |p: &crate::catalog::Product| p.price > 0.0 && p.bands.iter().any(|b| *b) && if mesh { matches!(p.role, Role::Mesh | Role::Extender) } else { p.role == Role::Ap };
        let mut picks: Vec<&'static crate::catalog::Product> = vec![];
        for std in crate::net::WifiStd::ALL {
            let mut of: Vec<_> = crate::catalog::all().iter().filter(|p| p.wifi == std && fits(p)).collect();
            of.sort_by(|a, b| a.price.total_cmp(&b.price));
            picks.extend(of.into_iter().take(3));
        }
        let base: Vec<Device> = self.radios().into_iter().cloned().collect();
        // A new unit as the shelf adds it, before its product is applied.
        let fresh = {
            let mut n = Network::default();
            let id = n.add_device(kind, [0.0, 0.0]);
            n.dev(id).unwrap().clone()
        };
        let mut best: Option<Proposal> = None;
        for product in picks {
            for k in 1..=spots.len() {
                let cost = product.price * k as f32;
                if cost > budget {
                    break;
                }
                probe.devices = base.clone();
                for (i, (f, pos)) in spots[..k].iter().enumerate() {
                    let mut d = fresh.clone();
                    d.id = self.next_id + 1 + i as u32;
                    d.pos = *pos;
                    d.floor = *f;
                    product.apply(&mut d);
                    probe.devices.push(d);
                }
                let good = worst(&probe);
                let p = Proposal { product, kind, spots: spots[..k].to_vec(), cost, good };
                let better = match &best {
                    None => true,
                    Some(b) if (b.good >= COVERED) != (good >= COVERED) => good >= COVERED,
                    Some(b) if good >= COVERED => cost < b.cost || (cost == b.cost && good > b.good),
                    Some(b) => good > b.good + 0.005 || (good > b.good - 0.005 && cost < b.cost),
                };
                if better {
                    best = Some(p);
                }
                if good >= COVERED {
                    break;
                }
            }
        }
        best.filter(|b| b.good > worst(&Network { devices: base, ..probe }))
    }

    /// Adds what `plan_budget` proposed: mesh nodes join the router over Wi-Fi, access points get a cable to it.
    pub fn apply_proposal(&mut self, p: &Proposal) -> Vec<u32> {
        let router = self.devices.iter().find(|d| d.kind == Kind::HomeRouter).map(|d| d.id);
        let mut ids = vec![];
        for (f, pos) in &p.spots {
            let id = self.add_device(p.kind, *pos);
            let d = self.dev_mut(id).unwrap();
            d.floor = *f;
            p.product.apply(d);
            if let Some(r) = router {
                let medium = if p.kind == Kind::Extender { crate::net::Medium::WiFi } else { crate::net::Medium::Cat6 };
                let _ = self.connect(r, id, medium);
            }
            self.replug(id);
            ids.push(id);
        }
        ids
    }

    /// Apply every one-click fix the checks offer (fixes can reveal new ones, so a few passes), then
    /// hand out addresses again. Returns what was done: only fixes whose problem went away count.
    pub fn fix_all(&mut self) -> Vec<String> {
        let offered = |n: &Network| n.check().into_iter().filter_map(|i| i.fix).map(|f| (format!("{f:?}"), f)).collect::<Vec<_>>();
        let (mut done, mut stuck) = (vec![], std::collections::HashSet::new());
        for _ in 0..3 {
            let mut fixes = offered(self);
            // Several problems can offer the same fix (one PoE switch for three cameras): apply it once.
            // A fix that didn't help last pass isn't tried again.
            let mut seen = std::collections::HashSet::new();
            fixes.retain(|(k, _)| !stuck.contains(k) && seen.insert(k.clone()));
            if fixes.is_empty() {
                break;
            }
            let applied: Vec<(String, String)> = fixes.into_iter().map(|(k, f)| (k, self.apply_fix(&f))).collect();
            let still: std::collections::HashSet<String> = offered(self).into_iter().map(|(k, _)| k).collect();
            for (k, msg) in applied {
                if still.contains(&k) {
                    stuck.insert(k);
                } else {
                    done.push(msg);
                }
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
    fn a_stricter_target_lowers_the_score() {
        let mut n = template("home");
        let easy = { n.target = -72.0; n.coverage(0).unwrap().good };
        let strict = { n.target = -60.0; n.coverage(0).unwrap().good };
        assert!(easy > strict, "{easy} {strict}");
        // The Wi-Fi map's colours move with it: -66 dBm is "good" for video calls, not for gaming.
        assert_ne!(crate::radio_ui::heat_color(-66.0, -67.0), crate::radio_ui::heat_color(-66.0, -60.0));
    }

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
    fn a_budget_buys_the_cheapest_cover() {
        let mut n = template("roaming");
        n.devices.retain(|d| !d.kind.radio() || d.kind.foreign() || d.kind == crate::net::Kind::HomeRouter);
        let before = n.coverage(0).unwrap().good;
        for mesh in [true, false] {
            let p = n.plan_budget(2000.0, mesh).expect("something helps");
            assert!(p.good > before && p.cost <= 2000.0 && !p.spots.is_empty(), "{p:?}");
            // A tight budget never buys more than it allows.
            if let Some(q) = n.plan_budget(p.cost - 1.0, mesh) {
                assert!(q.cost < p.cost);
            }
            let mut m = n.clone();
            let ids = m.apply_proposal(&p);
            assert_eq!(ids.len(), p.spots.len());
            assert!((m.coverage(0).unwrap().good - p.good).abs() < 0.02, "{} vs {}", m.coverage(0).unwrap().good, p.good);
            assert!(ids.iter().all(|id| m.links.iter().any(|l| l.a == *id || l.b == *id)), "joined to the router");
        }
        assert!(n.plan_budget(1.0, true).is_none(), "nothing costs 1");
    }

    #[test]
    fn outside_the_walls_does_not_count() {
        let mut n = crate::net::Network::default();
        // Two 10 m rooms 30 m apart: the gap between them is outdoors.
        for x0 in [0.0, 800.0] {
            let (x1, y1) = (x0 + 200.0, 200.0);
            for (a, b) in [([x0, 0.0], [x1, 0.0]), ([x1, 0.0], [x1, y1]), ([x1, y1], [x0, y1]), ([x0, y1], [x0, 0.0])] {
                n.walls.push(crate::wifi::Wall { id: n.walls.len() as u32, a, b, kind: crate::wifi::WallKind::Brick, floor: 0, door: false });
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

    #[test]
    fn a_hungry_poe_device_gets_poe_plus_plus_and_the_problem_goes_away() {
        use crate::net::{Kind, Medium};
        let mut n = template("home");
        let sw = n.add_device(Kind::Switch, [300.0, 300.0]);
        let ap = n.add_device(Kind::AccessPoint, [340.0, 300.0]);
        n.connect(sw, ap, Medium::Cat6).unwrap();
        n.dev_mut(sw).unwrap().poe_class = 2;
        n.dev_mut(ap).unwrap().poe_draw = 45.0;
        let fix = n.check().into_iter().filter_map(|i| i.fix).find(|f| matches!(f, crate::issues::Fix::GivePoe(s, ..) if *s == sw)).expect("a PoE fix");
        assert!(matches!(fix, crate::issues::Fix::GivePoe(_, _, c) if c >= 3), "{fix:?}");
        n.apply_fix(&fix);
        assert!(n.poe_problems().iter().all(|p| p.0 != ap), "{:?}", n.poe_problems());
    }

    #[test]
    fn reconnect_moves_to_another_access_point_and_fix_all_counts_only_what_helped() {
        let mut n = template("roaming");
        for d in n.devices.iter().filter(|d| d.kind.wifi_client() && !d.kind.radio()).map(|d| d.id).collect::<Vec<_>>() {
            let Some(l) = n.wifi_link_of(d).cloned() else { continue };
            let Some((ap, _)) = n.wifi_ends(&l).map(|(a, c)| (a.id, c.id)) else { continue };
            n.reconnect(d);
            let now = n.wifi_link_of(d).cloned().unwrap();
            let moved = n.wifi_ends(&now).unwrap().0.id;
            assert!(moved != ap || n.reconnect(d).starts_with("No other"), "reconnect stayed on the same access point");
        }
        let before = n.check().iter().filter(|i| i.fix.is_some()).count();
        let done = n.fix_all();
        assert!(done.len() <= before.max(1) * 3, "{done:?}");
    }

    #[test]
    fn a_garden_next_to_the_router_has_better_wifi_than_one_far_away() {
        let n = template("home");
        let r = n.devices.iter().find(|d| d.kind == crate::net::Kind::HomeRouter).unwrap();
        let near = super::Outdoor { a: [r.pos[0] - 100.0, r.pos[1] + 60.0], b: [r.pos[0] + 100.0, r.pos[1] + 200.0], ..Default::default() };
        let far = super::Outdoor { a: [r.pos[0] + 2000.0, r.pos[1]], b: [r.pos[0] + 2200.0, r.pos[1] + 140.0], ..Default::default() };
        let (cn, cf) = (n.outdoor_coverage(&near), n.outdoor_coverage(&far));
        assert!(cn.usable > cf.usable && cf.good == 0.0 && (cn.area_m2 - 70.0).abs() < 1.0, "{} {}", cn.usable, cf.usable);
    }

    #[test]
    fn a_courtyard_marked_as_outdoor_leaves_the_indoor_score() {
        let mut n = template("home");
        let before = n.coverage(0).unwrap().area_m2;
        let ys: Vec<f32> = n.walls.iter().filter(|w| w.floor == 0).flat_map(|w| [w.a[1], w.b[1]]).collect();
        let xs: Vec<f32> = n.walls.iter().filter(|w| w.floor == 0).flat_map(|w| [w.a[0], w.b[0]]).collect();
        let (x0, y0) = (xs.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MAX, f32::min));
        n.outdoor.push(super::Outdoor { id: 1, name: "Courtyard".into(), floor: 0, a: [x0, y0], b: [x0 + 5.0 * crate::net::PX_PER_M, y0 + 4.0 * crate::net::PX_PER_M] });
        let after = n.coverage(0).unwrap().area_m2;
        assert!((before - after - 20.0).abs() < 6.0, "{before} → {after}");
    }
}