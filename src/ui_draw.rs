//! Window parts for drawing the home: room labels, right-click menus and the measuring tape.
use super::*;
use crate::radio_ui::{insert_opening, snap_point};

impl App {
    /// Room names on the map: drag to move (Move tool), click to edit.
    pub(crate) fn draw_labels(&mut self, ui: &mut egui::Ui, painter: &egui::Painter) {
        let ids: Vec<u32> = self.net.labels.iter().filter(|l| l.floor == self.floor).map(|l| l.id).collect();
        let size = (13.0 * self.zoom.sqrt()).clamp(11.0, 22.0);
        for id in ids {
            let l = self.net.labels.iter().find(|l| l.id == id).unwrap();
            let c = self.to_screen(vec2(l.pos[0], l.pos[1]));
            let selected = self.sel == Sel::Label(id);
            let g = painter.layout_no_wrap(if l.text.is_empty() { "Room".into() } else { l.text.clone() }, FontId::proportional(size), if selected { pal().accent } else { pal().dim.gamma_multiply(1.3) });
            let r = Rect::from_center_size(c, g.size() + vec2(8.0, 4.0));
            if selected {
                painter.rect_stroke(r, 4, Stroke::new(1.0, pal().accent), StrokeKind::Outside);
            }
            painter.galley(r.min + vec2(4.0, 2.0), g, pal().text);
            if self.mode != Mode::Design || self.tool != Tool::Select {
                continue;
            }
            let resp = ui.interact(r, Id::new(("label", id)), Sense::click_and_drag());
            if resp.clicked() {
                self.sel = Sel::Label(id);
            }
            if resp.drag_started() {
                self.snapshot();
            }
            if resp.dragged() {
                let d = resp.drag_delta() / self.zoom;
                let l = self.net.labels.iter_mut().find(|l| l.id == id).unwrap();
                l.pos = [l.pos[0] + d.x, l.pos[1] + d.y];
                self.sel = Sel::Label(id);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        }
    }

    pub(crate) fn label_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(l) = self.net.labels.iter_mut().find(|l| l.id == id) else { return };
        Self::header(ui, ic::TEXT_T, pal().accent, "Room label", "Drag it on the map to move it");
        ui.add_space(4.0);
        egui::Grid::new(("lbl", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new("Text").color(pal().dim));
            ui.add(TextEdit::singleline(&mut l.text).hint_text("e.g. Living room"));
            ui.end_row();
            ui.label(RichText::new("Storey").color(pal().dim));
            ui.add(egui::DragValue::new(&mut l.floor).range(-3..=20).custom_formatter(|v, _| net::floor_name(v as i8)));
            ui.end_row();
        });
        ui.add_space(6.0);
        if ui.small_button(RichText::new(format!("{}  Remove label", ic::TRASH)).color(pal().red)).clicked() {
            self.snapshot();
            self.net.labels.retain(|l| l.id != id);
            self.sel = Sel::None;
        }
    }

    /// Right-click on a device.
    pub(crate) fn device_menu(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(d) = self.net.dev(id) else { return };
        let (name, off, design) = (d.name.clone(), d.off, self.mode == Mode::Design);
        ui.label(RichText::new(name).strong());
        if ui.button(format!("{}  Try it", ic::PLAY)).clicked() {
            self.sel = Sel::Dev(id);
            if self.mode != Mode::Simulate {
                self.mode = Mode::Simulate;
                let ctx = ui.ctx().clone();
                self.start_sim(&ctx);
            }
            ui.close();
        }
        if design && ui.button(format!("{}  Duplicate", ic::COPY)).clicked() {
            self.snapshot();
            if let Some(new) = self.net.duplicate(id) {
                self.sel = Sel::Dev(new);
            }
            ui.close();
        }
        if ui.button(format!("{}  {}", ic::POWER, if off { "Switch on" } else { "Switch off" })).clicked() {
            self.net.dev_mut(id).unwrap().off = !off;
            self.issues = None;
            ui.close();
        }
        if design && ui.button(RichText::new(format!("{}  Delete", ic::TRASH)).color(pal().red)).clicked() {
            self.snapshot();
            self.net.remove_device(id);
            self.sel = Sel::None;
            ui.close();
        }
    }

    /// Right-click on the map: on a wall, a cable, or empty floor.
    pub(crate) fn map_menu(&mut self, ui: &mut egui::Ui) {
        let Some((wall, link, at)) = self.ctx_at else { return };
        let design = self.mode == Mode::Design;
        if let (Some(w), true) = (wall, design) {
            ui.label(RichText::new("Wall").strong());
            ui.menu_button("Material", |ui| {
                for k in WallKind::ALL {
                    if ui.button(k.label()).clicked() {
                        self.snapshot();
                        if let Some(x) = self.net.walls.iter_mut().find(|x| x.id == w) {
                            x.kind = k;
                        }
                        self.heat_cache = None;
                        ui.close();
                    }
                }
            });
            for (label, kind, width) in [("Put in a door", Some(WallKind::Wood), 0.9), ("Put in a window", Some(WallKind::Glass), 1.2), ("Leave an opening", None, 0.9)] {
                if ui.button(label).clicked() {
                    self.snapshot();
                    let msg = insert_opening(&mut self.net, w, kind, width);
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, msg);
                    ui.close();
                }
            }
            if ui.button(RichText::new(format!("{}  Remove wall", ic::TRASH)).color(pal().red)).clicked() {
                self.snapshot();
                self.net.walls.retain(|x| x.id != w);
                self.sel = Sel::None;
                ui.close();
            }
            ui.separator();
        } else if let Some(l) = link {
            let unplugged = self.net.links.iter().find(|x| x.id == l).is_some_and(|x| x.unplugged);
            ui.label(RichText::new("Connection").strong());
            if ui.button(if unplugged { "Plug it back in" } else { "Unplug — what if it breaks?" }).clicked() {
                if let Some(x) = self.net.links.iter_mut().find(|x| x.id == l) {
                    x.unplugged = !unplugged;
                }
                self.issues = None;
                ui.close();
            }
            if design && ui.button(RichText::new(format!("{}  Remove connection", ic::TRASH)).color(pal().red)).clicked() {
                self.snapshot();
                self.net.remove_link(l);
                self.sel = Sel::None;
                ui.close();
            }
            ui.separator();
        }
        if design {
            ui.menu_button(format!("{}  Add a device here", ic::PLUS), |ui| {
                for k in Kind::SHELF {
                    if ui.button(format!("{}  {}", kind_icon(k), k.label())).clicked() {
                        self.snapshot();
                        let id = self.net.add_device(k, at);
                        self.net.dev_mut(id).unwrap().floor = self.floor;
                        self.sel = Sel::Dev(id);
                        ui.close();
                    }
                }
            });
            if ui.button(format!("{}  Add a room label here", ic::TEXT_T)).clicked() {
                self.snapshot();
                let id = self.net.add_label(at, self.floor, "Room");
                self.sel = Sel::Label(id);
                ui.close();
            }
        }
        if ui.button(format!("{}  Fit to window (F)", ic::CORNERS_OUT)).clicked() {
            self.fit();
            ui.close();
        }
    }

    /// The measuring tape: click a start and an end; the distance stays until the next click or Esc.
    pub(crate) fn measure_tool(&mut self, resp: &egui::Response, painter: &egui::Painter, pointer: Option<Pos2>) {
        let shift = resp.ctx.input(|i| i.modifiers.shift);
        let from = self.measure.and_then(|m| m.1.is_none().then_some(m.0));
        let world = |me: &Self, p: Pos2| snap_point(&me.net.walls, me.floor, me.to_world(p), 12.0 / me.zoom, from, shift, me.snap).0;
        if resp.clicked() {
            if let Some(p) = pointer {
                let pt = world(self, p);
                self.measure = match self.measure {
                    Some((a, None)) => Some((a, Some(pt))),
                    _ => Some((pt, None)),
                };
            }
        }
        let Some((a, b)) = self.measure else {
            if let Some(p) = pointer {
                painter.text(p + vec2(12.0, -12.0), Align2::LEFT_BOTTOM, "click where the measurement starts", FontId::proportional(11.5), pal().dim);
            }
            return;
        };
        let Some(b) = b.or_else(|| pointer.map(|p| world(self, p))) else { return };
        let (sa, sb) = (self.to_screen(vec2(a[0], a[1])), self.to_screen(vec2(b[0], b[1])));
        painter.line_segment([sa, sb], Stroke::new(2.0, pal().amber));
        for p in [sa, sb] {
            painter.circle_filled(p, 4.0, pal().amber);
        }
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt() / net::PX_PER_M;
        let g = painter.layout_no_wrap(format!("{len:.2} m"), FontId::proportional(13.0), pal().bg);
        let r = Rect::from_center_size(sa.lerp(sb, 0.5) - vec2(0.0, 14.0), g.size() + vec2(10.0, 4.0));
        painter.rect_filled(r, 5, pal().amber);
        painter.galley(r.min + vec2(5.0, 2.0), g, pal().bg);
    }

    /// Left panel under the Measure tool: the distance, and fixing a floor-plan picture's scale from it.
    pub(crate) fn measure_panel(&mut self, ui: &mut egui::Ui) {
        let Some((a, Some(b))) = self.measure else {
            ui.label(RichText::new("Click two points to measure. Esc clears.").small().color(pal().dim));
            return;
        };
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt() / net::PX_PER_M;
        ui.label(RichText::new(format!("{len:.2} m on the map")).strong());
        let Some(i) = self.net.plans.iter().position(|p| p.floor == self.floor) else { return };
        ui.label(RichText::new("Measured a wall you know on the floor-plan picture? Type its real length:").small().color(pal().dim));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.measure_real).range(0.1..=500.0).speed(0.05).suffix(" m"));
            if ui.button("Scale the picture").on_hover_text("Stretches the floor-plan picture so this distance comes out at its real length").clicked() && len > 0.0 {
                self.snapshot();
                let p = &mut self.net.plans[i];
                let f = self.measure_real / len;
                p.width_m *= f;
                // Keep the first measured point where it is on the picture.
                p.pos = [a[0] - (a[0] - p.pos[0]) * f, a[1] - (a[1] - p.pos[1]) * f];
                self.measure = None;
                let ctx = ui.ctx().clone();
                self.say(&ctx, format!("The picture is now {:.1} m wide.", self.net.plans[i].width_m));
            }
        });
    }
}
