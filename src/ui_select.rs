//! Working on several things at once: the selection beyond the one in the side panel (Shift+click, Shift+drag a
//! box), moving, nudging, deleting, aligning and spreading it, and copy and paste (also onto another storey).
use super::*;
use crate::net::{Device, Link, RoomLabel};
use crate::wifi::Wall;

/// What Ctrl+C took: devices, the cables between them, walls and room labels, and the middle of it all.
pub(crate) struct Clip {
    devices: Vec<Device>,
    links: Vec<Link>,
    walls: Vec<Wall>,
    labels: Vec<RoomLabel>,
    centre: [f32; 2],
}

impl App {
    /// Everything selected: the one in the side panel first.
    pub(crate) fn selection(&self) -> Vec<Sel> {
        std::iter::once(self.sel).filter(|s| *s != Sel::None).chain(self.extra.iter().copied()).collect()
    }

    /// Shift+click: adds to the selection, or takes out what's already in it.
    pub(crate) fn toggle_sel(&mut self, s: Sel) {
        self.multi_touched = true;
        if self.sel == Sel::None {
            self.sel = s;
        } else if self.sel == s {
            self.sel = self.extra.pop().unwrap_or(Sel::None);
        } else if let Some(i) = self.extra.iter().position(|x| *x == s) {
            self.extra.remove(i);
        } else {
            self.extra.push(s);
        }
    }

    /// Selects what lies inside a box drawn on the map (map points): devices, walls (both ends), labels, pins.
    pub(crate) fn select_box(&mut self, a: Vec2, b: Vec2, add: bool) {
        let r = Rect::from_two_pos(a.to_pos2(), b.to_pos2());
        let inside = |p: [f32; 2]| r.contains(pos2(p[0], p[1]));
        let f = self.floor;
        let mut found: Vec<Sel> = self.net.devices.iter().filter(|d| d.floor == f && inside(d.pos)).map(|d| Sel::Dev(d.id)).collect();
        found.extend(self.net.walls.iter().filter(|w| w.floor == f && inside(w.a) && inside(w.b)).map(|w| Sel::Wall(w.id)));
        found.extend(self.net.labels.iter().filter(|l| l.floor == f && inside(l.pos)).map(|l| Sel::Label(l.id)));
        found.extend(self.net.points.iter().filter(|p| p.floor == f && inside(p.pos)).map(|p| Sel::Point(p.id)));
        let mut all = if add { self.selection() } else { vec![] };
        for s in found {
            if !all.contains(&s) {
                all.push(s);
            }
        }
        self.sel = all.first().copied().unwrap_or(Sel::None);
        self.extra = all.into_iter().skip(1).collect();
        self.multi_touched = true;
    }

    /// Moves everything selected by `d` (map units). Cables and walls between moved things follow.
    pub(crate) fn move_selection(&mut self, d: [f32; 2]) {
        for s in self.selection() {
            match s {
                Sel::Dev(id) => {
                    if let Some(x) = self.net.dev_mut(id) {
                        x.pos = [x.pos[0] + d[0], x.pos[1] + d[1]];
                    }
                }
                Sel::Wall(id) => {
                    if let Some(w) = self.net.walls.iter_mut().find(|w| w.id == id) {
                        w.a = [w.a[0] + d[0], w.a[1] + d[1]];
                        w.b = [w.b[0] + d[0], w.b[1] + d[1]];
                    }
                }
                Sel::Label(id) => {
                    if let Some(l) = self.net.labels.iter_mut().find(|l| l.id == id) {
                        l.pos = [l.pos[0] + d[0], l.pos[1] + d[1]];
                    }
                }
                Sel::Point(id) => {
                    if let Some(p) = self.net.points.iter_mut().find(|p| p.id == id) {
                        p.pos = [p.pos[0] + d[0], p.pos[1] + d[1]];
                    }
                }
                Sel::Link(_) | Sel::None => {}
            }
        }
        self.heat_cache = None;
    }

    pub(crate) fn delete_selection(&mut self) {
        for s in self.selection() {
            match s {
                Sel::Dev(id) => self.net.remove_device(id),
                Sel::Link(id) => self.net.remove_link(id),
                Sel::Wall(id) => self.net.walls.retain(|w| w.id != id),
                Sel::Point(id) => self.net.points.retain(|p| p.id != id),
                Sel::Label(id) => self.net.labels.retain(|l| l.id != id),
                Sel::None => {}
            }
        }
        self.sel = Sel::None;
        self.extra.clear();
    }

    /// The selection for Ctrl+C (cables only between copied devices). None when there's nothing to copy.
    pub(crate) fn copy_selection(&self) -> Option<Clip> {
        let sel = self.selection();
        let devices: Vec<Device> = sel.iter().filter_map(|s| if let Sel::Dev(id) = s { self.net.dev(*id).cloned() } else { None }).collect();
        let has = |id: u32| devices.iter().any(|d| d.id == id);
        let links: Vec<Link> = self.net.links.iter().filter(|l| has(l.a) && has(l.b)).cloned().collect();
        let walls: Vec<Wall> = sel.iter().filter_map(|s| if let Sel::Wall(id) = s { self.net.walls.iter().find(|w| w.id == *id).cloned() } else { None }).collect();
        let labels: Vec<RoomLabel> = sel.iter().filter_map(|s| if let Sel::Label(id) = s { self.net.labels.iter().find(|l| l.id == *id).cloned() } else { None }).collect();
        let pts: Vec<[f32; 2]> = devices.iter().map(|d| d.pos).chain(walls.iter().flat_map(|w| [w.a, w.b])).chain(labels.iter().map(|l| l.pos)).collect();
        if pts.is_empty() {
            return None;
        }
        let (lo, hi) = pts.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        Some(Clip { devices, links, walls, labels, centre: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0] })
    }

    /// Puts a copy down with its middle at `at` on the storey shown, and selects it. Devices get fresh addresses.
    pub(crate) fn paste(&mut self, clip: &Clip, at: [f32; 2]) -> usize {
        let off = [at[0] - clip.centre[0], at[1] - clip.centre[1]];
        let mv = |p: [f32; 2]| [p[0] + off[0], p[1] + off[1]];
        let floor = self.floor;
        let mut new: Vec<Sel> = vec![];
        let mut ids: HashMap<u32, u32> = HashMap::new();
        for d in &clip.devices {
            // A device that's still there is copied the usual way (fresh addresses); one deleted since comes back as it was.
            let id = if self.net.dev(d.id).is_some() {
                self.net.duplicate(d.id)
            } else {
                let id = self.net.add_device(d.kind, d.pos);
                let fresh = self.net.dev(id).unwrap().clone();
                *self.net.dev_mut(id).unwrap() = Device { id, ifaces: fresh.ifaces, ..d.clone() };
                Some(id)
            };
            let Some(id) = id else { continue };
            let x = self.net.dev_mut(id).unwrap();
            x.pos = mv(d.pos);
            x.floor = floor;
            ids.insert(d.id, id);
            new.push(Sel::Dev(id));
        }
        for l in &clip.links {
            if let (Some(a), Some(b)) = (ids.get(&l.a), ids.get(&l.b)) {
                if let Ok(id) = self.net.connect(*a, *b, l.medium) {
                    if let Some(x) = self.net.links.iter_mut().find(|x| x.id == id) {
                        x.path = l.path.iter().map(|p| mv(*p)).collect();
                    }
                }
            }
        }
        for w in &clip.walls {
            crate::scenarios::wall(&mut self.net, mv(w.a), mv(w.b), w.kind);
            let x = self.net.walls.last_mut().unwrap();
            x.floor = floor;
            x.door = w.door;
            new.push(Sel::Wall(x.id));
        }
        for l in &clip.labels {
            new.push(Sel::Label(self.net.add_label(mv(l.pos), floor, &l.text)));
        }
        let n = new.len();
        self.sel = new.first().copied().unwrap_or(Sel::None);
        self.extra = new.into_iter().skip(1).collect();
        self.multi_touched = true;
        self.heat_cache = None;
        n
    }

    /// Where a selected thing sits (its middle), for aligning.
    fn sel_pos(&self, s: Sel) -> Option<[f32; 2]> {
        match s {
            Sel::Dev(id) => self.net.dev(id).map(|d| d.pos),
            Sel::Label(id) => self.net.labels.iter().find(|l| l.id == id).map(|l| l.pos),
            Sel::Point(id) => self.net.points.iter().find(|p| p.id == id).map(|p| p.pos),
            Sel::Wall(id) => self.net.walls.iter().find(|w| w.id == id).map(|w| [(w.a[0] + w.b[0]) / 2.0, (w.a[1] + w.b[1]) / 2.0]),
            _ => None,
        }
    }

    fn move_one(&mut self, s: Sel, d: [f32; 2]) {
        let (sel, extra) = (self.sel, std::mem::take(&mut self.extra));
        self.sel = s;
        self.move_selection(d);
        (self.sel, self.extra) = (sel, extra);
    }

    /// The side panel while several things are selected: what's in it, and what can be done to all of them.
    pub(crate) fn multi_panel(&mut self, ui: &mut egui::Ui) {
        let sel = self.selection();
        let count = |f: fn(&Sel) -> bool| sel.iter().filter(|s| f(s)).count();
        let (devs, walls, labels) = (count(|s| matches!(s, Sel::Dev(_))), count(|s| matches!(s, Sel::Wall(_))), count(|s| matches!(s, Sel::Label(_))));
        let mut parts = vec![];
        if devs > 0 {
            parts.push(trf("{} devices", &[&devs]));
        }
        if walls > 0 {
            parts.push(trf("{} walls", &[&walls]));
        }
        if labels > 0 {
            parts.push(trf("{} labels", &[&labels]));
        }
        Self::header(ui, ic::SELECTION_ALL, pal().accent, &trf("{} things selected", &[&sel.len()]), &parts.join(" · "));
        ui.label(RichText::new(tr("Drag any of them to move them all; arrow keys nudge (Shift: 1 m). Shift+click adds or removes one.")).small().color(pal().dim));
        ui.add_space(6.0);
        if self.mode != Mode::Design {
            return;
        }
        let pos: Vec<(Sel, [f32; 2])> = sel.iter().filter_map(|s| Some((*s, self.sel_pos(*s)?))).collect();
        if pos.len() >= 2 {
            ui_kit::caption(ui, tr("Line up"));
            ui.horizontal_wrapped(|ui| {
                let (lo, hi) = pos.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), (_, p)| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
                let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
                for (icon, label, axis, to) in [(ic::ALIGN_LEFT, tr("Left"), 0, lo[0]), (ic::ALIGN_CENTER_HORIZONTAL, tr("Centre"), 0, mid[0]), (ic::ALIGN_TOP, tr("Top"), 1, lo[1]), (ic::ALIGN_CENTER_VERTICAL, tr("Middle"), 1, mid[1])] {
                    if ui.button(format!("{icon}  {label}")).clicked() {
                        self.snapshot();
                        for (s, p) in &pos {
                            let mut d = [0.0, 0.0];
                            d[axis] = to - p[axis];
                            self.move_one(*s, d);
                        }
                    }
                }
            });
        }
        if pos.len() >= 3 {
            ui.horizontal_wrapped(|ui| {
                for (label, axis) in [(tr("Spread out across"), 0), (tr("Spread out down"), 1)] {
                    if ui.button(format!("{}  {label}", if axis == 0 { ic::ARROWS_HORIZONTAL } else { ic::ARROWS_VERTICAL })).on_hover_text(tr("Equal gaps between the first and the last")).clicked() {
                        self.snapshot();
                        let mut by = pos.clone();
                        by.sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]));
                        let (a, b) = (by[0].1[axis], by[by.len() - 1].1[axis]);
                        let step = (b - a) / (by.len() - 1) as f32;
                        for (i, (s, p)) in by.iter().enumerate() {
                            let mut d = [0.0, 0.0];
                            d[axis] = a + step * i as f32 - p[axis];
                            self.move_one(*s, d);
                        }
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(format!("{}  {}", ic::COPY, tr("Copy (Ctrl+C)"))).on_hover_text(tr("Paste with Ctrl+V where the pointer is: also on another storey")).clicked() {
                self.clip = self.copy_selection();
                let ctx = ui.ctx().clone();
                self.say(&ctx, tr("Copied — Ctrl+V puts it where the pointer is."));
            }
            if ui.button(format!("{}  {}", ic::COPY_SIMPLE, tr("Duplicate (Ctrl+D)"))).clicked() {
                self.duplicate_selection();
            }
            if ui.button(RichText::new(format!("{}  {}", ic::TRASH, tr("Delete all"))).color(pal().red)).clicked() {
                self.snapshot();
                self.delete_selection();
            }
        });
    }

    /// Ctrl+D: a copy of the selection a little down and to the right.
    pub(crate) fn duplicate_selection(&mut self) {
        if let Some(c) = self.copy_selection() {
            self.snapshot();
            let at = [c.centre[0] + PX_PER_M, c.centre[1] + PX_PER_M];
            self.paste(&c, at);
        }
    }

    /// Ctrl+V: the copied things with their middle under the pointer (or beside where they came from).
    pub(crate) fn paste_clip(&mut self, ctx: &egui::Context) {
        let Some(c) = self.clip.take() else {
            self.say(ctx, tr("Nothing copied yet: select something and press Ctrl+C."));
            return;
        };
        let at = ctx.input(|i| i.pointer.hover_pos()).filter(|p| self.canvas_rect.contains(*p)).map(|p| self.to_world(p)).map_or([c.centre[0] + PX_PER_M, c.centre[1] + PX_PER_M], |w| [w.x, w.y]);
        self.snapshot();
        let n = self.paste(&c, at);
        self.clip = Some(c);
        self.say(ctx, trf("Pasted {} things on {}.", &[&n, &self.net.storey(self.floor)]));
    }

    /// While a device is dragged: lines it up with another device that's nearly level or nearly above it (dotted
    /// guide), so rows of devices come out straight. True when it snapped.
    pub(crate) fn align_guides(&mut self, id: u32, painter: &egui::Painter) -> bool {
        let Some(me) = self.net.dev(id).cloned() else { return false };
        let tol = 6.0 / self.zoom;
        let others = self.net.devices.iter().filter(|d| d.id != id && d.floor == me.floor && !self.extra.contains(&Sel::Dev(d.id)));
        let mut best: [Option<(f32, [f32; 2])>; 2] = [None, None];
        for o in others {
            for axis in 0..2 {
                let gap = (o.pos[axis] - me.pos[axis]).abs();
                if gap < tol && best[axis].is_none_or(|b| gap < (b.0 - me.pos[axis]).abs()) {
                    best[axis] = Some((o.pos[axis], o.pos));
                }
            }
        }
        let mut hit = false;
        for axis in 0..2 {
            if let Some((v, other)) = best[axis] {
                hit = true;
                if let Some(d) = self.net.dev_mut(id) {
                    d.pos[axis] = v;
                }
                let p = self.net.dev(id).unwrap().pos;
                let (a, b) = (self.to_screen(Vec2::from(other)), self.to_screen(Vec2::from(p)));
                painter.add(egui::Shape::dashed_line(&[a, b], Stroke::new(1.0, pal().accent), 4.0, 4.0));
            }
        }
        hit
    }

    /// Keeps the extra selection honest at the end of a frame: a selection made any other way (a click, the search,
    /// Tab) replaces it, and things that are gone leave it.
    pub(crate) fn tidy_selection(&mut self) {
        if (self.sel != self.sel_seen && !self.multi_touched) || self.sel == Sel::None {
            self.extra.clear();
        }
        let sel = self.sel;
        let n = &self.net;
        self.extra.retain(|s| {
            *s != sel
                && match *s {
                    Sel::Dev(id) => n.dev(id).is_some(),
                    Sel::Link(id) => n.links.iter().any(|l| l.id == id),
                    Sel::Wall(id) => n.walls.iter().any(|w| w.id == id),
                    Sel::Label(id) => n.labels.iter().any(|l| l.id == id),
                    Sel::Point(id) => n.points.iter().any(|p| p.id == id),
                    Sel::None => false,
                }
        });
        self.sel_seen = self.sel;
        self.multi_touched = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wall_edited_in_its_panel_can_be_undone() {
        let mut app = App::for_tests();
        let id = app.net.walls[0].id;
        app.sel = Sel::Wall(id);
        let (old, json) = app.sel_copy().unwrap();
        app.net.walls[0].kind = WallKind::Glass;
        assert_ne!(app.sel_copy().unwrap().1, json);
        app.snapshot_before(old);
        assert_eq!(app.net.walls[0].kind, WallKind::Glass);
        let back = app.undo.pop().unwrap().0;
        assert_ne!(back.walls.iter().find(|w| w.id == id).unwrap().kind, WallKind::Glass);
    }

    #[test]
    fn copy_and_paste_gives_fresh_ids_on_the_storey_shown() {
        let mut app = App::for_tests();
        let ids: Vec<u32> = app.net.devices.iter().take(3).map(|d| d.id).collect();
        let wall = app.net.walls[0].id;
        app.sel = Sel::Dev(ids[0]);
        app.extra = vec![Sel::Dev(ids[1]), Sel::Dev(ids[2]), Sel::Wall(wall)];
        let clip = app.copy_selection().unwrap();
        let (devs, walls, links) = (app.net.devices.len(), app.net.walls.len(), app.net.links.len());
        let inner = clip.links.len();
        app.floor = 1;
        assert_eq!(app.paste(&clip, [5000.0, 5000.0]), 4);
        assert_eq!((app.net.devices.len(), app.net.walls.len(), app.net.links.len()), (devs + 3, walls + 1, links + inner));
        let new: Vec<u32> = app.selection().iter().filter_map(|s| if let Sel::Dev(id) = s { Some(*id) } else { None }).collect();
        assert!(new.iter().all(|id| !ids.contains(id) && app.net.dev(*id).unwrap().floor == 1));
        // Moving the pasted things moves all of them; deleting takes them all away again.
        let before = app.net.dev(new[0]).unwrap().pos;
        app.move_selection([10.0, 0.0]);
        assert_eq!(app.net.dev(new[0]).unwrap().pos[0], before[0] + 10.0);
        app.delete_selection();
        assert_eq!((app.net.devices.len(), app.net.walls.len()), (devs, walls));
    }
}
