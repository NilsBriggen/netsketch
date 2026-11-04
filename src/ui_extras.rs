//! Window parts for the planning extras: floor plan pictures, measurements, model library,
//! busy-hour apps, firewall / guest Wi-Fi / IPv6, Power over Ethernet and Ethernet ports.
use super::*;
use crate::extras::{load_library, save_library, FloorPlan, Model, Sample, GUEST_VLAN, POE};
use crate::ports::{PortGroup, PortKind, SPEEDS};

impl App {
    // ---------- floor plan pictures ----------

    fn texture(&mut self, ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
        if !self.textures.contains_key(path) {
            let loaded = std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|b| image::load_from_memory(&b).map_err(|e| e.to_string()))
                .map(|img| {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    ctx.load_texture(path, egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()), egui::TextureOptions::LINEAR)
                });
            self.textures.insert(path.to_string(), loaded);
        }
        self.textures.get(path).and_then(|t| t.as_ref().ok()).cloned()
    }

    /// Draws this storey's floor plan picture under everything else.
    pub(crate) fn draw_plan(&mut self, ctx: &egui::Context, painter: &egui::Painter) {
        let Some(p) = self.net.plans.iter().find(|p| p.floor == self.floor).cloned() else { return };
        let Some(tex) = self.texture(ctx, &p.path) else { return };
        let [w, h] = tex.size();
        let width = p.width_m * PX_PER_M;
        let height = width * h as f32 / w.max(1) as f32;
        let r = Rect::from_min_max(self.to_screen(vec2(p.pos[0], p.pos[1])), self.to_screen(vec2(p.pos[0] + width, p.pos[1] + height)));
        painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply(p.opacity));
    }

    /// Measurement dots on the map: filled with the real reading, ringed by how far the model is off.
    pub(crate) fn draw_survey(&self, painter: &egui::Painter, pointer: Option<Pos2>) -> Option<String> {
        let mut tip = None;
        for s in self.net.survey.iter().filter(|s| s.floor == self.floor) {
            let c = self.to_screen(vec2(s.pos[0], s.pos[1]));
            let pred = self.net.predict(s).unwrap_or(s.rssi);
            let off = (pred - s.rssi).abs();
            let ring = if off < 3.0 { pal().green } else if off < 8.0 { pal().amber } else { pal().red };
            painter.circle_filled(c, 6.0, quality_color(s.rssi));
            painter.circle_stroke(c, 8.0, Stroke::new(2.0, ring));
            if pointer.is_some_and(|p| p.distance(c) < 9.0) {
                tip = Some(format!("Measured {:.0} dBm from {} · model says {:.0} dBm ({:+.1})", s.rssi, self.net.name(s.ap), pred, pred - s.rssi));
            }
        }
        tip
    }

    /// Overview: floor plan picture, measurements and the model library.
    pub(crate) fn plan_extras(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if Self::section_hint(ui, "Floor plan picture", "A picture of this storey's plan under the map (PNG or JPEG, e.g. a scan or a screenshot of the architect's PDF). Trace the walls over it with the Walls tool, then the Wi-Fi map is your real home.") {
        let idx = self.net.plans.iter().position(|p| p.floor == self.floor);
        ui.horizontal(|ui| {
            if ui.button(format!("{}  {}", ic::IMAGE, if idx.is_some() { "Replace…" } else { "Choose a picture…" })).on_hover_text("PNG or JPEG of this storey's plan, e.g. a scan or a screenshot of the architect's PDF").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Picture", &["png", "jpg", "jpeg"]).pick_file() {
                    let path = path.to_string_lossy().into_owned();
                    self.textures.remove(&path);
                    match self.texture(&ctx, &path) {
                        Some(_) => {
                            let plan = FloorPlan { floor: self.floor, path, pos: [0.0, 0.0], width_m: 15.0, opacity: 0.5 };
                            match idx {
                                Some(i) => self.net.plans[i] = plan,
                                None => self.net.plans.push(plan),
                            }
                        }
                        None => {
                            let err = self.textures.get(&path).and_then(|t| t.as_ref().err()).cloned().unwrap_or_default();
                            self.say(&ctx, format!("Couldn't read that picture: {err}"));
                        }
                    }
                }
            }
            if idx.is_some() && ui.small_button(ic::TRASH).on_hover_text("Remove the picture").clicked() {
                self.snapshot();
                self.net.plans.retain(|p| p.floor != self.floor);
            }
        });
        if let Some(Some(Err(e))) = self.net.plans.iter().find(|p| p.floor == self.floor).map(|p| self.textures.get(&p.path)) {
            ui.label(RichText::new(format!("{} The picture can't be shown ({e}). Was it moved? Use Replace… to pick it again.", ic::WARNING)).color(pal().amber));
        }
        if let Some(p) = self.net.plans.iter_mut().find(|p| p.floor == self.floor) {
            egui::Grid::new("planpic").num_columns(2).show(ui, |ui| {
                ui.label("Real width").on_hover_text("Measure one wall you know, then set the picture's full width so the walls come out at their real length");
                ui.add(egui::DragValue::new(&mut p.width_m).range(2.0..=500.0).speed(0.1).suffix(" m"));
                ui.end_row();
                ui.label("Position");
                ui.horizontal(|ui| {
                    let mut x = p.pos[0] / PX_PER_M;
                    let mut y = p.pos[1] / PX_PER_M;
                    ui.add(egui::DragValue::new(&mut x).speed(0.1).prefix("x ").suffix(" m"));
                    ui.add(egui::DragValue::new(&mut y).speed(0.1).prefix("y ").suffix(" m"));
                    p.pos = [x * PX_PER_M, y * PX_PER_M];
                });
                ui.end_row();
                ui.label("See-through");
                ui.add(egui::Slider::new(&mut p.opacity, 0.1..=1.0));
                ui.end_row();
            });
        }
        }

        if !Self::section_hint(ui, "Real measurements", "Measure with the NetSketch Survey phone app (Android): at each spot it records every network it hears, how busy their channels are and, if you like, the speed. Import its file here, drag each pin to where you stood, then add the networks that aren't yours as neighbours and fit the model to what you measured.\n\nAlso possible: a text file with lines like '4.5, 3.0, 0, Home router, -61' (x m, y m, storey, access point, dBm), or readings recorded in Simulate on a device's Wi-Fi tab.") {
            return;
        }
        ui.horizontal(|ui| {
            if ui.button(format!("{}  Import measurements…", ic::DOWNLOAD_SIMPLE)).on_hover_text("A survey from the NetSketch Survey phone app, or a text file of readings").clicked() {
                if let Some(path) = rfd::FileDialog::new().add_filter("Measurements", &["netsurvey", "json", "csv", "txt", "tsv"]).pick_file() {
                    let companion = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("netsurvey") || e.eq_ignore_ascii_case("json"));
                    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    let msg = if companion {
                        self.snapshot();
                        match text.and_then(|t| self.net.import_companion(&t)) {
                            Ok((p, n)) => format!("Imported {p} measuring points ({n} readings). Drag each orange pin to where you stood."),
                            Err(e) => format!("Couldn't import: {e}"),
                        }
                    } else {
                        match text.and_then(|t| self.net.import_survey(&t)) {
                            Ok(n) => format!("Imported {n} readings."),
                            Err(e) => format!("Couldn't import: {e}"),
                        }
                    };
                    self.say(&ctx, msg);
                }
            }
            if !self.net.survey.is_empty() {
                if ui.button(format!("{}  Calibrate", ic::RULER)).on_hover_text("Adjust each access point so the model matches your readings on average").clicked() {
                    self.snapshot();
                    let msg = self.net.calibrate();
                    self.say(&ctx, msg);
                }
                if ui.small_button(ic::TRASH).on_hover_text("Delete all readings (Ctrl+Z brings them back)").clicked() {
                    self.snapshot();
                    self.net.survey.clear();
                }
            }
        });
        if let Some(e) = self.net.survey_error() {
            ui.label(format!("{} readings · model is off by {e:.1} dB on average", self.net.survey.len()));
        }
        self.measured_section(ui);
    }

    // ---------- device settings ----------

    pub(crate) fn device_extras(&mut self, ui: &mut egui::Ui, id: u32) {
        let ctx = ui.ctx().clone();
        let kind = self.net.dev(id).unwrap().kind;

        // Busy-hour apps.
        if !kind.uses().is_empty() && Self::section_hint(ui, "Busy-hour use", "What this device does at its busiest. Together these fill the cables and the air; the traffic slider in the overview picks the hour.") {
            let d = self.net.dev_mut(id).unwrap();
            ui.horizontal_wrapped(|ui| {
                for u in kind.uses().iter().copied() {
                    let mut on = d.uses.contains(&u);
                    let (down, up) = u.demand();
                    if ui.checkbox(&mut on, u.label()).on_hover_text(format!("≈ {down} Mbit/s down, {up} up{}", if u.sensitive() { " — needs low delay" } else { "" })).changed() {
                        if on {
                            d.uses.push(u);
                        } else {
                            d.uses.retain(|x| *x != u);
                        }
                    }
                }
            });
        }
        // Wi-Fi devices pick which of their access point's names they join (home, guest, gadgets…).
        if kind.wifi_client() && !kind.radio() {
            self.join_picker(ui, id);
        }

        // Routers: guest Wi-Fi, IPv6, firewall.
        if matches!(kind, Kind::HomeRouter | Kind::Router) && Self::section(ui, "Firewall & extras") {
            if kind == Kind::HomeRouter {
                let mut guest = self.net.dev(id).unwrap().ifaces.iter().any(|f| f.vlan == GUEST_VLAN);
                if ui.checkbox(&mut guest, "Guest Wi-Fi").on_hover_text("Its own network 192.168.99.0/24, isolated from the home").changed() {
                    self.net.set_guest(id, guest);
                }
                let d = self.net.dev_mut(id).unwrap();
                if guest {
                    ui.horizontal(|ui| {
                        ui.label("Guest name");
                        ui.text_edit_singleline(&mut d.guest_ssid);
                    });
                }
                ui.checkbox(&mut d.ipv6, "IPv6").on_hover_text("Every device also gets a public IPv6 address (try ping6)");
                ui.checkbox(&mut d.sqm, "Smart queueing (SQM)").on_hover_text("Keeps calls and games smooth when the line is full; costs about 8% top speed");
            }
            let d = self.net.dev_mut(id).unwrap();
            ui.label(RichText::new("Firewall rules").color(pal().dim));
            ui.add(TextEdit::multiline(&mut d.rules).desired_rows(3).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text("deny guest lan\nallow internet 192.168.1.10 443\nforward 443 192.168.1.10"))
                .on_hover_text("First match wins: allow|deny <from> <to> [port] — from/to: any, lan, guest, internet, an address or network/prefix. Port forwarding: forward <port> <address>.");
            ui.label(RichText::new("One rule per line, the first match wins: allow|deny <from> <to> [port] (any, lan, guest, internet, an address or network/prefix) · forward <port> <address> opens a port from the internet.").small().color(pal().dim));
        }

        // Power over Ethernet.
        let d = self.net.dev_mut(id).unwrap();
        if kind == Kind::Switch {
            if Self::section(ui, "Power over Ethernet") {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("poe", id)).selected_text(POE[d.poe_class as usize].0).width(220.0).show_ui(ui, |ui| {
                    for (i, (t, _)) in POE.iter().enumerate() {
                        ui.selectable_value(&mut d.poe_class, i as u8, *t);
                    }
                });
                // A typical budget: about half the ports at full power (e.g. 8 × 30 W → 120 W).
                if d.poe_class > 0 && d.poe_budget <= 0.0 {
                    d.poe_budget = (POE[d.poe_class as usize].1 * d.port_count().max(1) as f32 * 0.5).round();
                }
                if d.poe_class > 0 {
                    ui.add(egui::DragValue::new(&mut d.poe_budget).range(0.0..=2000.0).suffix(" W total"));
                }
            });
            }
        } else if kind.poe_powered() || d.poe_draw > 0.0 {
            ui.horizontal(|ui| {
                ui.label("Powered over the cable");
                ui.add(egui::DragValue::new(&mut d.poe_draw).range(0.0..=90.0).speed(0.2).suffix(" W")).on_hover_text("0 = it has its own power supply");
            });
        }

        if kind == Kind::Internet || kind.foreign() {
            return;
        }
        // Model library.
        if !Self::section_hint(ui, "Model", "Save this device's exact hardware (ports, PoE, radio, price) as a model in this plan and your library, or apply a saved one.") {
            return;
        }
        if self.library.is_none() {
            self.library = Some(load_library());
        }
        let models: Vec<Model> = self.net.models.iter().chain(self.library.as_ref().unwrap().iter()).filter(|m| m.spec.kind == kind).cloned().collect();
        let mut apply = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("model", id)).selected_text("Apply a saved model…").width(200.0).show_ui(ui, |ui| {
                if models.is_empty() {
                    ui.label(RichText::new("No saved models for this kind yet").color(pal().dim));
                }
                for m in &models {
                    if ui.selectable_label(false, &m.name).clicked() {
                        apply = Some(m.clone());
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut self.model_name).desired_width(170.0).hint_text("e.g. Brand X AP-620"));
            if ui.button(format!("{}  Save as model", ic::FLOPPY_DISK)).on_hover_text("Stores this device's hardware — ports, PoE, radio, price — in this plan and in your library").clicked() && !self.model_name.trim().is_empty() {
                let m = Model::from(self.model_name.trim(), self.net.dev(id).unwrap());
                self.net.models.retain(|x| x.name != m.name);
                self.net.models.push(m.clone());
                let lib = self.library.as_mut().unwrap();
                lib.retain(|x| x.name != m.name);
                lib.push(m.clone());
                let msg = match save_library(lib) {
                    Ok(()) => format!("Saved model '{}'.", m.name),
                    Err(e) => format!("Saved in the plan, but not in your library: {e}"),
                };
                self.net.dev_mut(id).unwrap().model = m.name;
                self.model_name.clear();
                self.say(&ctx, msg);
            }
        });
        if let Some(m) = apply {
            self.snapshot();
            m.apply(self.net.dev_mut(id).unwrap());
            self.net.replug(id);
            if !self.net.models.iter().any(|x| x.name == m.name) {
                self.net.models.push(m.clone());
            }
            self.say(&ctx, format!("Applied '{}'.", m.name));
        }
    }

    /// Simulate, Wi-Fi tab: record a real reading where the device stands.
    pub(crate) fn record_reading(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(ap) = self.net.wifi_link_of(id).and_then(|l| self.net.wifi_ends(l)).map(|(a, _)| a.id) else { return };
        ui.horizontal(|ui| {
            ui.label("Real reading here");
            ui.add(egui::DragValue::new(&mut self.reading).range(-100.0..=-20.0).suffix(" dBm"));
            if ui.button(format!("{} Record", ic::PLUS)).on_hover_text("Stand where the device is on the map and type what your phone's Wi-Fi app shows").clicked() {
                let d = self.net.dev(id).unwrap();
                self.net.survey.push(Sample { pos: d.pos, floor: d.floor, ap, rssi: self.reading });
                let ctx = ui.ctx().clone();
                self.say(&ctx, "Reading recorded. Calibrate from the plan overview once you have a few.");
            }
        });
    }

    /// Simulate, Ports tab: every cable or radio link of a piece of network gear.
    pub(crate) fn ports_tab(&mut self, ui: &mut egui::Ui, id: u32) {
        let stp = self.net.stp();
        let load = self.net.loads();
        let d = self.net.dev(id).unwrap();
        let links: Vec<&net::Link> = self.net.links.iter().filter(|l| l.a == id || l.b == id).collect();
        let wired = links.iter().filter(|l| !l.medium.wireless()).count();
        ui.label(RichText::new(format!("{wired} of {} ports in use{}", d.port_count(), if d.poe_class > 0 { format!(" · {}", POE[d.poe_class as usize].0) } else { String::new() })).color(pal().dim));
        if links.is_empty() {
            ui.label(RichText::new("Nothing plugged in.").color(pal().dim));
            return;
        }
        let mut poe_total = 0.0;
        // One small block per port (name, then link and state), so it fits any panel width.
        for l in &links {
            let other = self.net.dev(l.other(id).0).unwrap();
            let link = match self.net.link_signal(l) {
                Some(s) => format!("{} {:.0} dBm · {:.0} Mbit/s", s.band.label(), s.rssi, s.mbps),
                None if l.medium == Medium::Powerline => format!("powerline ≈ {:.0} Mbit/s", self.net.link_rate(l)),
                None => {
                    let n = self.net.negotiated(l);
                    if n >= 1000 { format!("{} · {} Gbit/s", l.medium.label(), n as f32 / 1000.0) } else { format!("{} · {n} Mbit/s", l.medium.label()) }
                }
            };
            let (role, col) = if stp.storm.contains(&l.id) {
                ("LOOP", pal().red)
            } else if stp.blocked.contains(&l.id) {
                ("standby", pal().amber)
            } else if stp.root_port.get(&id) == Some(&l.id) {
                ("towards the root", pal().accent)
            } else if d.kind.bridges() || d.kind == Kind::HomeRouter {
                ("forwarding", pal().dim)
            } else {
                ("routed", pal().dim)
            };
            let u = self.net.utilisation(&load, l.id);
            let mut state = format!("{role} · {:.0}% busy", u * 100.0);
            if other.poe_draw > 0.0 && d.poe_class > 0 && !l.medium.wireless() {
                poe_total += other.poe_draw;
                state += &format!(" · powers it ({:.1} W)", other.poe_draw);
            }
            egui::Frame::new().fill(pal().card).corner_radius(6).inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&other.name).strong());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(link).small());
                    ui.label(RichText::new(state).small().color(if u > 1.0 { pal().red } else if u > 0.8 { pal().amber } else { col }));
                });
            });
        }
        if d.poe_class > 0 {
            ui.label(format!("PoE in use: {poe_total:.1} W of {:.0} W", d.poe_budget));
        }
    }
}

// ---------- internet plans ----------

/// Search over Swiss internet plans; picking one sets the plan's speed.
pub(crate) fn plan_picker(ui: &mut egui::Ui, d: &mut crate::net::Device, query: &mut String) {
    if crate::catalog::plans().is_empty() {
        return;
    }
    if let Some(p) = crate::catalog::plans().iter().find(|p| p.title() == d.model) {
        let router = if p.router.is_empty() { String::new() } else { format!(" · comes with {}", p.router) };
        ui.label(RichText::new(format!("{}{router}", p.summary())).small().color(pal().dim)).on_hover_text(format!("{}\n{}", p.note, p.source));
    }
    ui.add(TextEdit::singleline(query).hint_text(format!("{}  Find your plan (Swisscom, Init7, Sunrise…)", ic::MAGNIFYING_GLASS)).desired_width(ui.available_width() - 8.0));
    if query.trim().is_empty() {
        return;
    }
    let hits = crate::catalog::search_plans(query);
    for p in hits.iter().take(8) {
        let mut job = egui::text::LayoutJob::default();
        job.append(&p.title(), 0.0, egui::TextFormat { font_id: FontId::proportional(14.0), color: pal().text, ..Default::default() });
        job.append(&format!("\n{}", p.summary()), 0.0, egui::TextFormat { font_id: FontId::proportional(11.5), color: pal().dim, ..Default::default() });
        if ui.selectable_label(d.model == p.title(), job).on_hover_text(&p.note).clicked() {
            d.plan_mbps = p.down_mbps;
            d.model = p.title();
            query.clear();
        }
    }
    if hits.is_empty() {
        ui.label(RichText::new("No plan matches. Try the provider's name, or set the speed below.").small().color(pal().dim));
    } else if hits.len() > 8 {
        ui.label(RichText::new(format!("…and {} more: type more of the name.", hits.len() - 8)).small().color(pal().dim));
    }
}

// ---------- Ethernet ports ----------

fn speed_text(mbps: u32) -> String {
    if mbps >= 1000 { format!("{} Gbit/s", mbps as f32 / 1000.0) } else { format!("{mbps} Mbit/s") }
}

pub(crate) fn speed_combo(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, mbps: &mut u32) {
    egui::ComboBox::from_id_salt(salt).selected_text(speed_text(*mbps)).width(84.0).show_ui(ui, |ui| {
        for v in SPEEDS {
            ui.selectable_value(mbps, v, speed_text(v));
        }
    });
}

/// A device's ports as groups ("4 × 1 Gbit/s RJ45"), and who is plugged into which port. True when a group was added or removed.
pub(crate) fn port_editor(ui: &mut egui::Ui, id: u32, ports: &mut Vec<PortGroup>, plugged: &[(u16, String)]) -> bool {
    ui.vertical(|ui| {
        let mut remove = None;
        let mut first = 0u16;
        for (i, g) in ports.iter_mut().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::DragValue::new(&mut g.n).range(1..=96)).on_hover_text("How many ports of this kind");
                ui.label("×");
                if g.kind.line() {
                    ui.add(egui::DragValue::new(&mut g.mbps).range(1..=10000).speed(10.0).suffix(" Mbit/s")).on_hover_text("The fastest line rate the modem supports");
                } else {
                    speed_combo(ui, ("pg-speed", id, i), &mut g.mbps);
                }
                egui::ComboBox::from_id_salt(("pg-kind", id, i)).selected_text(g.kind.short()).width(56.0).show_ui(ui, |ui| {
                    for k in PortKind::ALL {
                        ui.selectable_value(&mut g.kind, k, k.label());
                    }
                });
                ui.toggle_value(&mut g.wan, "WAN").on_hover_text("For the internet line: other cables only use it when nothing else is free");
                if ui.small_button(ic::X).on_hover_text("Remove these ports").clicked() {
                    remove = Some(i);
                }
            });
            let end = first + g.n as u16;
            let users: Vec<String> = plugged.iter().filter(|(p, _)| (first..end).contains(p)).map(|(p, who)| format!("{} {who}", *p + 1)).collect();
            if !users.is_empty() {
                ui.label(RichText::new(format!("{} {}", ic::PLUGS_CONNECTED, users.join(" · "))).small().color(pal().dim));
            }
            first = end;
        }
        if let Some(i) = remove {
            ports.remove(i);
        }
        let add = ui.small_button(format!("{} Add ports", ic::PLUS)).clicked();
        if add {
            ports.push(PortGroup::rj45(1, 1000));
        }
        remove.is_some() || add
    })
    .inner
}

impl App {
    /// Which port each end of a cable is in; picking a port another cable uses swaps the two.
    pub(crate) fn link_ports(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(l) = self.net.links.iter().find(|l| l.id == id).cloned() else { return };
        let ends: Vec<u32> = [l.a, l.b].into_iter().filter(|e| self.net.dev(*e).is_some_and(|d| d.kind != Kind::Internet && d.port_count() > 1)).collect();
        if !l.medium.takes_port() || ends.is_empty() || !Self::section(ui, "Ports") {
            return;
        }
        let mut pick = None;
        egui::Grid::new(("lports", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            for e in ends {
                let users = self.net.port_users(e);
                let now = l.port_at(e);
                ui.label(RichText::new(self.net.name(e)).color(pal().dim));
                egui::ComboBox::from_id_salt(("lport", id, e)).selected_text(now.map_or("no port free".into(), |p| self.net.port_name(e, p))).width(210.0).show_ui(ui, |ui| {
                    let count = self.net.dev(e).map_or(0, |d| d.port_count()).min(256);
                    for p in (0..count as u16).filter(|p| self.net.fits_port(&l, e, *p)) {
                        let who = users.iter().find(|(q, _)| *q == p && Some(p) != now).map_or(String::new(), |(_, w)| format!(" — {w}"));
                        if ui.selectable_label(now == Some(p), format!("{}{who}", self.net.port_name(e, p))).clicked() {
                            pick = Some((e, p));
                        }
                    }
                });
                ui.end_row();
            }
        });
        if let Some((e, p)) = pick {
            self.snapshot();
            self.net.move_port(id, e, p);
        }
    }
}
