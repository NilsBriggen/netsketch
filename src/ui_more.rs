//! Window parts for: failures and weak spots, power use, time of day, extra Wi-Fi names, joining a name,
//! the internet-port choice, searching devices and exporting the map as a picture.
use super::*;

impl App {
    /// "Switch off" / "Switch on" for any device that's part of the network.
    pub(crate) fn power_toggle(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(d) = self.net.dev(id) else { return };
        if d.kind.foreign() || d.kind == Kind::Internet {
            return;
        }
        let off = d.off;
        let label = if off { format!("{}  Switch on", ic::POWER) } else { format!("{}  Switch off", ic::POWER) };
        if ui.button(RichText::new(label).color(if off { GREEN } else { AMBER })).on_hover_text("Try what happens when this device fails or loses power").clicked() {
            let d = self.net.dev_mut(id).unwrap();
            d.off = !off;
            self.issues = None;
            let name = d.name.clone();
            let ctx = ui.ctx().clone();
            self.say(&ctx, if off { format!("{name} is back on.") } else { format!("{name} is off — see what stopped working in the problem list and with ping.") });
        }
        if off {
            ui.label(RichText::new("Switched off: it passes no traffic and its Wi-Fi is silent.").small().color(AMBER));
        }
    }

    /// Link settings: unplug it, or make it a home router's internet port.
    pub(crate) fn link_extras(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(l) = self.net.links.iter().find(|l| l.id == id).cloned() else { return };
        if !Self::section(ui, "Try it") {
            return;
        }
        let mut unplugged = l.unplugged;
        if ui.checkbox(&mut unplugged, "Unplugged — what if this cable breaks?").changed() {
            self.net.links.iter_mut().find(|x| x.id == id).unwrap().unplugged = unplugged;
            self.issues = None;
        }
        for end in [l.a, l.b] {
            let (Some(r), Some(o)) = (self.net.dev(end), self.net.dev(l.other(end).0)) else { continue };
            if r.kind != Kind::HomeRouter || o.kind == Kind::Internet || l.medium.wireless() {
                continue;
            }
            let mut wan = l.end_if(end) == 1;
            let label = format!("This is {}'s internet (WAN) port — router behind router", r.name);
            if ui.checkbox(&mut wan, label).on_hover_text("Plug a second router's WAN port into your network: it makes its own network behind the first (double NAT)").changed() {
                self.snapshot();
                self.net.set_wan(end, id, wan);
                let mut log = vec![];
                self.net.dhcp_one(end, &mut log);
                self.push_logs(log);
            }
        }
    }

    /// Overview: what breaks the most if it fails, power use and the hour traffic is looked at.
    pub(crate) fn resilience_and_power(&mut self, ui: &mut egui::Ui) {
        self.coverage_section(ui);
        if Self::section_hint(ui, "Weak spots", "Devices whose failure cuts others off. A second path (cable or mesh), a spare switch on the shelf, or a mobile backup line takes the sting out.") {
        let spots = self.net.weak_spots();
        if spots.is_empty() {
            ui.label(RichText::new("No single device whose failure cuts others off.").small().color(DIM));
        }
        let mut pick = None;
        for (id, n) in spots.iter().take(4) {
            ui.horizontal_wrapped(|ui| {
                let col = if *n >= 10 { RED } else if *n >= 4 { AMBER } else { DIM };
                if ui.add(egui::Label::new(RichText::new(self.net.name(*id)).strong()).sense(Sense::click())).on_hover_text("Select it").clicked() {
                    pick = Some(*id);
                }
                ui.label(RichText::new(format!("— if it fails, {n} device{} lose the network", if *n == 1 { "" } else { "s" })).color(col));
            });
        }
        if let Some(id) = pick {
            self.focus(id);
        }
        }
        if !Self::section(ui, "Power and time") {
            return;
        }
        let w = self.net.power_w();
        let kwh = self.net.yearly_kwh();
        let cur = self.net.currency.clone();
        egui::Grid::new("power").num_columns(2).show(ui, |ui| {
            ui.label(RichText::new("Power use").color(DIM));
            ui.label(format!("≈ {w:.0} W around the clock · {kwh:.0} kWh a year"));
            ui.end_row();
            ui.label(RichText::new("Electricity").color(DIM));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.net.kwh_price).range(0.0..=2.0).speed(0.01).prefix(cur.clone()).suffix(" /kWh"));
                ui.label(RichText::new(format!("≈ {cur}{:.0} a year", kwh * self.net.kwh_price)).strong());
            });
            ui.end_row();
            ui.label(RichText::new("Traffic at").color(DIM));
            let before = self.net.hour;
            ui.add(egui::Slider::new(&mut self.net.hour, 0.0..=23.75).step_by(0.25).custom_formatter(|v, _| more::clock(v as f32))).on_hover_text("Apps follow the day: streaming peaks in the evening, calls in the daytime, backups at night");
            if self.net.hour != before {
                self.issues = None;
            }
            ui.end_row();
        });
    }

    /// Radios: extra network names, each into its own VLAN (one radio, several networks).
    pub(crate) fn names_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let d = self.net.dev_mut(id).unwrap();
        if !Self::section_hint(ui, "More network names", "One access point can broadcast several names; devices that join a name land in its VLAN (staff, guests, gadgets…). Each extra name costs a little airtime.") {
            return;
        }
        let mut remove = None;
        for (i, (name, vlan)) in d.ssids.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(name).desired_width(140.0));
                ui.add(egui::DragValue::new(vlan).range(1..=4094).prefix("VLAN "));
                if ui.small_button(ic::X).clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            d.ssids.remove(i);
        }
        if ui.small_button(format!("{} Add a name", ic::PLUS)).clicked() {
            let n = d.ssids.len() + 2;
            d.ssids.push((format!("{}-{n}", d.ssid), 10 * n as u16));
        }
    }

    /// Wi-Fi clients: which of their access point's names they joined.
    pub(crate) fn join_picker(&mut self, ui: &mut egui::Ui, id: u32) {
        let Some(l) = self.net.wifi_link_of(id) else { return };
        let Some((ap, cl)) = self.net.wifi_ends(l) else { return };
        let names = Network::names_of(ap);
        if names.len() < 2 {
            return;
        }
        let current = Network::joined(cl, ap).to_string();
        if !Self::section(ui, "Joined network") {
            return;
        }
        let mut pick = None;
        egui::ComboBox::from_id_salt(("join", id)).selected_text(&current).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
            for (n, v) in &names {
                if ui.selectable_label(*n == current, format!("{n}{}", if *v != 1 { format!("  (VLAN {v})") } else { String::new() })).clicked() {
                    pick = Some((n.clone(), *v));
                }
            }
        });
        if let Some((n, v)) = pick {
            // The main name keeps the device's own VLAN setting; others put it into theirs.
            let keep = self.net.dev(id).unwrap().ifaces.first().map(|f| f.vlan).unwrap_or(1);
            let vlan = if v == 1 && !names.iter().any(|x| x.1 == keep && x.1 != 1) { keep } else { v };
            self.net.join(id, if n == names[0].0 { "" } else { &n }, vlan);
            self.issues = None;
        }
    }

    /// Search box over the map: type part of a name, pick a match, the map jumps to it.
    pub(crate) fn search_overlay(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let r = Rect::from_min_size(rect.left_top() + vec2(10.0, 10.0), vec2(220.0f32.min(rect.width() - 20.0), 30.0));
        let mut jump = None;
        ui.put(r, |ui: &mut egui::Ui| {
            ui.add(TextEdit::singleline(&mut self.search).hint_text(format!("{}  Find a device…", ic::MAGNIFYING_GLASS)).desired_width(r.width()))
        });
        let q = self.search.trim().to_lowercase();
        if !q.is_empty() {
            let hits: Vec<(u32, String)> = self
                .net
                .devices
                .iter()
                .filter(|d| d.name.to_lowercase().contains(&q) || d.ifaces.iter().any(|f| f.ip.starts_with(&q)))
                .take(8)
                .map(|d| (d.id, d.name.clone()))
                .collect();
            egui::Area::new(Id::new("search_hits")).fixed_pos(r.left_bottom() + vec2(0.0, 4.0)).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
                egui::Frame::new().fill(CARD).corner_radius(8).inner_margin(Margin::same(6)).show(ui, |ui| {
                    ui.set_width(r.width() - 12.0);
                    if hits.is_empty() {
                        ui.label(RichText::new("No match").color(DIM));
                    }
                    for (id, name) in hits {
                        if ui.selectable_label(false, name).clicked() {
                            jump = Some(id);
                        }
                    }
                });
            });
        }
        if let Some(id) = jump {
            self.focus(id);
            self.search.clear();
        }
    }

    /// Saves the visible map as a PNG: ask where, then grab the next frame.
    pub(crate) fn export_picture(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Picture", &["png"]).set_file_name("network-map.png").save_file() {
            self.export_pic = Some(path);
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
    }

    /// Crops a screenshot to the map and writes it.
    pub(crate) fn save_picture(&mut self, ctx: &egui::Context, img: &egui::ColorImage) {
        let Some(path) = self.export_pic.take() else { return };
        let ppp = ctx.pixels_per_point();
        let r = self.canvas_rect;
        let (x0, y0) = ((r.left() * ppp) as usize, (r.top() * ppp) as usize);
        let (w, h) = (((r.width() * ppp) as usize).min(img.size[0].saturating_sub(x0)), ((r.height() * ppp) as usize).min(img.size[1].saturating_sub(y0)));
        let mut bytes = Vec::with_capacity(w * h * 4);
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                bytes.extend(img.pixels[y * img.size[0] + x].to_srgba_unmultiplied());
            }
        }
        let msg = match image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8) {
            Ok(()) => format!("Saved the map as {}", path.display()),
            Err(e) => format!("Couldn't save the picture: {e}"),
        };
        self.say(ctx, msg);
    }

    /// Coverage score of the shown storey and a suggestion where one more access point helps most.
    fn coverage_section(&mut self, ui: &mut egui::Ui) {
        // No walls, no rooms to score; a closed section isn't worked out at all.
        if !self.net.walls.iter().any(|w| w.floor == self.floor) || !Self::section(ui, "Wi-Fi coverage") {
            return;
        }
        let now = ui.input(|i| i.time);
        if !self.cov_cache.as_ref().is_some_and(|c| c.0 == self.floor && now - c.1 < 1.0) {
            self.cov_cache = Some((self.floor, now, self.net.coverage(self.floor)));
        }
        let Some(Some(c)) = self.cov_cache.as_ref().map(|c| c.2.as_ref()) else { return };
        let (good, usable, area) = (c.good, c.usable, c.area_m2);
        let col = if good >= 0.9 { GREEN } else if good >= 0.7 { AMBER } else { RED };
        ui.label(RichText::new(format!("{:.0}% of this storey (≈ {area:.0} m²) has good Wi-Fi, {:.0}% at least usable.", good * 100.0, usable * 100.0)).color(col))
            .on_hover_text(format!("Good: {:.0} dBm or better. Usable: {:.0} dBm or better. Measured inside the outline of the walls on this storey.", plan::GOOD, plan::USABLE));
        ui.horizontal_wrapped(|ui| {
            if good < 0.98 && ui.small_button(format!("{} Where would another access point help?", ic::MAGNIFYING_GLASS)).clicked() {
                self.ap_hint = self.net.suggest_ap(self.floor).map(|(p, a, b)| (self.floor, p, a, b));
                if self.ap_hint.is_none() {
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, "No spot found where one more access point adds good coverage.");
                }
            }
            if let Some((f, p, a, b)) = self.ap_hint.filter(|h| h.0 == self.floor) {
                ui.label(RichText::new(format!("Marked on the map: {:.0}% → {:.0}%", a * 100.0, b * 100.0)).color(GREEN));
                if ui.small_button(format!("{} Put one there", ic::PLUS)).on_hover_text("Adds an access point at the mark; connect it by cable to a switch or the router").clicked() {
                    self.snapshot();
                    let id = self.net.add_device(Kind::AccessPoint, p);
                    self.net.dev_mut(id).unwrap().floor = f;
                    self.sel = Sel::Dev(id);
                    self.ap_hint = None;
                    self.issues = None;
                    self.cov_cache = None;
                }
                if ui.small_button(ic::X).on_hover_text("Hide the mark").clicked() {
                    self.ap_hint = None;
                }
            }
        });
    }
}
