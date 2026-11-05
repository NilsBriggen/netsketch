//! Window parts for companion-app measurements: pins on the map, the list of measured networks,
//! and the panel of one measuring point.
use super::*;
use crate::companion::band_of;

impl App {
    /// Pins where measurements were taken; drag them to where you stood, click one to see its readings.
    pub(crate) fn draw_points(&mut self, ui: &mut egui::Ui, painter: &egui::Painter) {
        let ids: Vec<u32> = self.net.points.iter().filter(|p| p.floor == self.floor).map(|p| p.id).collect();
        for id in ids {
            let p = self.net.points.iter().find(|p| p.id == id).unwrap();
            let c = self.to_screen(vec2(p.pos[0], p.pos[1]));
            let col = if p.placed { pal().accent } else { pal().amber };
            let selected = self.sel == Sel::Point(id);
            let r = Rect::from_center_size(c, Vec2::splat(26.0));
            painter.circle_filled(c, 13.0, pal().bg.gamma_multiply(0.9));
            painter.circle_stroke(c, 13.0, Stroke::new(if selected { 3.0 } else { 2.0 }, col));
            painter.text(c, Align2::CENTER_CENTER, ic::MAP_PIN, FontId::proportional(15.0), col);
            // Placed pins keep quiet (their name shows on hover); unplaced ones ask to be moved.
            if !p.placed || selected {
                let label = if p.placed { p.name.clone() } else { trf("{} — drag me to where you measured", &[&p.name]) };
                painter.text(c + vec2(0.0, 16.0), Align2::CENTER_TOP, label, FontId::proportional(11.0), col);
            }
            let resp = ui.interact(r, Id::new(("pin", id)), Sense::click_and_drag());
            if resp.clicked() {
                self.sel = Sel::Point(id);
            }
            if resp.drag_started() && self.mode == Mode::Design {
                self.snapshot();
            }
            if resp.dragged() && self.mode == Mode::Design {
                let d = resp.drag_delta() / self.zoom;
                let p = self.net.points.iter_mut().find(|p| p.id == id).unwrap();
                p.pos[0] += d.x;
                p.pos[1] += d.y;
                p.placed = true;
                self.sel = Sel::Point(id);
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            let p = self.net.points.iter().find(|p| p.id == id).unwrap();
            let top: Vec<String> = p.networks.iter().take(5).map(|s| format!("{:>4.0} dBm  {}", s.rssi, if s.ssid.is_empty() { tr("(hidden)") } else { &s.ssid })).collect();
            resp.on_hover_text(trf("{} — {} networks measured\n{}", &[&p.name, &(p.networks.len()), &(top.join("\n"))]));
        }
    }

    /// Keeps the phone's copy of the floor plan fresh and takes in the points it sends.
    pub(crate) fn phone_tick(&mut self, ctx: &egui::Context) {
        let Some(link) = &self.phone else { return };
        let now = Self::now(ctx);
        if now - self.phone_plan_at > 1.5 {
            self.phone_plan_at = now;
            link.set_plan(self.net.plan_pack(), &self.net.title);
        }
        let got = link.take();
        if got.is_empty() {
            return;
        }
        // Undo only gets a step when something actually arrived.
        let mut next = self.net.clone();
        let mut added = 0;
        for body in got {
            match next.import_companion(&body) {
                Ok((p, _)) => added += p,
                Err(e) => self.say(ctx, trf("The phone sent something unreadable: {}", &[&e])),
            }
        }
        if added > 0 {
            self.snapshot();
            self.net = next;
            let last = self.net.points.last().map(|p| (p.name.clone(), p.placed, p.floor));
            if let Some((name, placed, floor)) = last {
                self.floor = floor;
                self.say(ctx, if placed { trf("New measurement from the phone: '{}' — pinned where you tapped.", &[&name]) } else { trf("New measurement from the phone: '{}' — drag its pin to where you stood.", &[&name]) });
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    /// The live phone link: start it, show the code, stop it.
    fn phone_card(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        match &self.phone {
            None => {
                if ui.button(format!("{}  {}", ic::DEVICE_MOBILE, tr("Link a phone (live)"))).on_hover_text(tr("The NetSketch Survey app finds this computer on your Wi-Fi, shows your floor plan, and every measurement lands here as a pin — no files to send")).clicked() {
                    let code = format!("{:04}", crate::sim::new_seed() % 10_000);
                    match phone_link::PhoneLink::start(phone_link::TCP_PORT, phone_link::UDP_PORT, code, self.net.title.clone()) {
                        Ok(l) => {
                            l.set_plan(self.net.plan_pack(), &self.net.title);
                            self.phone = Some(l);
                        }
                        Err(e) => self.say(&ctx, trf("Couldn't start the phone link: {}", &[&e])),
                    }
                }
            }
            Some(l) => {
                let heard = l.last_seen.lock().unwrap().map(|t| t.elapsed().as_secs());
                let mut stop = false;
                egui::Frame::new().fill(pal().card).corner_radius(8).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ic::DEVICE_MOBILE).size(20.0).color(pal().accent));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(trf("Code {}", &[&l.code])).size(18.0).strong().color(pal().accent));
                            ui.label(RichText::new(format!("{}:{}", l.addr, l.port)).small().monospace().color(pal().dim));
                        });
                    });
                    ui.label(RichText::new(tr("In NetSketch Survey tap Link: it finds this computer on your Wi-Fi. Enter the code, tap where you stand on the plan, measure.")).small().color(pal().dim));
                    ui.label(RichText::new(tr("Phone doesn't find it? Type the address above on the phone. Still nothing: this computer's firewall blocks it — allow ports 47878 (TCP) and 47879 (UDP).")).small().color(pal().dim))
                        .on_hover_text(tr("Linux with ufw: sudo ufw allow 47878/tcp && sudo ufw allow 47879/udp\nWindows asks the first time — allow access on private networks.\nmacOS: allow incoming connections when asked."));
                    ui.horizontal(|ui| {
                        let (txt, col) = match heard {
                            Some(s) if s < 30 => (tr("Phone connected").to_string(), pal().green),
                            Some(s) => (trf("Phone last seen {} min ago", &[&(s / 60 + 1)]), pal().amber),
                            None => (tr("Waiting for the phone…").to_string(), pal().dim),
                        };
                        ui.label(RichText::new(txt).color(col));
                        if ui.small_button(tr("Stop")).on_hover_text(tr("Close the link")).clicked() {
                            stop = true;
                        }
                    });
                });
                if stop {
                    self.phone = None;
                }
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
    }

    /// Overview: import a survey, see every measured access point, decide what each one is, fit the model.
    pub(crate) fn measured_section(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.phone_card(ui);
        self.phone_report(ui);
        let pts = self.net.points.len();
        let unplaced = self.net.points.iter().filter(|p| !p.placed).count();
        if pts > 0 {
            ui.label(trf("{} measuring points from the companion app", &[&pts]));
            if unplaced > 0 {
                let what = if unplaced == 1 { tr("1 pin still needs its place: drag the orange pin").to_string() } else { trf("{} pins still need their place: drag the orange pins", &[&unplaced]) };
                ui.label(RichText::new(trf("{} {} to where you stood.", &[&(ic::MAP_PIN), &what])).color(pal().amber));
            }
        }
        let found = self.net.measured_networks();
        if found.is_empty() {
            return;
        }
        // What each measured access point can be linked to: your radios and the neighbours in the plan.
        let targets: Vec<(u32, String)> = self.net.devices.iter().filter(|d| d.kind.transmits() && d.kind != Kind::Interference).map(|d| (d.id, d.name.clone())).collect();
        let mut add = vec![];
        let mut link: Option<(String, Option<u32>)> = None;
        egui::ScrollArea::vertical().id_salt("measured").max_height(260.0).show(ui, |ui| {
            {
                for m in &found {
                    ui.horizontal(|ui| {
                    ui.add_sized([34.0, 18.0], egui::Label::new(RichText::new(format!("{:.0}", m.best)).monospace().color(quality_color(m.best))));
                    let chans: Vec<String> = m.radios.iter().map(|(b, c, _)| format!("{} ch {c}", b.label())).collect();
                    let busy = m.utilization.map(|u| format!(" · {}", trf("{}% busy", &[&format!("{:.0}", u * 100.0)]))).unwrap_or_default();
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 170.0).max(90.0));
                        ui.label(if m.ssid.is_empty() { tr("(hidden)") } else { &m.ssid });
                        ui.label(RichText::new(format!("{}{busy}", chans.join(" · "))).small().color(pal().dim));
                    });
                    let current = m.dev.and_then(|d| targets.iter().find(|t| t.0 == d)).map(|t| t.1.clone()).unwrap_or_else(|| tr("Not in the plan").into());
                    egui::ComboBox::from_id_salt(("belongs", &m.key)).selected_text(current).width(150.0).show_ui(ui, |ui| {
                        if ui.selectable_label(m.dev.is_none(), tr("Not in the plan")).clicked() {
                            link = Some((m.key.clone(), None));
                        }
                        if ui.selectable_label(false, format!("{} {}", ic::PLUS, tr("Add as a neighbour"))).clicked() {
                            add.push(m.key.clone());
                        }
                        ui.separator();
                        for (id, name) in &targets {
                            if ui.selectable_label(m.dev == Some(*id), name).clicked() {
                                link = Some((m.key.clone(), Some(*id)));
                            }
                        }
                    });
                    });
                    ui.add(egui::Separator::default().spacing(2.0));
                }
            }
        });
        let strangers: Vec<String> = found.iter().filter(|m| m.dev.is_none() && m.best >= -85.0).map(|m| m.key.clone()).collect();
        ui.horizontal_wrapped(|ui| {
            if !strangers.is_empty() && ui.small_button(trf("{} Add {} neighbours", &[&(ic::PLUS), &(strangers.len())])).on_hover_text(tr("Every measured network that isn't yours and is louder than −85 dBm")).clicked() {
                add.extend(strangers.iter().cloned());
            }
            if ui.small_button(format!("{} {}", ic::RULER, tr("Fit to measurements"))).on_hover_text(tr("Moves measured neighbours to where their signal fits your readings best and sets how loud every measured access point is")).clicked() {
                self.snapshot();
                let msg = self.net.fit_all_measured();
                self.say(&ctx, msg);
            }
        });
        if !add.is_empty() {
            self.snapshot();
            let n = add.iter().filter_map(|k| self.net.add_measured_neighbour(k)).count();
            self.issues = None;
            self.say(&ctx, trf("Added {} neighbours with their measured channels and busy time. Drag one to where it really is, then press Fit to measurements again.", &[&n]));
        }
        if let Some((k, d)) = link {
            self.snapshot();
            self.net.link_measured(&k, d);
        }
        if let Some(e) = self.net.measured_error() {
            ui.label(RichText::new(trf("Model vs. measurement: off by {} dB on average", &[&format!("{:.1}", e)])).color(if e < 4.0 { pal().green } else if e < 8.0 { pal().amber } else { pal().red }));
        }
    }

    /// What the phone watched over time and the devices it found on the network.
    fn phone_report(&mut self, ui: &mut egui::Ui) {
        let r = &self.net.phone;
        if let Some(w) = &r.watch {
            let count = |k: &str| w.events.iter().filter(|e| e.kind == k).count();
            let short = |t: &str| t.get(..16).unwrap_or(t).replace('T', " ");
            ui.label(RichText::new(trf("The phone watched from {} to {}", &[&short(&w.start), &short(&w.end)])).strong());
            ui.label(trf("{} channel changes · {} hand-overs · {} dropouts · {} delay spikes · {} loss bursts", &[&count("channel"), &count("roam"), &count("drop"), &count("spike"), &count("loss")]));
            if !w.events.is_empty() {
                egui::CollapsingHeader::new(trf("What happened ({})", &[&w.events.len()])).id_salt("watch_events").show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("watch_scroll").max_height(200.0).show(ui, |ui| {
                        for e in w.events.iter().rev().take(200) {
                            let who = if e.ssid.is_empty() { e.bssid.clone() } else { e.ssid.clone() };
                            let what = match e.kind.as_str() {
                                "channel" => trf("{} moved from channel {} to {}", &[&who, &e.from, &e.to]),
                                "roam" => trf("The phone moved from {} to {}", &[&e.from, &e.to]),
                                "drop" => trf("Wi-Fi gone for {} s", &[&format!("{:.0}", e.value)]),
                                "spike" => trf("Delay spike: {} ms", &[&format!("{:.0}", e.value)]),
                                "loss" => trf("{}% of pings lost", &[&format!("{:.0}", e.value)]),
                                "dip" => trf("Signal dipped to {} dBm", &[&format!("{:.0}", e.value)]),
                                _ => e.note.clone(),
                            };
                            ui.label(RichText::new(format!("{}  {what}", short(&e.time))).small());
                        }
                    });
                });
            }
        }
        if !r.findings.is_empty() {
            ui.label(RichText::new(trf("{} findings from the phone are in the problem list.", &[&r.findings.len()])).small().color(pal().dim));
        }
        if !r.lan.is_empty() {
            egui::CollapsingHeader::new(trf("Devices the phone found on the network ({})", &[&r.lan.len()])).id_salt("lan_list").show(ui, |ui| {
                for d in &r.lan {
                    let what = [d.kind.as_str(), d.vendor.as_str(), d.model.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&d.ip).monospace().small().color(pal().dim));
                        ui.label(if d.name.is_empty() { tr("(no name)") } else { &d.name });
                        ui.label(RichText::new(what).small().color(pal().dim));
                    });
                }
            });
        }
    }

    /// One measuring point: where, what the phone heard, and how the model compares.
    pub(crate) fn point_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let mut delete = false;
        let models: Vec<(String, u32)> = self.net.measured.clone();
        let Some(p) = self.net.points.iter_mut().find(|p| p.id == id) else { return };
        let sub = if p.phone.is_empty() { tr("Measuring point").to_string() } else { trf("Measured with {}", &[&p.phone]) };
        Self::header(ui, ic::MAP_PIN, if p.placed { pal().accent } else { pal().amber }, tr("Measuring point"), &sub);
        ui.add_space(4.0);
        egui::Grid::new(("pt", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(tr("Name")).color(pal().dim));
            ui.text_edit_singleline(&mut p.name);
            ui.end_row();
            ui.label(RichText::new(tr("Storey")).color(pal().dim));
            ui.add(egui::DragValue::new(&mut p.floor).range(-3..=20).custom_formatter(|v, _| net::floor_name(v as i8)));
            ui.end_row();
            ui.label(RichText::new(tr("Place")).color(pal().dim));
            ui.label(if p.placed { RichText::new(tr("on the map")).color(pal().green) } else { RichText::new(tr("not yet — drag the pin")).color(pal().amber) });
            ui.end_row();
            if let Some(c) = &p.connected {
                ui.label(RichText::new(tr("Phone was on")).color(pal().dim));
                ui.label(trf("{} · {} dBm · {} Mbit/s link", &[&c.ssid, &format!("{:.0}", c.rssi), &format!("{:.0}", c.link_mbps)]));
                ui.end_row();
                if let (Some(tx), Some(rx)) = (c.tx_mbps, c.rx_mbps) {
                    ui.label(RichText::new(tr("Link rates")).color(pal().dim));
                    let std = if c.standard.is_empty() { String::new() } else { format!(" · {}", c.standard) };
                    ui.label(trf("{} Mbit/s sending, {} receiving{}", &[&format!("{:.0}", tx), &format!("{:.0}", rx), &std]));
                    ui.end_row();
                }
            }
            if let Some(g) = &p.ping {
                ui.label(RichText::new(tr("Router ping")).color(pal().dim));
                ui.label(trf("{} ms (±{} ms, worst {} ms) · {}% lost", &[&format!("{:.0}", g.avg_ms), &format!("{:.0}", g.jitter_ms), &format!("{:.0}", g.max_ms), &format!("{:.0}", g.loss_pct)]));
                ui.end_row();
            }
            if let Some(s) = &p.speed {
                ui.label(RichText::new(tr("Speed test")).color(pal().dim));
                let text = if s.up_mbps > 0.0 {
                    trf("{} down, {} up Mbit/s · {} ms (±{} ms)", &[&format!("{:.0}", s.down_mbps), &format!("{:.0}", s.up_mbps), &format!("{:.0}", s.latency_ms), &format!("{:.0}", s.jitter_ms)])
                } else {
                    trf("{} Mbit/s · {} ms", &[&format!("{:.0}", s.down_mbps), &format!("{:.0}", s.latency_ms)])
                };
                ui.label(RichText::new(text).strong());
                ui.end_row();
            }
        });
        let p = self.net.points.iter().find(|p| p.id == id).unwrap().clone();
        if Self::section_hint(ui, "Measured here", "What the phone heard at this spot (average of its scans). Where the network belongs to a device in the plan and the pin is placed, the model's value is shown next to it.") {
            {
                for s in &p.networks {
                    ui.horizontal(|ui| {
                    ui.add_sized([34.0, 18.0], egui::Label::new(RichText::new(format!("{:.0}", s.rssi)).monospace().color(quality_color(s.rssi))));
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 110.0).max(90.0));
                        ui.label(if s.ssid.is_empty() { tr("(hidden)") } else { &s.ssid });
                        ui.label(RichText::new(trf("{} ch {} · {} MHz", &[&(band_of(s.freq).label()), &s.channel, &s.width])).small().color(pal().dim));
                        let mut extra: Vec<String> = vec![];
                        let roam: String = [(s.k, "k"), (s.v, "v"), (s.r, "r")].iter().filter(|x| x.0).map(|x| x.1).collect();
                        if !roam.is_empty() {
                            extra.push(format!("802.11{roam}"));
                        }
                        if let Some(d) = s.distance_m {
                            extra.push(trf("{} m away (RTT)", &[&format!("{:.1}", d)]));
                        }
                        if let (Some(lo), Some(hi)) = (s.rssi_min, s.rssi_max) {
                            if hi - lo >= 6.0 {
                                extra.push(trf("swung {} dB", &[&format!("{:.0}", hi - lo)]));
                            }
                        }
                        if s.wps {
                            extra.push("WPS".into());
                        }
                        if !extra.is_empty() {
                            ui.label(RichText::new(extra.join(" · ")).small().color(pal().dim));
                        }
                    });
                    let key = crate::companion::key_of(s);
                    let model = models.iter().find(|m| m.0 == key).and_then(|m| self.net.dev(m.1)).filter(|_| p.placed).map(|d| self.net.rssi_at(d, p.pos, p.floor, band_of(s.freq)));
                    match model {
                        Some(v) => {
                            let off = v - s.rssi;
                            ui.label(RichText::new(if off.abs() < 0.5 { trf("model {} (same)", &[&format!("{:.0}", v)]) } else { format!("model {v:.0} ({off:+.0})") }).small().color(if off.abs() < 4.0 { pal().green } else if off.abs() < 8.0 { pal().amber } else { pal().red }));
                        }
                        None => {}
                    }
                    });
                    ui.add(egui::Separator::default().spacing(2.0));
                }
            }
        }
        ui.add_space(6.0);
        if ui.small_button(RichText::new(format!("{}  {}", ic::TRASH, tr("Remove point"))).color(pal().red)).clicked() {
            delete = true;
        }
        if delete {
            self.snapshot();
            self.net.points.retain(|p| p.id != id);
            self.sel = Sel::None;
        }
    }
}
