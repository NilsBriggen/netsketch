//! Window parts for the air and the wires: Wi-Fi map, walls, radio/adapter settings, spanning tree,
//! VLAN interfaces, the Wi-Fi tab with walks and roaming, and problem rows with fix buttons.
use super::*;
use crate::issues::Fix;
use crate::net::{Device, Iface};
use crate::wifi::{width_factor, Band, WallKind, CLIENT_PRESETS, RADIO_PRESETS};

/// Walking speed in the demo walk (px/s): about 3 m/s so a walk takes seconds, not minutes.
const WALK_SPEED: f32 = 3.0 * PX_PER_M;

pub struct Walk {
    pub pts: Vec<[f32; 2]>,
    pub t0: f64,
}

pub fn wall_color(k: WallKind) -> Color32 {
    match k {
        WallKind::Drywall => Color32::from_rgb(170, 178, 190),
        WallKind::Wood => Color32::from_rgb(180, 130, 80),
        WallKind::Glass => Color32::from_rgb(140, 200, 240),
        WallKind::LowE => Color32::from_rgb(56, 140, 200),
        WallKind::Brick => Color32::from_rgb(190, 90, 70),
        WallKind::Concrete => Color32::from_rgb(120, 126, 136),
        WallKind::Metal => Color32::from_rgb(200, 210, 225),
    }
}

/// Real wall thickness in map units (1 m = PX_PER_M), so walls keep their size when zooming in.
fn wall_width(k: WallKind) -> f32 {
    PX_PER_M
        * match k {
            WallKind::Glass | WallKind::LowE | WallKind::Metal => 0.05,
            WallKind::Drywall => 0.1,
            WallKind::Wood => 0.08,
            WallKind::Brick => 0.2,
            WallKind::Concrete => 0.25,
        }
}

/// Colour for a signal strength on the Wi-Fi map.
pub fn heat_color(rssi: f32) -> Option<Color32> {
    let c = |r, g, b, a| Some(Color32::from_rgba_unmultiplied(r, g, b, a));
    match rssi {
        r if r >= -55.0 => c(34, 197, 94, 70),
        r if r >= -65.0 => c(163, 230, 53, 60),
        r if r >= -72.0 => c(250, 204, 21, 55),
        r if r >= -80.0 => c(249, 115, 22, 50),
        r if r >= -86.0 => c(239, 68, 68, 40),
        _ => None,
    }
}

pub fn quality_color(rssi: f32) -> Color32 {
    match rssi {
        r if r >= -60.0 => pal().green,
        r if r >= -70.0 => Color32::from_rgb(163, 230, 53),
        r if r >= -77.0 => pal().amber,
        _ => pal().red,
    }
}

pub fn snap(v: Vec2) -> [f32; 2] {
    let s = PX_PER_M / 2.0;
    [(v.x / s).round() * s, (v.y / s).round() * s]
}

/// One row in a problem list: click the text to select, or press the fix. Returns (selected, fix).
pub fn issue_row(ui: &mut egui::Ui, i: &Issue) -> (bool, Option<Fix>) {
    let (icon, col) = match i.level {
        Level::Error => (ic::WARNING, pal().red),
        Level::Warning => (ic::WARNING, pal().amber),
        Level::Tip => (ic::INFO, pal().accent),
    };
    let mut fix = None;
    let clicked = ui
        .horizontal_wrapped(|ui| {
            ui.label(RichText::new(icon).color(col));
            let r = ui.add(egui::Label::new(&i.text).sense(Sense::click()));
            if let Some(f) = &i.fix {
                if ui.small_button(RichText::new(format!("{} {}", ic::WRENCH, f.label())).color(pal().accent)).clicked() {
                    fix = Some(f.clone());
                }
            }
            r.clicked()
        })
        .inner;
    (clicked, fix)
}

/// Search box over the built-in product catalog; picking a product copies its hardware onto the device and returns the device as it was.
pub(crate) fn catalog_picker(ui: &mut egui::Ui, d: &mut Device, query: &mut String) -> Option<Device> {
    const SHOWN: usize = 8;
    if let Some(p) = crate::catalog::find(&d.model) {
        ui.label(RichText::new(format!("{} {}", p.year, p.note)).small().color(pal().dim)).on_hover_text(trf("{}\nSpecs from {}", &[&(p.summary()), &p.source]));
    }
    let total = crate::catalog::search(d.kind, "").len();
    ui.add(TextEdit::singleline(query).hint_text(format!("{}  {}", ic::MAGNIFYING_GLASS, trf("Find a real model ({} in the catalog)", &[&total]))).desired_width(ui.available_width() - 8.0))
        .on_hover_text(tr("Type a brand or name — Swisscom, Internet-Box, FRITZ!Box, Deco, Orbi, UniFi… — and pick it to copy its radio, ports, power use and price"));
    if query.trim().is_empty() {
        return None;
    }
    let mut applied = None;
    let hits = crate::catalog::search(d.kind, query);
    if hits.is_empty() {
        ui.label(RichText::new(tr("No model matches. Try just the brand, or pick a generic type below.")).small().color(pal().dim));
    }
    for p in hits.iter().take(SHOWN) {
        let mut job = egui::text::LayoutJob::default();
        job.append(&p.title(), 0.0, egui::TextFormat { font_id: FontId::proportional(14.0), color: pal().text, ..Default::default() });
        job.append(&format!("\n{}", p.summary()), 0.0, egui::TextFormat { font_id: FontId::proportional(11.5), color: pal().dim, ..Default::default() });
        let r = ui.selectable_label(d.model == p.title(), job).on_hover_text(&p.note);
        if r.clicked() {
            applied = Some(d.clone());
            p.apply(d);
            query.clear();
        }
    }
    if hits.len() > SHOWN {
        ui.label(RichText::new(trf("…and {} more: type more of the name.", &[&(hits.len() - SHOWN)])).small().color(pal().dim));
    }
    ui.add_space(4.0);
    applied
}

fn walk_pos(pts: &[[f32; 2]], mut dist: f32) -> ([f32; 2], bool) {
    for w in pts.windows(2) {
        let len = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        if dist <= len && len > 0.0 {
            let t = dist / len;
            return ([w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t], false);
        }
        dist -= len;
    }
    (*pts.last().unwrap(), true)
}

impl App {
    // ---------- canvas layers ----------

    pub(crate) fn draw_heatmap(&mut self, painter: &egui::Painter, rect: Rect, now: f64) {
        if !self.net.devices.iter().any(|d| d.kind.radio()) {
            return;
        }
        // About 14 screen pixels per cell, never finer than 25 cm.
        let cell = (14.0 / self.zoom).max(PX_PER_M / 4.0).round();
        let (tl, br) = (self.to_world(rect.min), self.to_world(rect.max));
        let key = (self.floor, cell as i32, self.heat_band);
        let view = [(tl.x / cell).floor() as i32, (tl.y / cell).floor() as i32, (br.x / cell).ceil() as i32, (br.y / cell).ceil() as i32];
        let radios: Vec<&Device> = self.net.devices.iter().filter(|d| d.kind.radio() && !d.off && d.bands.iter().any(|b| *b)).collect();
        // Panning stays inside the area already worked out (a quarter view extra each side); twice a second the
        // radios and walls are compared, and only a change redraws.
        let inside = |c: &((i8, i32, usize), [i32; 4], Vec<(Rect, Color32)>, f64, u64)| c.0 == key && c.1[0] <= view[0] && c.1[1] <= view[1] && c.1[2] >= view[2] && c.1[3] >= view[3];
        let fingerprint = || {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_vec(&(&radios, &self.net.walls, self.net.slab, &self.net.storey_heights)).unwrap_or_default().hash(&mut h);
            h.finish()
        };
        let mut stamp = 0;
        let fresh = match self.heat_cache.as_mut() {
            Some(c) if inside(c) && now - c.3 < 0.5 => true,
            Some(c) if inside(c) => {
                stamp = fingerprint();
                c.3 = now;
                c.4 == stamp
            }
            _ => false,
        };
        if !fresh {
            if stamp == 0 {
                stamp = fingerprint();
            }
            let (mx, my) = ((view[2] - view[0]) / 4 + 1, (view[3] - view[1]) / 4 + 1);
            let area = [view[0] - mx, view[1] - my, view[2] + mx, view[3] + my];
            let mut cells = vec![];
            for cy in area[1]..area[3] {
                for cx in area[0]..area[2] {
                    let (x, y) = (cx as f32 * cell, cy as f32 * cell);
                    let p = [x + cell / 2.0, y + cell / 2.0];
                    let best = radios
                        .iter()
                        .map(|r| match self.heat_band {
                            0 => self.net.best_rssi(r, p, self.floor),
                            b => self.net.rssi_at(r, p, self.floor, crate::wifi::Band::ALL[b - 1]),
                        })
                        .fold(-120.0, f32::max);
                    if let Some(c) = heat_color(best) {
                        cells.push((Rect::from_min_size(pos2(x, y), Vec2::splat(cell)), c));
                    }
                }
            }
            self.heat_cache = Some((key, area, cells, now, stamp));
        }
        for (r, c) in &self.heat_cache.as_ref().unwrap().2 {
            let min = self.to_screen(r.min.to_vec2());
            painter.rect_filled(Rect::from_min_size(min, r.size() * self.zoom + Vec2::splat(0.6)), 0, *c);
        }
    }

    /// The Wi-Fi map colour key, drawn on top of everything at the bottom left.
    pub(crate) fn heat_legend(&self, painter: &egui::Painter, rect: Rect) {
        let items = [(-50.0, "excellent"), (-60.0, "good"), (-70.0, "fair"), (-76.0, "weak"), (-83.0, "barely")];
        // Narrow canvas: lift the legend above the zoom buttons so they don't collide.
        let lift = if rect.width() < 810.0 { 44.0 } else { 0.0 };
        let r = Rect::from_min_size(rect.left_bottom() + vec2(12.0, -40.0 - lift), vec2(if rect.width() < 810.0 { 330.0f32 } else { 470.0 }.min(rect.width() - 24.0), 28.0));
        painter.rect_filled(r, 8, pal().card.gamma_multiply(0.92));
        let mut x = r.left() + 10.0;
        // Narrow canvas: a short caption so the colours still fit.
        let caption = if rect.width() < 810.0 { tr("Signal:") } else { [tr("Wi-Fi map (best band):"), tr("Wi-Fi map (2.4 GHz):"), tr("Wi-Fi map (5 GHz):"), tr("Wi-Fi map (6 GHz):")][self.heat_band.min(3)] };
        let g = painter.layout_no_wrap(caption.into(), FontId::proportional(11.5), pal().dim);
        let w = g.size().x;
        painter.galley(pos2(x, r.center().y - g.size().y / 2.0), g, pal().dim);
        x += w + 12.0;
        for (v, t) in items {
            painter.rect_filled(Rect::from_center_size(pos2(x + 5.0, r.center().y), Vec2::splat(10.0)), 2, heat_color(v).unwrap().gamma_multiply(2.5));
            let g = painter.layout_no_wrap(tr(t).into(), FontId::proportional(11.0), pal().text);
            let w = g.size().x;
            painter.galley(pos2(x + 14.0, r.center().y - g.size().y / 2.0), g, pal().text);
            x += 14.0 + w + 12.0;
        }
        }

    /// Draws the walls; returns the one under the pointer.
    pub(crate) fn draw_walls(&self, painter: &egui::Painter, pointer: Option<Pos2>) -> Option<u32> {
        let mut hovered = None;
        for w in self.net.walls.iter().filter(|w| w.floor == self.floor) {
            let (a, b) = (self.to_screen(vec2(w.a[0], w.a[1])), self.to_screen(vec2(w.b[0], w.b[1])));
            let sel = self.sel == Sel::Wall(w.id);
            let width = (wall_width(w.kind) * self.zoom).max(2.0);
            // Thick walls (zoomed in) can be clicked anywhere on them, thin ones a little beside.
            let near = pointer.is_some_and(|p| dist_to_segment(p, a, b) < (width / 2.0 + 2.0).max(5.0));
            if near {
                hovered = Some(w.id);
            }
            if sel || (near && self.mode == Mode::Design) {
                painter.line_segment([a, b], Stroke::new(width + 6.0, pal().accent.gamma_multiply(if sel { 0.4 } else { 0.18 })));
            }
            painter.line_segment([a, b], Stroke::new(width, wall_color(w.kind)));
            if matches!(w.kind, WallKind::Glass | WallKind::LowE) {
                painter.line_segment([a, b], Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.5)));
            }
        }
        hovered
    }

    /// The wall tool: click to start, move to preview, click to place — each click continues the chain.
    /// Right-click, double-click or Esc ends it. Points lock onto wall corners and onto walls (T-joints).
    pub(crate) fn wall_tool(&mut self, resp: &egui::Response, painter: &egui::Painter, pointer: Option<Pos2>) {
        let ctx = resp.ctx.clone();
        let (clicked, ended, shift) = ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.secondary_clicked(), i.modifiers.shift));
        let Some(p) = pointer else { return };
        // About 12 screen pixels of "magnet", whatever the zoom.
        let tol = 12.0 / self.zoom;
        let (pt, kind) = snap_point(&self.net.walls, self.floor, self.to_world(p), tol, self.wall_start, shift, self.snap);
        let sp = self.to_screen(vec2(pt[0], pt[1]));
        match kind {
            Snap::Corner => {
                painter.circle_stroke(sp, 8.0, Stroke::new(2.0, pal().green));
            }
            Snap::OnWall => {
                painter.rect_stroke(Rect::from_center_size(sp, Vec2::splat(12.0)), 2, Stroke::new(2.0, pal().amber), StrokeKind::Middle);
            }
            Snap::Grid => {
                painter.circle_filled(sp, 3.0, pal().text);
            }
        }
        if let Some(a) = self.wall_start {
            let sa = self.to_screen(vec2(a[0], a[1]));
            painter.line_segment([sa, sp], Stroke::new((wall_width(self.wall_kind) * self.zoom).max(2.0), wall_color(self.wall_kind).gamma_multiply(0.75)));
            painter.circle_filled(sa, 4.0, wall_color(self.wall_kind));
            let len = ((pt[0] - a[0]).powi(2) + (pt[1] - a[1]).powi(2)).sqrt() / PX_PER_M;
            let ang = (pt[1] - a[1]).atan2(pt[0] - a[0]).to_degrees().rem_euclid(180.0);
            let g = painter.layout_no_wrap(format!("{len:.1} m · {ang:.0}°"), FontId::proportional(12.0), pal().text);
            let r = Rect::from_min_size(sp + vec2(12.0, -28.0), g.size() + vec2(10.0, 4.0));
            painter.rect_filled(r, 5, pal().card);
            painter.galley(r.min + vec2(5.0, 2.0), g, pal().text);
        } else {
            painter.text(sp + vec2(12.0, -12.0), Align2::LEFT_BOTTOM, tr("click to start a wall"), FontId::proportional(11.5), pal().dim);
        }
        if ended || resp.double_clicked() {
            self.wall_start = None;
            return;
        }
        if clicked && resp.hovered() {
            match self.wall_start {
                None => self.wall_start = Some(pt),
                Some(a) => {
                    let len = ((pt[0] - a[0]).powi(2) + (pt[1] - a[1]).powi(2)).sqrt() / PX_PER_M;
                    if len >= 0.25 {
                        self.snapshot();
                        crate::scenarios::wall(&mut self.net, a, pt, self.wall_kind);
                        self.net.walls.last_mut().unwrap().floor = self.floor;
                        self.heat_cache = None;
                        // Closing a room on its first corner ends the chain; otherwise keep going from here.
                        self.wall_start = if kind == Snap::Corner && self.net.walls.iter().filter(|w| w.floor == self.floor && (w.a == pt || w.b == pt)).count() >= 2 { None } else { Some(pt) };
                    }
                }
            }
        }
    }

    /// Drawing a cable's route: each click adds a point, snapped to wall corners and along walls like the wall tool.
    pub(crate) fn route_tool(&mut self, id: u32, resp: &egui::Response, painter: &egui::Painter, pointer: Option<Pos2>) {
        let ctx = resp.ctx.clone();
        let (clicked, ended, shift) = ctx.input(|i| (i.pointer.primary_clicked(), i.pointer.secondary_clicked(), i.modifiers.shift));
        let Some(last) = self.net.links.iter().find(|l| l.id == id).and_then(|l| l.path.last().copied().or(self.net.dev(l.a).map(|d| d.pos))) else {
            self.route = None;
            return;
        };
        if ended || resp.double_clicked() {
            self.route = None;
            return;
        }
        let Some(p) = pointer else { return };
        let (pt, _) = snap_point(&self.net.walls, self.floor, self.to_world(p), 12.0 / self.zoom, Some(last), shift, self.snap);
        let (sa, sp) = (self.to_screen(vec2(last[0], last[1])), self.to_screen(vec2(pt[0], pt[1])));
        painter.extend(Shape::dashed_line(&[sa, sp], Stroke::new(2.0, pal().accent), 6.0, 4.0));
        painter.circle_filled(sp, 4.0, pal().accent);
        painter.text(sp + vec2(12.0, -12.0), Align2::LEFT_BOTTOM, tr("click along the walls · right-click to finish"), FontId::proportional(11.5), pal().dim);
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        if clicked && resp.hovered() {
            self.snapshot();
            self.net.links.iter_mut().find(|l| l.id == id).unwrap().path.push(pt);
        }
    }

    // ---------- inspector parts ----------

    pub(crate) fn wall_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let mut delete = false;
        let Some(w) = self.net.walls.iter_mut().find(|w| w.id == id) else { return };
        let len = ((w.b[0] - w.a[0]).powi(2) + (w.b[1] - w.a[1]).powi(2)).sqrt() / PX_PER_M;
        Self::header(ui, ic::WALL, wall_color(w.kind), tr("Wall"), &format!("{} · {len:.1} m", w.kind.label()));
        ui.add_space(4.0);
        egui::Grid::new(("wall", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(tr("Material")).color(pal().dim));
            egui::ComboBox::from_id_salt(("wk", id)).selected_text(w.kind.label()).width(200.0).show_ui(ui, |ui| {
                for k in WallKind::ALL {
                    ui.selectable_value(&mut w.kind, k, k.label()).on_hover_text(k.note());
                }
            });
            ui.end_row();
            let (l24, l5) = w.kind.loss();
            ui.label(RichText::new(tr("Signal loss")).color(pal().dim)).on_hover_text(tr("Every 10 dB lost leaves a tenth of the signal; 3 dB is half."));
            ui.label(trf("−{} dB at 2.4 GHz · −{} dB at 5 GHz", &[&format!("{:.0}", l24), &format!("{:.0}", l5)])).on_hover_text(w.kind.note());
            ui.end_row();
            ui.label(RichText::new(tr("Storey")).color(pal().dim));
            ui.add(egui::DragValue::new(&mut w.floor).range(-3..=20).custom_formatter(|v, _| crate::net::floor_name(v as i8)));
            ui.end_row();
        });
        // Exact length: the far end moves along the wall (and takes walls joined there with it).
        let (a, b, floor) = (w.a, w.b, w.floor);
        let mut want = len;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Length")).color(pal().dim));
            if ui.add(egui::DragValue::new(&mut want).range(0.2..=200.0).speed(0.05).suffix(" m")).on_hover_text(tr("Type the measured length; the end you drew last moves")).changed() && len > 0.0 {
                let f = want / len;
                let to = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
                move_corner(&mut self.net.walls, floor, b, to);
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Put in")).color(pal().dim));
            for (label, kind, width) in [(tr("Door"), Some(WallKind::Wood), 0.9), (tr("Window"), Some(WallKind::Glass), 1.2), (tr("Opening"), None, 0.9)] {
                if ui.small_button(label).on_hover_text(trf("A {} m {} in the middle of this wall", &[&width, &(label.to_lowercase())])).clicked() {
                    self.snapshot();
                    let msg = insert_opening(&mut self.net, id, kind, width);
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, msg);
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.small_button(RichText::new(format!("{}  {}", ic::TRASH, tr("Remove wall"))).color(pal().red)).clicked() {
                delete = true;
            }
            if ui.small_button(RichText::new(format!("{}  {}", ic::TRASH, tr("Remove joined walls"))).color(pal().red)).on_hover_text(tr("This wall and every wall connected to it at a corner")).clicked() {
                self.snapshot();
                let chain = joined_walls(&self.net.walls, id);
                self.net.walls.retain(|w| !chain.contains(&w.id));
                self.sel = Sel::None;
                self.heat_cache = None;
            }
        });
        if delete {
            self.snapshot();
            self.net.walls.retain(|w| w.id != id);
            self.sel = Sel::None;
        }
    }

    /// Handles on the selected wall's ends (Move tool): drag one and every wall joined there follows.
    pub(crate) fn wall_handles(&mut self, ui: &egui::Ui, painter: &egui::Painter) {
        let Sel::Wall(id) = self.sel else { return };
        let Some(w) = self.net.walls.iter().find(|w| w.id == id).cloned() else { return };
        for (i, end) in [w.a, w.b].into_iter().enumerate() {
            let c = self.to_screen(vec2(end[0], end[1]));
            painter.circle_filled(c, 6.0, pal().bg);
            painter.circle_stroke(c, 6.0, Stroke::new(2.0, pal().accent));
            let resp = ui.interact(Rect::from_center_size(c, Vec2::splat(16.0)), Id::new(("wall_end", id, i)), Sense::drag());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Move);
            }
            if resp.drag_started() {
                self.snapshot();
            }
            if resp.dragged() {
                let d = resp.drag_delta() / self.zoom;
                move_corner(&mut self.net.walls, w.floor, end, [end[0] + d.x, end[1] + d.y]);
                self.heat_cache = None;
            }
            if resp.drag_stopped() && self.snap {
                let now = if i == 0 { self.net.walls.iter().find(|x| x.id == id).map(|x| x.a) } else { self.net.walls.iter().find(|x| x.id == id).map(|x| x.b) };
                if let Some(now) = now {
                    move_corner(&mut self.net.walls, w.floor, now, snap(vec2(now[0], now[1])));
                }
            }
        }
    }

    /// Router, access point, extender and neighbour radios: hardware, bands, channels, widths, power.
    pub(crate) fn radio_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let self_targets: Vec<(u32, String)> = self.net.devices.iter().filter(|x| x.kind.radio() || x.kind.is_host()).map(|x| (x.id, x.name.clone())).collect();
        // Whatever was changed last frame (standard, a band switched on…) gets sensible values first.
        let linked = self.net.links.iter().find(|l| l.a == id || l.b == id).map(|l| l.other(id).0);
        let d = self.net.dev_mut(id).unwrap();
        crate::wifi::normalize_radio(d);
        let kind = d.kind;
        if kind == Kind::Interference {
            if !Self::section_hint(ui, "What it is", "It doesn't wait its turn like Wi-Fi does, so it causes collisions on overlapping 2.4 GHz channels nearby. 5 and 6 GHz are unaffected.") {
                return;
            }
            egui::ComboBox::from_id_salt(("dist", id)).selected_text(d.model.clone()).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for (i, x) in crate::wifi::DISTURBERS.iter().enumerate() {
                    if ui.selectable_label(d.model == x.0, x.0).clicked() {
                        crate::wifi::apply_disturber(d, i);
                    }
                }
            });
            egui::Grid::new(("dist_g", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                ui.label(tr("Around 2.4 GHz channel"));
                ui.add(egui::DragValue::new(&mut d.ch[0]).range(1..=13));
                ui.end_row();
                ui.label(tr("Active"));
                ui.add(egui::Slider::new(&mut d.busy, 0.0..=1.0).custom_formatter(|v, _| trf("{}% of the time", &[&format!("{:.0}", v * 100.0)])));
                ui.end_row();
                ui.label(tr("Leaks about"));
                ui.add(egui::DragValue::new(&mut d.tx[0]).range(-30.0..=20.0).speed(0.2).suffix(" dBm"));
                ui.end_row();
            });
            return;
        }
        let speaker = kind == Kind::Speaker;
        if Self::section(ui, if kind == Kind::Neighbor { "Their Wi-Fi" } else if speaker { "Speaker mesh radio" } else { "Wi-Fi hardware" }) {
        if !speaker {
            if let Some(old) = catalog_picker(ui, d, &mut self.catalog_query) {
                self.before = Some(crate::Before::Device(old));
                self.replug = Some(id);
            }
        }
        egui::ComboBox::from_id_salt(("preset", id)).selected_text(if d.model.is_empty() { tr("Custom").to_string() } else { d.model.clone() }).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
            // Speakers have their own radio; routers and access points can't be a speaker.
            for p in RADIO_PRESETS.iter().filter(|p| p.name.starts_with("Multi-room speaker") == speaker) {
                if ui.selectable_label(d.model == p.name, p.name).clicked() {
                    self.before = Some(crate::Before::Device(d.clone()));
                    p.apply(d);
                }
            }
        });
        egui::Grid::new(("radio", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            if !speaker {
                ui.label(tr("Name (SSID)"));
                ui.text_edit_singleline(&mut d.ssid);
                ui.end_row();
            }
            ui.label(tr("Standard"));
            egui::ComboBox::from_id_salt(("std", id)).selected_text(d.wifi.label()).width(210.0).show_ui(ui, |ui| {
                for s in WifiStd::ALL {
                    ui.selectable_value(&mut d.wifi, s, s.label());
                }
            });
            ui.end_row();
            if kind != Kind::Neighbor && !speaker {
                ui.label(tr("Security"));
                egui::ComboBox::from_id_salt(("sec", id)).selected_text(d.security.label()).width(210.0).show_ui(ui, |ui| {
                    for s in Security::ALL {
                        ui.selectable_value(&mut d.security, s, s.label());
                    }
                });
                ui.end_row();
            }
            ui.label(tr("Antennas"));
            ui.add(egui::Slider::new(&mut d.streams, 1..=4).suffix(tr(" streams"))).on_hover_text(tr("Antenna streams (MIMO): each one adds a lane of speed"));
            ui.end_row();
        });
        }
        if Self::section_hint(ui, "Bands", if speaker { "All speakers of one system share the mesh channel. On 2.4 GHz it competes with your Wi-Fi." } else { "Width: wider is faster but collides with more neighbours. Power: the EU limit is shown when you hover the value. Radar (DFS) channels on 5 GHz may have to be left when radar shows up." }) {
        egui::Grid::new(("bands", id)).num_columns(4).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            for h in ["", tr("Width"), tr("Channel"), tr("Power")] {
                ui.label(RichText::new(tr(h)).small().color(pal().dim));
            }
            ui.end_row();
            for b in Band::ALL {
                let i = b.i();
                let allowed = d.wifi.max_width(i);
                ui.add_enabled(allowed > 0, egui::Checkbox::new(&mut d.bands[i], b.label()));
                if d.bands[i] && allowed > 0 {
                    egui::ComboBox::from_id_salt(("w", id, i)).selected_text(format!("{} MHz", d.width[i])).width(80.0).show_ui(ui, |ui| {
                        for w in b.widths().iter().filter(|w| **w <= allowed) {
                            ui.selectable_value(&mut d.width[i], *w, format!("{w} MHz"));
                        }
                    });
                    let chans = b.channels(d.width[i]);
                    if !chans.contains(&d.ch[i]) {
                        d.ch[i] = chans.first().copied().unwrap_or(d.ch[i]);
                    }
                    egui::ComboBox::from_id_salt(("c", id, i)).selected_text(d.ch[i].to_string()).width(64.0).show_ui(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                            for c in chans {
                                let dfs = crate::wifi::dfs(b, c, d.width[i]);
                                ui.selectable_value(&mut d.ch[i], c, if dfs { format!("{c} (radar/DFS)") } else { c.to_string() });
                            }
                        });
                    });
                    let limit = b.eirp_limit(d.ch[i]);
                    let over = d.tx[i] > limit;
                    ui.add(egui::DragValue::new(&mut d.tx[i]).range(0.0..=30.0).speed(0.2).suffix(" dBm")).on_hover_text(trf("EU limit on {} channel {}: {} dBm{}", &[&(b.label()), &(d.ch[i]), &format!("{:.0}", limit), &(if over { tr(" — above it!") } else { "" })]));
                } else {
                    ui.label(RichText::new(if allowed == 0 { tr("not with this standard") } else { "off" }).small().color(pal().dim));
                    ui.label("");
                    ui.label("");
                }
                ui.end_row();
            }
        });
        let top: Vec<String> = Band::ALL
            .iter()
            .filter(|b| d.bands[b.i()] && d.wifi.max_width(b.i()) > 0)
            .map(|b| format!("{} {:.0}", b.label(), d.wifi.stream20(b.i()) * width_factor(d.width[b.i()].min(d.wifi.max_width(b.i()))) * d.streams as f32))
            .collect();
        ui.label(RichText::new(trf("Top rate {} Mbit/s", &[&(top.join(" · "))])).small().color(pal().dim)).on_hover_text(tr("Link rate on paper; real throughput is about 60% of it, shared by everyone on the channel."));
        }
        if kind == Kind::Neighbor {
            ui.add(egui::Slider::new(&mut d.busy, 0.0..=1.0).text(tr("of the airtime they use")).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
            return;
        }
        if speaker {
            return;
        }
        if Self::section(ui, "Antenna") {
        let mut directional = d.beam > 0.0;
        if ui.checkbox(&mut directional, tr("Directional")).on_hover_text(tr("A panel or dish: strong one way, weak elsewhere. Links between buildings use a pair aimed at each other.")).changed() {
            d.beam = if directional { 30.0 } else { 0.0 };
            // Start pointed at whatever it's connected to.
            if directional {
                if let Some(t) = linked {
                    self.net.aim_at(id, t);
                }
            }
        }
        let d = self.net.dev_mut(id).unwrap();
        let mut aim_target = None;
        if d.beam > 0.0 {
            ui.add(egui::Slider::new(&mut d.beam, 5.0..=120.0).text(tr("beam width")).suffix("°"));
            ui.add(egui::Slider::new(&mut d.aim, 0.0..=359.0).text(tr("points to")).suffix("°"));
            let me = id;
            let others: Vec<(u32, String)> = self_targets.iter().filter(|t| t.0 != me).cloned().collect();
            egui::ComboBox::from_id_salt(("aim", id)).selected_text(tr("Aim at…")).width(200.0).show_ui(ui, |ui| {
                for (t, name) in &others {
                    if ui.selectable_label(false, name).clicked() {
                        aim_target = Some(*t);
                    }
                }
            });
            ui.label(RichText::new(trf("≈ +{} dB towards where it points", &[&format!("{:.0}", (10.0 * (360.0 / d.beam).log10()).min(24.0))])).small().color(pal().dim));
        }
        if let Some(t) = aim_target {
            self.net.aim_at(id, t);
        }
        }
        let d = self.net.dev_mut(id).unwrap();
        if !Self::section(ui, "Behaviour") {
            return;
        }
        ui.checkbox(&mut d.band_steer, tr("Band steering")).on_hover_text(tr("Pushes devices to the faster band"));
        ui.checkbox(&mut d.fast_roam, tr("Fast roaming (802.11k/v/r)")).on_hover_text(tr("Quick, well-timed hand-overs between access points"));
        if d.wifi >= WifiStd::Wifi7 {
            ui.checkbox(&mut d.mlo, tr("Multi-Link (MLO)")).on_hover_text(tr("Wi-Fi 7 devices with Multi-Link use two bands at once: their speeds add up, and if one band drops the other carries on without a gap"));
        }
        let mut kick = d.min_rssi != 0;
        ui.horizontal(|ui| {
            if ui.checkbox(&mut kick, tr("Drop clients weaker than")).on_hover_text(tr("Pushes far-away devices off so they look for a closer access point")).changed() {
                d.min_rssi = if kick { -75 } else { 0 };
            }
            if kick {
                ui.add(egui::DragValue::new(&mut d.min_rssi).range(-95..=-55).suffix(" dBm"));
            }
        });
        if matches!(kind, Kind::AccessPoint | Kind::Extender) {
            ui.checkbox(&mut d.backhaul, tr("Separate backhaul radio")).on_hover_text(tr("Tri-band mesh or cross-band: no halving of speed when it's fed over Wi-Fi"));
        }
    }

    /// A client's Wi-Fi adapter: what the device itself can do.
    pub(crate) fn adapter_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let d = self.net.dev_mut(id).unwrap();
        // A Thread or Zigbee gadget has no Wi-Fi to set up.
        if d.on_802154() {
            return;
        }
        crate::wifi::normalize_radio(d);
        if Self::section_hint(ui, "Wi-Fi adapter", "Phones send weaker than routers (≈ 15 dBm vs 20–23): the router may be heard where its answer from the phone isn't.") {
            egui::ComboBox::from_id_salt(("cpreset", id)).selected_text(if d.model.is_empty() { "Choose a typical device…".to_string() } else { d.model.clone() }).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for p in CLIENT_PRESETS {
                    if ui.selectable_label(d.model == p.name, p.name).clicked() {
                        self.before = Some(crate::Before::Device(d.clone()));
                        p.apply(d);
                    }
                }
            });
            egui::Grid::new(("ad", id)).num_columns(3).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                ui.label(tr("Standard"));
                egui::ComboBox::from_id_salt(("cstd", id)).selected_text(d.wifi.label()).width(200.0).show_ui(ui, |ui| {
                    for s in WifiStd::ALL {
                        ui.selectable_value(&mut d.wifi, s, s.label());
                    }
                });
                ui.end_row();
                ui.label(tr("Antennas"));
                ui.add(egui::Slider::new(&mut d.streams, 1..=4));
                ui.end_row();
                if d.wifi >= WifiStd::Wifi7 {
                    ui.label(tr("Multi-Link"));
                    ui.checkbox(&mut d.mlo, tr("MLO")).on_hover_text(tr("Wi-Fi 7 devices with Multi-Link use two bands at once: their speeds add up, and if one band drops the other carries on without a gap"));
                    ui.end_row();
                }
                for b in Band::ALL {
                    let i = b.i();
                    ui.checkbox(&mut d.bands[i], b.label());
                    if d.bands[i] {
                        ui.add(egui::DragValue::new(&mut d.tx[i]).range(0.0..=25.0).speed(0.2).suffix(" dBm"));
                    }
                    ui.end_row();
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr("Roams below"));
                ui.add(egui::DragValue::new(&mut d.roam_at).range(-90..=-55).suffix(" dBm")).on_hover_text(tr("Signal at which it starts looking for a better access point"));
            });
        }
    }

    pub(crate) fn stp_editor(&mut self, ui: &mut egui::Ui, id: u32, root: Option<u32>, cost: Option<u32>) {
        let d = self.net.dev_mut(id).unwrap();
        if !Self::section_hint(ui, "Spanning tree", "Spanning tree (STP/RSTP) finds loops between switches and keeps one link on standby; the switch with the lowest bridge ID becomes the root.") {
            return;
        }
        ui.checkbox(&mut d.stp, tr("Run spanning tree")).on_hover_text(tr("STP/RSTP — blocks loops"));
        if d.stp {
            ui.horizontal(|ui| {
                ui.label(tr("Priority"));
                egui::ComboBox::from_id_salt(("prio", id)).selected_text(d.priority.to_string()).width(110.0).show_ui(ui, |ui| {
                    for p in (0..=61440u32).step_by(4096) {
                        ui.selectable_value(&mut d.priority, p as u16, format!("{p}{}", if p == 32768 { " (default)" } else if p == 4096 { " (core)" } else { "" }));
                    }
                });
            });
            ui.checkbox(&mut d.stp_short, tr("Old short path-cost scale (802.1D-1998)")).on_hover_text(tr("1 Gbit/s costs 4 instead of 20000. Mixed with modern switches, paths through this device look far too cheap."));
            let mac = d.ifaces.first().map(|f| f.mac.clone()).unwrap_or_default();
            ui.label(RichText::new(trf("Bridge ID {}.{}", &[&d.priority, &mac])).small().color(pal().dim)).on_hover_text(tr("Lowest wins the root election"));
        }
        if root == Some(id) {
            ui.label(RichText::new(format!("{}  {}", ic::CROWN, tr("Root bridge: every path is measured from here."))).color(pal().amber));
        } else if let Some(c) = cost {
            ui.label(RichText::new(trf("Path cost to the root: {}", &[&c])).small().color(pal().dim));
        }
    }

    /// VLAN sub-interfaces of a router (router-on-a-stick).
    pub(crate) fn vlan_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let mut remove = None;
        let mut add = false;
        {
            let d = self.net.dev(id).unwrap();
            let subs: Vec<usize> = (0..d.ifaces.len()).filter(|i| d.ifaces[*i].parent.is_some()).collect();
            if !Self::section_hint(ui, "VLANs", "Each VLAN interface is a separate network riding on one cable (a trunk). Devices pick their VLAN in their own settings.") {
                return;
            }
            let d = self.net.dev_mut(id).unwrap();
            egui::CollapsingHeader::new(format!("VLAN interfaces ({})", subs.len())).id_salt(("vl", id)).default_open(subs.len() <= 8).show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(240.0).id_salt(("vls", id)).show(ui, |ui| {
                    egui::Grid::new(("vg", id)).num_columns(4).spacing(vec2(6.0, 4.0)).striped(true).show(ui, |ui| {
                        for i in subs {
                            let f = &mut d.ifaces[i];
                            ui.label(RichText::new(&f.name).monospace().small().color(pal().dim));
                            ui.add(egui::DragValue::new(&mut f.vlan).range(1..=4094).prefix("VLAN "));
                            ui.horizontal(|ui| {
                                ui.add(TextEdit::singleline(&mut f.ip).desired_width(100.0).font(egui::TextStyle::Monospace));
                                ui.label(tr("/"));
                                ui.add(egui::DragValue::new(&mut f.prefix).range(8..=30));
                            });
                            if ui.small_button(ic::X).on_hover_text(tr("Remove this VLAN interface")).clicked() {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                });
            });
            let has_port = !d.ifaces.is_empty();
            if ui.add_enabled(has_port, egui::Button::new(format!("{}  {}", ic::PLUS, tr("Add VLAN interface")))).on_disabled_hover_text(tr("Connect a cable first: VLAN interfaces ride on a port")).clicked() {
                add = true;
            }
        }
        if add {
            let used: Vec<usize> = self.net.links.iter().filter_map(|l| if l.a == id { Some(l.a_if) } else if l.b == id { Some(l.b_if) } else { None }).collect();
            let d = self.net.dev_mut(id).unwrap();
            let Some(port) = (0..d.ifaces.len()).find(|i| d.ifaces[*i].parent.is_none() && used.contains(i)).or_else(|| (0..d.ifaces.len()).find(|i| d.ifaces[*i].parent.is_none())) else {
                return;
            };
            let v = d.ifaces.iter().map(|f| f.vlan).filter(|v| *v > 1).max().map(|v| v + 1).unwrap_or(10).min(4094);
            let parent = d.ifaces[port].clone();
            d.ifaces.push(Iface { name: format!("{}.{v}", parent.name.split_whitespace().next().unwrap_or("eth")), ip: crate::scenarios::vlan_ip(v), prefix: 24, mac: parent.mac, vlan: v, parent: Some(port) });
        }
        if let Some(i) = remove {
            // Keep cable and parent references pointing at the right ports.
            self.net.dev_mut(id).unwrap().ifaces.remove(i);
            for f in self.net.dev_mut(id).unwrap().ifaces.iter_mut() {
                if let Some(p) = f.parent.as_mut() {
                    if *p > i {
                        *p -= 1;
                    }
                }
            }
            for l in self.net.links.iter_mut() {
                if l.a == id && l.a_if > i {
                    l.a_if -= 1;
                }
                if l.b == id && l.b_if > i {
                    l.b_if -= 1;
                }
            }
        }
    }

    // ---------- simulate: Wi-Fi tab, walks and roaming ----------

    pub(crate) fn start_walk(&mut self, id: u32, now: f64) {
        self.remember_home(id);
        let Some(d) = self.net.dev(id) else { return };
        let ssid = self.net.wifi_link_of(id).and_then(|l| self.net.wifi_ends(l)).map(|(ap, _)| ap.ssid.clone()).unwrap_or_default();
        let start = d.pos;
        let mut stops: Vec<[f32; 2]> = self.net.devices.iter().filter(|r| r.kind.radio() && (ssid.is_empty() || r.ssid == ssid)).map(|r| [r.pos[0] + 40.0, r.pos[1] + 60.0]).collect();
        stops.sort_by(|a, b| {
            let da = (a[0] - start[0]).powi(2) + (a[1] - start[1]).powi(2);
            let db = (b[0] - start[0]).powi(2) + (b[1] - start[1]).powi(2);
            da.total_cmp(&db)
        });
        if stops.len() < 2 {
            stops.push([start[0] + 30.0 * PX_PER_M, start[1]]);
        }
        let mut pts = vec![start];
        pts.extend(stops);
        pts.push(start);
        self.walks.insert(id, Walk { pts, t0: now });
    }

    /// Moves walkers, runs roaming for moved devices and samples the selected device's signal.
    pub(crate) fn tick(&mut self, ctx: &egui::Context) {
        let now = Self::now(ctx);
        if self.mode != Mode::Simulate {
            self.walks.clear();
            if !self.sim_home.is_empty() {
                self.restore_home(None);
            }
            return;
        }
        let ids: Vec<u32> = self.walks.keys().copied().collect();
        for id in ids {
            let (pos, done) = {
                let w = &self.walks[&id];
                walk_pos(&w.pts, (now - w.t0) as f32 * WALK_SPEED)
            };
            if let Some(d) = self.net.dev_mut(id) {
                d.pos = pos;
            }
            if done {
                self.walks.remove(&id);
                self.say(ctx, trf("{} is back where it started.", &[&(self.net.name(id))]));
            }
            self.after_move(ctx, id);
        }
        if let Sel::Dev(id) = self.sel {
            if now - self.last_sample > 0.25 {
                self.sample(id, now, false);
                self.last_sample = now;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if !self.walks.is_empty() {
            ctx.request_repaint();
        }
    }

    /// After a device moved in Simulate mode: let it roam and record what happened.
    pub(crate) fn after_move(&mut self, ctx: &egui::Context, id: u32) {
        let now = Self::now(ctx);
        let Some(r) = self.net.roam_step(id) else { return };
        let (name, from, to) = (self.net.name(id), self.net.name(r.from), self.net.name(r.to));
        let how = if r.fast {
            tr("802.11r fast transition — a call wouldn't notice").to_string()
        } else if r.lost {
            tr("after losing the signal completely — everything stalled").to_string()
        } else {
            tr("full re-login — a video call freezes for a moment").to_string()
        };
        let mut info = trf("{} hands over {} → {}: {} → {} dBm in {} ms ({})", &[&name, &from, &to, &format!("{:.0}", r.before), &format!("{:.0}", r.after), &format!("{:.0}", r.ms), &how]);
        if r.kicked {
            info += tr(", pushed off by the minimum-signal rule");
        }
        let mut logs = vec![
            net::Log { src: name.clone(), dst: to.clone(), proto: "802.11", layer: tr("Link"), info: info.clone(), dev: id, ..Default::default() },
        ];
        if !r.same_lan {
            logs.push(net::Log { src: name.clone(), dst: to.clone(), proto: "802.11", layer: tr("Link"), info: tr("New network behind this access point: new address, open connections break").into(), dev: id, ..Default::default() });
            self.net.dhcp_one(id, &mut logs);
        }
        self.push_logs(logs);
        self.terms.entry(id).or_default().lines.push(format!("[Wi-Fi] {info}"));
        self.sample(id, now, true);
        self.say(ctx, trf("{} roamed to {} ({} ms)", &[&name, &to, &format!("{:.0}", r.ms)]));
    }

    fn sample(&mut self, id: u32, now: f64, roam: bool) {
        let Some(s) = self.net.wifi_link_of(id).and_then(|l| self.net.link_signal(l)) else { return };
        let h = self.hist.entry(id).or_default();
        h.push((now, s.rssi, roam));
        h.retain(|x| x.0 > now - 45.0);
    }

    pub(crate) fn wifi_tab(&mut self, ui: &mut egui::Ui, id: u32) {
        let now = Self::now(ui.ctx());
        let d = self.net.dev(id).unwrap().clone();
        if let Some(l) = self.net.wifi_link_of(id).cloned() {
            let (ap, _) = self.net.wifi_ends(&l).map(|(a, c)| (a.clone(), c.clone())).unwrap();
            let Some(s) = self.net.link_signal(&l) else {
                ui.label(RichText::new(trf("{} and {} have no Wi-Fi band in common (check both sides' bands), so they can't talk.", &[&d.name, &ap.name])).color(pal().red));
                return;
            };
            ui.horizontal(|ui| {
                let bars = match s.rssi {
                    r if r >= -60.0 => ic::WIFI_HIGH,
                    r if r >= -72.0 => ic::WIFI_MEDIUM,
                    r if r >= -84.0 => ic::WIFI_LOW,
                    _ => ic::WIFI_SLASH,
                };
                ui.label(RichText::new(bars).size(34.0).color(quality_color(s.rssi)));
                ui.vertical(|ui| {
                    ui.label(RichText::new(trf("{} dBm · {}", &[&format!("{:.0}", s.rssi), &(s.quality())])).size(20.0).strong().color(quality_color(s.rssi)));
                    let on_guest = d.ifaces.first().is_some_and(|f| f.vlan == crate::extras::GUEST_VLAN);
                    let ssid = if on_guest && !ap.guest_ssid.is_empty() { &ap.guest_ssid } else { &ap.ssid };
                    ui.label(RichText::new(trf("via {} · '{}'", &[&ap.name, &ssid])).color(pal().dim));
                });
            });
            egui::Grid::new(("wifi_tab", id)).num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                let row = |ui: &mut egui::Ui, k: &str, v: String| {
                    ui.label(RichText::new(k).color(pal().dim));
                    ui.label(v);
                    ui.end_row();
                };
                row(ui, "Wi-Fi link", if s.alive() { format!("≈ {:.0} Mbit/s", s.mbps) } else { "no connection".into() });
                if let Some((e, why)) = self.e2e.get(&id).filter(|_| s.alive()) {
                    let real = e.min(s.mbps);
                    ui.label(RichText::new(tr("To the internet")).color(pal().dim));
                    let r = ui.label(RichText::new(trf("≈ {} Mbit/s", &[&format!("{:.0}", real)])).strong());
                    if real < s.mbps * 0.95 {
                        r.on_hover_text(trf("Limited by {}", &[&why]));
                    }
                    ui.end_row();
                }
                row(ui, tr("Band"), trf("{} · channel {} · {} MHz", &[&(s.band.label()), &(self.net.channel(&ap, s.band)), &s.width]));
                row(ui, tr("Antennas"), trf("{}×{} streams ({} on both sides)", &[&s.streams, &s.streams, &s.streams]));
                row(ui, tr("Answer back"), trf("{} dBm at the access point", &[&format!("{:.0}", s.up)]));
                row(ui, tr("Walls"), trf("{} in the way", &[&s.walls]));
                row(ui, tr("Airtime"), if s.contention > 1.05 { trf("shared ×{} with other networks", &[&format!("{:.1}", s.contention)]) } else { tr("channel to itself").into() });
                let th = crate::wifi::roam_threshold(&ap, &d);
                row(ui, tr("Roaming"), if ap.fast_roam { trf("fast (802.11k/v/r) — looks around below {} dBm", &[&format!("{:.0}", th)]) } else { trf("basic — this device hangs on until {} dBm", &[&format!("{:.0}", th)]) });
            });
        } else if d.kind.transmits() {
            let clients: Vec<(String, f32, f32)> = self
                .net
                .links
                .iter()
                .filter_map(|l| {
                    let (ap, cl) = self.net.wifi_ends(l)?;
                    let s = self.net.link_signal(l)?;
                    (ap.id == id).then(|| (cl.name.clone(), s.rssi, s.mbps))
                })
                .collect();
            if !d.kind.foreign() && Self::section(ui, "Connected devices") {
                if clients.is_empty() {
                    ui.label(RichText::new(tr("Nobody connected.")).color(pal().dim));
                }
                for (n, r, m) in clients {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(trf("{} dBm", &[&format!("{:>4.0}", r)])).monospace().color(quality_color(r)));
                        ui.label(trf("{} — ≈ {} Mbit/s", &[&n, &format!("{:.0}", m)]));
                    });
                }
            }
            if Self::section(ui, if d.kind.foreign() { "Overlaps with" } else { "Channels" }) {
            for b in Band::ALL.into_iter().filter(|b| d.bands[b.i()]) {
                let (c, who) = self.net.contention(&d, b);
                let names: Vec<String> = who.iter().map(|w| self.net.name(*w)).collect();
                let with = if who.is_empty() {
                    if d.kind.foreign() { tr("nothing of yours nearby").to_string() } else { tr("all yours").into() }
                } else if d.kind.foreign() {
                    names.join(", ")
                } else {
                    trf("shared ×{} with {}", &[&format!("{:.1}", c), &(names.join(", "))])
                };
                ui.label(trf("{} ch {} ({} MHz): {}", &[&(b.label()), &(self.net.channel(&d, b)), &(d.width[b.i()]), &with]));
            }
            }
        } else {
            ui.label(RichText::new(trf("{} is on a cable, not on Wi-Fi.", &[&d.name])).color(pal().dim));
            return;
        }
        // Only devices that connect to Wi-Fi can walk around and roam.
        if !d.kind.transmits() {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let walking = self.walks.contains_key(&id);
                if ui.button(if walking { format!("{}  {}", ic::STOP, tr("Stop walking")) } else { format!("{}  {}", ic::PERSON_SIMPLE_WALK, tr("Take a walk")) }).on_hover_text(tr("Walk past every access point and back; watch the hand-overs. You can also drag the device on the map — it roams as you move it.")).clicked() {
                    if walking {
                        self.walks.remove(&id);
                        self.restore_home(Some(id));
                    } else {
                        self.start_walk(id, now);
                    }
                }
                if ui.button(format!("{}  {}", ic::ARROWS_CLOCKWISE, tr("Reconnect to the best"))).clicked() {
                    let msg = self.net.reconnect(id);
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, msg);
                }
            });
            self.record_reading(ui, id);
            self.signal_plot(ui, id, now);
        }
        if !Self::section(ui, "Networks in the air here") {
            return;
        }
        let mut seen: Vec<(f32, &Device)> = self.net.devices.iter().filter(|t| t.kind.transmits() && t.id != id).map(|t| (self.net.best_rssi(t, d.pos, d.floor), t)).filter(|x| x.0 > -92.0).collect();
        seen.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (r, t) in seen {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{r:>4.0}")).monospace().color(quality_color(r)));
                let (label, tag) = match t.kind {
                    Kind::Neighbor => (t.ssid.clone(), " (neighbour)"),
                    Kind::Interference => (t.name.clone(), tr(" (not Wi-Fi)")),
                    Kind::Speaker => (t.name.clone(), tr(" (speaker mesh)")),
                    _ => (t.ssid.clone(), ""),
                };
                ui.label(format!("{label}{tag}"));
                if t.ifaces.iter().any(|f| f.vlan == crate::extras::GUEST_VLAN) && !t.guest_ssid.is_empty() {
                    ui.label(RichText::new(trf("+ '{}' (guest)", &[&t.guest_ssid])).small().color(pal().dim));
                }
                if t.kind != Kind::Neighbor {
                    ui.label(RichText::new(format!("({})", t.name)).small().color(pal().dim));
                }
                let chans: Vec<String> = Band::ALL.iter().filter(|b| t.bands[b.i()]).map(|b| self.net.channel(t, *b).to_string()).collect();
                ui.label(RichText::new(format!("ch {}", chans.join("/"))).small().color(pal().dim));
            });
        }
    }

    fn signal_plot(&self, ui: &mut egui::Ui, id: u32, now: f64) {
        let Some(h) = self.hist.get(&id).filter(|h| h.len() > 1) else { return };
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 8, pal().bg);
        let y = |r: f32| rect.bottom() - (r.clamp(-95.0, -30.0) + 95.0) / 65.0 * rect.height();
        let x = |t: f64| rect.right() - ((now - t) as f32 / 45.0) * rect.width();
        let th = self.net.wifi_link_of(id).and_then(|l| self.net.wifi_ends(l)).map(|(ap, cl)| crate::wifi::roam_threshold(ap, cl)).unwrap_or(-75.0);
        for (lvl, txt) in [(-67.0, tr("good")), (th, tr("starts looking for another access point"))] {
            p.extend(Shape::dashed_line(&[pos2(rect.left(), y(lvl)), pos2(rect.right(), y(lvl))], Stroke::new(1.0, pal().dim.gamma_multiply(0.6)), 4.0, 4.0));
            p.text(pos2(rect.left() + 4.0, y(lvl) - 2.0), Align2::LEFT_BOTTOM, format!("{lvl:.0} {txt}"), FontId::proportional(10.0), pal().dim);
        }
        let pts: Vec<Pos2> = h.iter().map(|s| pos2(x(s.0), y(s.1))).collect();
        p.add(Shape::line(pts, Stroke::new(2.0, pal().accent)));
        for s in h.iter().filter(|s| s.2) {
            p.line_segment([pos2(x(s.0), rect.top()), pos2(x(s.0), rect.bottom())], Stroke::new(1.5, pal().amber));
            p.text(pos2(x(s.0) + 3.0, rect.top() + 2.0), Align2::LEFT_TOP, tr("hand-over"), FontId::proportional(10.0), pal().amber);
        }
        p.text(rect.right_top() + vec2(-6.0, 4.0), Align2::RIGHT_TOP, tr("signal, last 45 s"), FontId::proportional(10.0), pal().dim);
    }
}

impl App {
    /// Hover card for a device on the map.
    pub(crate) fn device_tooltip(&self, ui: &mut egui::Ui, id: u32, stp: &crate::stp::Stp) {
        let Some(d) = self.net.dev(id) else { return };
        ui.label(RichText::new(&d.name).strong());
        ui.label(RichText::new(if d.model.is_empty() { d.kind.label().to_string() } else { format!("{} · {}", d.kind.label(), d.model) }).small().color(pal().dim));
        for f in d.ifaces.iter().filter(|f| !f.ip.is_empty()).take(4) {
            ui.label(RichText::new(format!("{} {}/{}{}", f.name, f.ip, f.prefix, if f.vlan != 1 { format!("  VLAN {}", f.vlan) } else { String::new() })).monospace().small());
        }
        let subs = d.ifaces.iter().filter(|f| f.parent.is_some()).count();
        if subs > 4 {
            ui.label(RichText::new(trf("… and {} VLAN interfaces", &[&subs])).small().color(pal().dim));
        }
        if let Some(s) = self.net.wifi_link_of(id).and_then(|l| self.net.link_signal(l)) {
            ui.label(RichText::new(trf("{} dBm on {} · Wi-Fi link ≈ {} Mbit/s", &[&format!("{:.0}", s.rssi), &(s.band.label()), &format!("{:.0}", s.mbps)])).color(quality_color(s.rssi)));
            if let Some((e, why)) = self.e2e.get(&id).filter(|e| e.0 < s.mbps * 0.95) {
                ui.label(RichText::new(trf("To the internet ≈ {} Mbit/s — {}", &[&format!("{:.0}", e), &why])).small().color(pal().dim));
            }
        }
        if d.kind.transmits() {
            let bands: Vec<String> = Band::ALL.iter().filter(|b| d.bands[b.i()]).map(|b| trf("{} ch {} {} MHz {} dBm", &[&(b.label()), &(self.net.channel(d, *b)), &(d.width[b.i()]), &format!("{:.0}", d.tx[b.i()])])).collect();
            for b in bands {
                ui.label(RichText::new(b).small());
            }
        }
        if stp.root == Some(id) {
            ui.label(RichText::new(format!("{} {}", ic::CROWN, tr("spanning-tree root"))).color(pal().amber));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Snap {
    Corner,
    OnWall,
    Grid,
}

/// Moves a corner: every wall end on this storey at `from` goes to `to`, so joined walls stay joined.
pub fn move_corner(walls: &mut [crate::wifi::Wall], floor: i8, from: [f32; 2], to: [f32; 2]) {
    let same = |p: [f32; 2]| (p[0] - from[0]).abs() < 0.01 && (p[1] - from[1]).abs() < 0.01;
    for w in walls.iter_mut().filter(|w| w.floor == floor) {
        if same(w.a) {
            w.a = to;
        }
        if same(w.b) {
            w.b = to;
        }
    }
}

/// A wall and every wall reachable from it through shared corners.
pub fn joined_walls(walls: &[crate::wifi::Wall], start: u32) -> Vec<u32> {
    let Some(first) = walls.iter().find(|w| w.id == start) else { return vec![] };
    let touch = |p: [f32; 2], q: [f32; 2]| (p[0] - q[0]).abs() < 0.01 && (p[1] - q[1]).abs() < 0.01;
    let mut out = vec![start];
    let mut i = 0;
    while i < out.len() {
        let w = walls.iter().find(|w| w.id == out[i]).unwrap().clone();
        let next: Vec<u32> = walls
            .iter()
            .filter(|o| o.floor == first.floor && !out.contains(&o.id) && [o.a, o.b].iter().any(|p| touch(*p, w.a) || touch(*p, w.b)))
            .map(|o| o.id)
            .collect();
        out.extend(next);
        i += 1;
    }
    out
}

/// Splits a wall around its middle and puts a door, window (or a plain gap) of `width` metres there.
pub fn insert_opening(n: &mut crate::net::Network, id: u32, kind: Option<WallKind>, width: f32) -> String {
    let Some(w) = n.walls.iter().find(|w| w.id == id).cloned() else { return tr("No such wall.").into() };
    let len = ((w.b[0] - w.a[0]).powi(2) + (w.b[1] - w.a[1]).powi(2)).sqrt() / PX_PER_M;
    if len < width + 0.2 {
        return trf("The wall is only {} m long — too short for a {} m opening.", &[&format!("{:.1}", len), &width]);
    }
    let at = |t: f32| [w.a[0] + (w.b[0] - w.a[0]) * t, w.a[1] + (w.b[1] - w.a[1]) * t];
    let (t0, t1) = (0.5 - width / len / 2.0, 0.5 + width / len / 2.0);
    n.walls.retain(|x| x.id != id);
    for (a, b, k) in [(w.a, at(t0), Some(w.kind)), (at(t0), at(t1), kind), (at(t1), w.b, Some(w.kind))] {
        if let Some(k) = k {
            crate::scenarios::wall(n, a, b, k);
            n.walls.last_mut().unwrap().floor = w.floor;
        }
    }
    match kind {
        Some(k) => trf("Put a {} m {} into the wall — drag its ends to move it.", &[&width, &(k.label().to_lowercase())]),
        None => trf("Left a {} m opening in the wall.", &[&width]),
    }
}

/// Where a wall point lands: on a wall corner, on a wall (T-joint), or on the grid — straightened to
/// 0/45/90° from the previous point when close (always with Shift).
pub fn snap_point(walls: &[crate::wifi::Wall], floor: i8, p: Vec2, tol: f32, from: Option<[f32; 2]>, shift: bool, grid: bool) -> ([f32; 2], Snap) {
    let on_floor = || walls.iter().filter(|w| w.floor == floor);
    let d = |a: [f32; 2]| ((a[0] - p.x).powi(2) + (a[1] - p.y).powi(2)).sqrt();
    // 1. Corners (wall ends) — and the chain's own start point.
    let corner = on_floor().flat_map(|w| [w.a, w.b]).chain(from).filter(|c| d(*c) <= tol).min_by(|a, b| d(*a).total_cmp(&d(*b)));
    if let Some(c) = corner {
        return (c, Snap::Corner);
    }
    // 2. Straighten towards the previous point.
    let mut q = p;
    if let Some(a) = from {
        let (dx, dy) = (p.x - a[0], p.y - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        let ang = dy.atan2(dx);
        let step = std::f32::consts::FRAC_PI_4;
        let near = (ang / step).round() * step;
        let lock = if shift { (ang / (2.0 * step)).round() * 2.0 * step } else { near };
        if shift || (ang - near).abs() < 6f32.to_radians() {
            q = vec2(a[0] + len * lock.cos(), a[1] + len * lock.sin());
        }
    }
    // 3. Onto an existing wall (T-joint).
    let mut best: Option<([f32; 2], f32)> = None;
    for w in on_floor() {
        let (ab, ap) = (vec2(w.b[0] - w.a[0], w.b[1] - w.a[1]), vec2(q.x - w.a[0], q.y - w.a[1]));
        let t = (ap.dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
        let c = [w.a[0] + ab.x * t, w.a[1] + ab.y * t];
        let dist = ((c[0] - q.x).powi(2) + (c[1] - q.y).powi(2)).sqrt();
        if dist <= tol && best.is_none_or(|b| dist < b.1) {
            best = Some((c, dist));
        }
    }
    if let Some((c, _)) = best {
        return (c, Snap::OnWall);
    }
    // 4. The half-metre grid (keeping a straightened line straight) — unless snapping is off.
    let g = if grid { snap(q) } else { [q.x, q.y] };
    if let Some(a) = from {
        if (q.x - a[0]).abs() < 0.01 {
            return ([a[0], g[1]], Snap::Grid);
        }
        if (q.y - a[1]).abs() < 0.01 {
            return ([g[0], a[1]], Snap::Grid);
        }
        if q != p {
            return ([q.x, q.y], Snap::Grid);
        }
    }
    (g, Snap::Grid)
}

#[cfg(test)]
mod wall_edit_tests {
    use super::*;

    #[test]
    fn corners_move_together_and_openings_split_walls() {
        let mut n = crate::net::Network::default();
        let m = |x: f32, y: f32| [x * PX_PER_M, y * PX_PER_M];
        crate::scenarios::wall(&mut n, m(0.0, 0.0), m(4.0, 0.0), WallKind::Brick);
        crate::scenarios::wall(&mut n, m(4.0, 0.0), m(4.0, 3.0), WallKind::Brick);
        crate::scenarios::wall(&mut n, m(9.0, 9.0), m(10.0, 9.0), WallKind::Brick);
        move_corner(&mut n.walls, 0, m(4.0, 0.0), m(5.0, 0.0));
        assert_eq!((n.walls[0].b, n.walls[1].a), (m(5.0, 0.0), m(5.0, 0.0)));
        let first = n.walls[0].id;
        assert_eq!(joined_walls(&n.walls, first).len(), 2);
        let msg = insert_opening(&mut n, first, Some(WallKind::Wood), 0.9);
        assert!(msg.contains("door") || msg.contains("wood"), "{msg}");
        assert_eq!(n.walls.len(), 5);
        let door = n.walls.iter().find(|w| w.kind == WallKind::Wood).unwrap();
        let len = ((door.b[0] - door.a[0]).powi(2) + (door.b[1] - door.a[1]).powi(2)).sqrt() / PX_PER_M;
        assert!((len - 0.9).abs() < 0.01);
        let tiny = n.walls.iter().find(|w| w.a == m(9.0, 9.0)).unwrap().id;
        assert!(insert_opening(&mut n, tiny, None, 0.9).contains("too short"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wifi::{Wall, WallKind};

    fn w(a: [f32; 2], b: [f32; 2]) -> Wall {
        Wall { id: 1, a, b, kind: WallKind::Brick, floor: 0 }
    }

    #[test]
    fn wall_points_join_corners_walls_and_straight_lines() {
        let walls = [w([0.0, 0.0], [200.0, 0.0])];
        // Near a corner: lands exactly on it.
        assert_eq!(snap_point(&walls, 0, vec2(204.0, 5.0), 12.0, None, false, true), ([200.0, 0.0], Snap::Corner));
        // Near the middle of a wall: a T-joint exactly on the wall.
        let (p, k) = snap_point(&walls, 0, vec2(97.0, 6.0), 12.0, None, false, true);
        assert_eq!((p[1], k), (0.0, Snap::OnWall));
        // Walls on another storey don't attract.
        assert_eq!(snap_point(&walls, 1, vec2(204.0, 5.0), 12.0, None, false, true).1, Snap::Grid);
        // Almost vertical from the last point: straightened exactly.
        let (p, _) = snap_point(&[], 0, vec2(103.0, 300.0), 12.0, Some([100.0, 100.0]), false, true);
        assert_eq!(p[0], 100.0);
        // Shift forces horizontal/vertical.
        let (p, _) = snap_point(&[], 0, vec2(300.0, 160.0), 12.0, Some([100.0, 100.0]), true, true);
        assert_eq!(p[1], 100.0);
        // Back at the chain's start closes the room.
        assert_eq!(snap_point(&[], 0, vec2(101.0, 98.0), 12.0, Some([100.0, 100.0]), false, true).1, Snap::Corner);
    }
}
