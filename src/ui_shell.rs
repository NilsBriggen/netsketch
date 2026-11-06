//! The window's frame: the top bar (file menu, Design/Simulate, undo, storeys, settings), the left rail of
//! workspaces (Plan, Coverage, Measure, Check, Traffic, Learn) and what each one shows, and the panels that
//! float over the map (tool dock, add-device palette, search and Wi-Fi map, zoom, the packet-log window).
use super::*;
use crate::ui_kit;

/// A workspace of the left rail.
#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Space {
    Plan,
    Coverage,
    Measure,
    Check,
    Traffic,
}

impl Space {
    fn icon(self) -> &'static str {
        match self {
            Space::Plan => ic::HOUSE_LINE,
            Space::Coverage => ic::WIFI_HIGH,
            Space::Measure => ic::MAP_PIN,
            Space::Check => ic::LIST_CHECKS,
            Space::Traffic => ic::PULSE,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Space::Plan => "Plan",
            Space::Coverage => "Coverage",
            Space::Measure => "Measure",
            Space::Check => "Check",
            Space::Traffic => "Traffic",
        }
    }
}

/// Groups of the add-device palette; every shelf kind lands in exactly one.
fn group_of(k: Kind) -> usize {
    use Kind::*;
    match k {
        Internet | HomeRouter | Router | Switch | AccessPoint | Extender => 0,
        Neighbor | Interference => 2,
        _ => 1,
    }
}
const GROUPS: [&str; 3] = ["Network", "Devices", "Around you"];

impl App {
    // ---------- top bar ----------

    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let full = ui.max_rect();
        let narrow = full.width() < 1150.0;
        ui.horizontal(|ui| {
            ui.label(RichText::new(ic::SHARE_NETWORK).size(20.0).color(pal().accent));
            ui.add(TextEdit::singleline(&mut self.net.title).desired_width(if narrow { 130.0 } else { 190.0 }).frame(egui::Frame::NONE).font(FontId::proportional(14.5)))
                .on_hover_text(tr("The plan's name"));
            ui.menu_button(format!("{}  {}", tr("File"), ic::CARET_DOWN), |ui| self.file_menu(ui, &ctx));

            // Design / Simulate in the middle of the bar.
            let seg = Rect::from_center_size(pos2(full.center().x, ui.max_rect().center().y), vec2(230.0, 28.0));
            if seg.left() > ui.cursor().left() + 8.0 {
                ui.put(seg, |ui: &mut egui::Ui| self.mode_switch(ui));
            } else {
                self.mode_switch(ui);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.settings_menu(ui);
                self.storey_picker(ui, narrow);
                ui.add_space(6.0);
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new(ic::ARROW_CLOCKWISE).frame(false)).on_hover_text(tr("Redo (Ctrl+Y)")).clicked() {
                    self.redo_once(&ctx);
                }
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new(ic::ARROW_COUNTER_CLOCKWISE).frame(false)).on_hover_text(tr("Undo (Ctrl+Z)")).clicked() {
                    self.undo_once(&ctx);
                }
            });
        });
    }

    fn mode_switch(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let ctx = ui.ctx().clone();
        egui::Frame::new().fill(pal().bg).corner_radius(9).inner_margin(Margin::same(2)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (m, icon, label) in [(Mode::Design, ic::PENCIL_SIMPLE, "Design"), (Mode::Simulate, ic::PLAY, "Simulate")] {
                    if ui_kit::pill(ui, self.mode == m, format!("{icon}  {}", tr(label))).clicked() && self.mode != m {
                        self.mode = m;
                        if m == Mode::Simulate {
                            self.start_sim(&ctx);
                        }
                    }
                }
            });
        })
        .response
    }

    fn file_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.set_min_width(260.0);
        ui.menu_button(format!("{}  {}", ic::FILE_PLUS, tr("New")), |ui| {
            ui.set_min_width(340.0);
            if Self::autosave_path().exists() && ui.button(RichText::new(format!("{}  {}", ic::CLOCK_COUNTER_CLOCKWISE, tr("Recover last session"))).strong()).on_hover_text(tr("The plan as it was at the last automatic save")).clicked() {
                match std::fs::read_to_string(Self::autosave_path()).map_err(|e| e.to_string()).and_then(|t| Network::from_json(&t)) {
                    Ok(n) => self.swap(ctx, Swap::Plan(Box::new(n), true)),
                    Err(e) => self.say(ctx, trf("Couldn't read the automatic save: {}", &[&e])),
                }
                ui.close();
            }
            ui.separator();
            for (key, label, desc) in scenarios::SCENARIOS {
                if ui.button(tr(label)).on_hover_text(tr(desc)).clicked() {
                    self.swap(ctx, Swap::Plan(Box::new(net::template(key)), false));
                    ui.close();
                }
            }
        });
        if ui.button(format!("{}  {}", ic::FOLDER_OPEN, tr("Open…"))).on_hover_text(tr("Open a plan or an older .fls project (Ctrl+O). Plans, phone surveys and floor-plan pictures can also be dropped on the window.")).clicked() {
            self.swap(ctx, Swap::Open);
            ui.close();
        }
        ui.menu_button(format!("{}  {}", ic::CLOCK, tr("Recent plans")), |ui| {
            let files = recent_files();
            if files.is_empty() {
                ui.label(RichText::new(tr("No recent plans yet")).color(pal().dim));
            }
            for f in files {
                let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if ui.button(name).on_hover_text(f.display().to_string()).clicked() {
                    self.swap(ctx, Swap::Path(f));
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.button(format!("{}  {}", ic::FLOPPY_DISK, tr("Save"))).on_hover_text(tr("Save (Ctrl+S)")).clicked() {
            self.save(ctx, false);
            ui.close();
        }
        if ui.button(format!("{}  {}", ic::FLOPPY_DISK_BACK, tr("Save as…"))).on_hover_text(tr("Save as a new file (Ctrl+Shift+S)")).clicked() {
            self.save(ctx, true);
            ui.close();
        }
        ui.separator();
        if ui.button(format!("{}  {}", ic::EXPORT, tr("Export a report…"))).on_hover_text(tr("Export a report: each storey's Wi-Fi map, the access points and channels, the shopping list, open problems and addresses (a web page that prints to PDF)")).clicked() {
            self.export(ctx);
            ui.close();
        }
        if ui.button(format!("{}  {}", ic::TABLE, tr("Export lists (CSV)…"))).on_hover_text(tr("The cable schedule (from, to, type, rooms, length) and the shopping list, as two spreadsheet files")).clicked() {
            self.export_csv(ctx);
            ui.close();
        }
        if ui.button(format!("{}  {}", ic::CAMERA, tr("Save the map as a picture (PNG)"))).clicked() {
            self.export_picture(ctx);
            ui.close();
        }
    }

    fn settings_menu(&mut self, ui: &mut egui::Ui) {
        let was = self.settings;
        ui.menu_button(RichText::new(ic::GEAR).size(16.0), |ui| {
            ui.set_min_width(300.0);
            ui.label(RichText::new(tr("Language")).strong());
            ui.horizontal(|ui| {
                for (i, name) in i18n::LANGUAGES.into_iter().enumerate() {
                    if ui_kit::pill(ui, self.settings.lang == i, name).clicked() {
                        self.settings.lang = i;
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Look")).strong());
            ui.horizontal(|ui| {
                for (i, name) in ["Dark", "Light", "High contrast"].into_iter().enumerate() {
                    if ui_kit::pill(ui, self.settings.theme == i, tr(name)).clicked() {
                        self.settings.theme = i;
                    }
                }
            });
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Text and buttons")).strong());
            ui.add(egui::Slider::new(&mut self.settings.zoom, 0.8..=1.8).step_by(0.1).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
            ui.label(RichText::new(tr("Ctrl + and Ctrl − change the size too.")).small().color(pal().dim));
            ui.separator();
            ui.menu_button(format!("{}  {}", ic::KEYBOARD, tr("Keyboard shortcuts")), |ui| {
                ui.set_min_width(320.0);
                egui::Grid::new("keys").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                    for (k, what) in SHORTCUTS {
                        ui.label(RichText::new(*k).monospace().color(pal().accent));
                        ui.label(tr(what));
                        ui.end_row();
                    }
                });
                ui.label(RichText::new(tr("Right-click anything on the map for more. Hold Shift while connecting to cable a laptop.")).small().color(pal().dim));
            });
        })
        .response
        .on_hover_text(tr("Settings: colours and text size"));
        if self.settings != was {
            self.settings.apply(ui.ctx());
            self.settings.save();
        }
    }

    fn storey_picker(&mut self, ui: &mut egui::Ui, narrow: bool) {
        let mut used = self.net.used_floors();
        used.extend([0, self.floor]);
        let (lo, hi) = (*used.first().unwrap(), *used.last().unwrap());
        egui::ComboBox::from_id_salt("floor").selected_text(format!("{}  {}", ic::STACK, self.net.storey(self.floor))).width(if narrow { 120.0 } else { 160.0 }).show_ui(ui, |ui| {
            for f in (lo..=hi).rev() {
                let n = self.net.devices.iter().filter(|d| d.floor == f).count();
                ui.selectable_value(&mut self.floor, f, format!("{}{}", self.net.storey(f), if n > 0 { format!(" ({n})") } else { String::new() }));
            }
            // A new storey is a clear choice of its own; it stays once something is placed on it.
            ui.separator();
            if hi < 20 && ui.button(trf("{}  Add a storey above ({})", &[&(ic::PLUS), &(net::floor_name(hi + 1))])).clicked() {
                self.floor = hi + 1;
            }
            if lo > -3 && ui.button(trf("{}  Add a basement level ({})", &[&(ic::PLUS), &(net::floor_name(lo - 1))])).clicked() {
                self.floor = lo - 1;
            }
            ui.separator();
            let f = self.floor;
            let mut name = self.net.storey_names.iter().find(|s| s.0 == f).map(|s| s.1.clone()).unwrap_or_default();
            if ui.add(TextEdit::singleline(&mut name).hint_text(trf("Name for {}", &[&(net::floor_name(f))])).desired_width(200.0)).changed() {
                self.net.storey_names.retain(|s| s.0 != f);
                self.net.storey_names.push((f, name));
            }
            let mut h = self.net.storey_h(f);
            ui.horizontal(|ui| {
                ui.label(tr("Height"));
                if ui.add(egui::DragValue::new(&mut h).range(2.0..=10.0).speed(0.05).max_decimals(2).suffix(" m")).on_hover_text(tr("Floor to floor, ceiling and slab included: the signal between storeys travels this far")).changed() {
                    self.net.storey_heights.retain(|s| s.0 != f);
                    self.net.storey_heights.push((f, h));
                    self.heat_cache = None;
                    self.heat_job = None;
                }
            });
            let below: Vec<crate::wifi::Wall> = self.net.walls.iter().filter(|w| w.floor == f - 1).cloned().collect();
            if !below.is_empty() && ui.button(trf("{}  Copy walls from {}", &[&(ic::COPY), &(self.net.storey(f - 1))])).on_hover_text(tr("Outer walls are usually the same on every storey")).clicked() {
                self.snapshot();
                for w in below {
                    crate::scenarios::wall(&mut self.net, w.a, w.b, w.kind);
                    self.net.walls.last_mut().unwrap().floor = f;
                }
                self.heat_cache = None;
                self.heat_job = None;
                ui.close();
            }
        })
        .response
        .on_hover_text(tr("Storeys: each one has its own walls; floors between them cost signal"));
    }

    // ---------- left rail and workspaces ----------

    pub(crate) fn rail(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        let errors = issues.iter().filter(|i| i.level == Level::Error).count();
        let open = issues.iter().filter(|i| i.level != Level::Tip).count();
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(4.0);
            for s in [Space::Plan, Space::Coverage, Space::Measure, Space::Check, Space::Traffic] {
                let on = self.mode != Mode::Learn && self.space == Some(s);
                let badge = (s == Space::Check).then_some((open, if errors > 0 { pal().red } else { pal().amber }));
                if ui_kit::rail_item(ui, s.icon(), tr(s.label()), on, badge).clicked() {
                    if self.mode == Mode::Learn {
                        self.mode = Mode::Design;
                        self.space = Some(s);
                    } else {
                        self.space = if on { None } else { Some(s) };
                    }
                    // Coverage is about the Wi-Fi map: opening it shows the map.
                    if s == Space::Coverage && self.space == Some(s) {
                        self.heat = true;
                    }
                }
            }
            ui.add_space((ui.available_height() - 56.0).max(8.0));
            if ui_kit::rail_item(ui, ic::GRADUATION_CAP, tr("Learn"), self.mode == Mode::Learn, None).clicked() {
                self.mode = if self.mode == Mode::Learn { Mode::Design } else { Mode::Learn };
            }
        });
    }

    pub(crate) fn workspace(&mut self, ui: &mut egui::Ui, space: Space, issues: &[Issue]) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr(space.label())).size(16.0).strong());
            if space == Space::Traffic {
                ui.label(RichText::new(ic::INFO).color(pal().dim)).on_hover_text(tr(TRAFFIC_HINT));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new(ic::CARET_DOUBLE_LEFT).color(pal().dim)).frame(false)).on_hover_text(tr("Hide this panel")).clicked() {
                    self.space = None;
                }
            });
        });
        ui.add_space(2.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            FLAT.with(|f| f.set(Some(space.label())));
            match space {
                Space::Plan => self.plan_space(ui, issues),
                Space::Coverage => self.coverage_space(ui),
                Space::Measure => self.measurements(ui),
                Space::Check => self.check_space(ui, issues),
                Space::Traffic => self.traffic_space(ui),
            }
            FLAT.with(|f| f.set(None));
            ui.add_space(12.0);
        });
    }

    fn plan_space(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        let stp = self.stp();
        let n = &self.net;
        let errors = issues.iter().filter(|i| i.level == Level::Error).count();
        let warns = issues.iter().filter(|i| i.level == Level::Warning).count();
        // Devices, connections, and one verdict that opens the Check workspace.
        let w = (ui.available_width() - 12.0) / 3.0 - 16.0;
        let (verdict, sub, col) = match (errors, warns) {
            (0, 0) => (tr("All good").to_string(), "nothing to fix", pal().green),
            (0, w) => (w.to_string(), "to look at", pal().amber),
            (e, _) => (e.to_string(), "to fix", pal().red),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            stat(ui, &n.devices.len().to_string(), "devices", pal().text, w);
            stat(ui, &n.links.len().to_string(), "connections", pal().text, w);
            let r = stat(ui, &verdict, sub, col, w);
            let r = ui.interact(r.rect, Id::new("verdict"), Sense::click()).on_hover_text(trf("{} problems and {} warnings — click to see them in Check", &[&errors, &warns]));
            if r.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if r.clicked() {
                self.space = Some(Space::Check);
            }
        });
        if self.mode == Mode::Simulate {
            ui_kit::banner(ui, pal().accent, |ui| {
                ui.label(RichText::new(format!("{}  {}", ic::PLAY, tr("Click any device to use its apps"))).strong());
                ui.label(RichText::new(tr("Addresses were handed out by DHCP. Click a device to see its Wi-Fi or take it for a walk to watch it roam; double-click a computer to open its desktop: browser, command line, email and its servers. Packets move along the cables and every step lands in the packet log.")).small().color(pal().dim));
            });
        }
        // A new plan: how to start comes first.
        let empty = self.net.devices.is_empty() && self.net.walls.is_empty();
        if empty {
            self.how_to(ui);
        }
        if !self.net.story.is_empty() && Self::section(ui, "About this setup") {
            ui.label(RichText::new(&self.net.story).color(pal().dim));
            if let Some((_, goal)) = scenarios::EXERCISES.iter().find(|e| e.0 == self.net.scenario) {
                ui.add_space(4.0);
                ui.label(RichText::new(format!("{}  {}", ic::TARGET, trf("Your task: {}", &[&tr(goal)]))).color(pal().accent));
                if ui.button(format!("{}  {}", ic::CHECK_SQUARE, tr("Check my work"))).clicked() {
                    let ctx = ui.ctx().clone();
                    match scenarios::check_exercise(&self.net) {
                        Ok(m) | Err(m) => self.say(&ctx, m),
                    }
                }
            }
        }
        let n = &self.net;
        let (_, total) = shopping(n);
        let cable: f32 = n.links.iter().filter(|l| !l.medium.wireless() && l.medium != Medium::Powerline).map(|l| n.link_len(l)).sum::<f32>() + 0.0;
        let wifi = n.devices.iter().filter(|d| !d.kind.radio() && n.wifi_link_of(d.id).is_some()).count();
        let cur = n.currency.clone();
        let root = stp.root.map(|r| n.name(r)).unwrap_or_else(|| "—".into());
        let vl: std::collections::BTreeSet<u16> = n.devices.iter().flat_map(|d| d.ifaces.iter().map(|f| f.vlan)).filter(|v| *v != 1).collect();
        let vlans = if vl.is_empty() { tr("none").to_string() } else { vl.len().to_string() };
        // Spanning tree and VLANs only once the plan has more than one switch or uses VLANs.
        let switches = n.devices.iter().filter(|d| d.kind.bridges() && d.stp).count();
        let shown = |k: &str| match k {
            "Spanning-tree root" => switches > 1,
            "VLANs in use" => !vl.is_empty(),
            _ => true,
        };
        if Self::section(ui, "At a glance") {
            egui::Grid::new("glance").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                for (k, v) in [("Estimated cost", format!("{cur}{total:.0}")), ("Cable to lay", format!("{cable:.0} m")), ("Wi-Fi devices", wifi.to_string()), ("Spanning-tree root", root), ("VLANs in use", vlans)].into_iter().filter(|x| shown(x.0)) {
                    ui.label(RichText::new(tr(k)).color(pal().dim));
                    ui.label(v);
                    ui.end_row();
                }
            });
        }
        if self.mode == Mode::Design {
            ui.add_space(4.0);
            if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(format!("{}  {}", ic::MAGIC_WAND, tr("Plan addresses for me")))).on_hover_text(tr("Gives every network a sensible address range, turns on DHCP and fills in routes. Addresses you typed by hand are replaced; Ctrl+Z brings them back.")).clicked() {
                self.snapshot();
                let logs = self.net.auto_address();
                self.push_logs(logs);
                let ctx = ui.ctx().clone();
                self.say(&ctx, tr("Addresses planned. Check the problem list for anything left."));
            }
        }
        self.floor_picture(ui);
        if Self::section(ui, "Settings") {
            egui::Grid::new("settings").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("Floors made of")).color(pal().dim)).on_hover_text(tr("What the floors between storeys are built from: it decides how much Wi-Fi gets through to the next storey"));
                egui::ComboBox::from_id_salt("slab").selected_text(self.net.slab.label()).width(160.0).show_ui(ui, |ui| {
                    for k in WallKind::ALL {
                        ui.selectable_value(&mut self.net.slab, k, k.label());
                    }
                });
                ui.end_row();
                ui.label(RichText::new(tr("Cable to spare")).color(pal().dim)).on_hover_text(tr("Added to every estimated cable run for detours, a loop to spare and waste. Runs you typed a measured length for stay as they are."));
                let mut pct = (self.net.cable_slack * 100.0).round();
                if ui.add(egui::DragValue::new(&mut pct).range(0.0..=100.0).speed(1.0).suffix(" %")).changed() {
                    self.net.cable_slack = pct / 100.0;
                }
                ui.end_row();
                ui.label(RichText::new(tr("Country rules")).color(pal().dim)).on_hover_text(tr("Which Wi-Fi channels are allowed and how loud a router may send"));
                let before = self.net.region;
                egui::ComboBox::from_id_salt("region").selected_text(tr(self.net.region.label())).width(160.0).show_ui(ui, |ui| {
                    for r in crate::wifi::Region::ALL {
                        ui.selectable_value(&mut self.net.region, r, tr(r.label()));
                    }
                });
                if self.net.region != before {
                    let new = self.net.region;
                    self.net.region = before;
                    self.snapshot();
                    self.net.region = new;
                    // Radios pick legal channels and power for the new rules.
                    crate::wifi::Region::set(self.net.region);
                    for d in self.net.devices.iter_mut().filter(|d| d.kind.radio()) {
                        crate::wifi::normalize_radio(d);
                    }
                    self.heat_cache = None;
                }
                ui.end_row();
                ui.label(RichText::new(tr("Currency")).color(pal().dim));
                ui.add(TextEdit::singleline(&mut self.net.currency).desired_width(40.0)).on_hover_text(tr("Only the symbol shown with prices: amounts aren't converted"));
                ui.end_row();
            });
            ui.collapsing(tr("Advanced"), |ui| {
                ui.checkbox(&mut self.live_var, tr("Signals fluctuate in Simulate")).on_hover_text(tr("Fading and changing traffic while simulating. Off: Simulate uses the same steady averages as planning."));
                if ui.small_button(format!("{}  {}", ic::SHUFFLE, tr("Randomise real hardware"))).on_hover_text(trf("Real devices differ: MAC addresses, how sticky each phone is, how busy the neighbours are, how good the house wiring is. Same plan, other physical devices. (Now: {})", &[&format!("{:08x}", self.net.seed as u32)])).clicked() {
                    self.snapshot();
                    self.net.shuffle_hardware(sim::new_seed());
                }
            });
        }
        if !empty {
            self.how_to(ui);
        }
    }

    fn how_to(&mut self, ui: &mut egui::Ui) {
        if Self::section(ui, "How to") {
            for (icon, t) in [
                (ic::PLUS, "Add devices with + in the tool dock at the top left of the map: click one, or drag it to its place."),
                (ic::PLUGS_CONNECTED, "Pick Connect, then drag from one device to another."),
                (ic::WALL, "Walls: click corner after corner; right-click or Esc ends the wall."),
                (ic::MONITOR, "Double-click a computer to sit at its desktop: browser, command line, files, email, and the servers it can run (web, DNS, DHCP, mail)."),
                (ic::MAP_PIN, "Measure the real air with the NetSketch Survey phone app: Measure → Link a phone, and every spot you measure lands here as a pin."),
                (ic::KEYBOARD, "Keyboard shortcuts: the gear at the top right lists them all."),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(icon).color(pal().accent));
                    ui.label(RichText::new(tr(t)).color(pal().dim));
                });
            }
        }
    }

    fn coverage_space(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.heat, tr("Wi-Fi map")).on_hover_text(tr("Colour the floor by signal strength or speed — walls, power, bands and antennas included. Signal or speed and the band are picked on the map's card; select a device for its own map."));
        if !self.net.walls.iter().any(|w| w.floor == self.floor) {
            ui.label(RichText::new(tr("Draw this storey's walls (the Walls tool, or a floor-plan picture to trace) to get a coverage score.")).small().color(pal().dim));
        }
        self.coverage_section(ui);
        if self.mode == Mode::Design && Self::section(ui, "Channels") {
            let radios = self.net.devices.iter().filter(|d| d.kind.radio()).count();
            let near = self.net.devices.iter().filter(|d| d.kind == Kind::Neighbor).count();
            ui.label(RichText::new(trf("Your access points: {} · neighbours in the plan: {}", &[&radios, &near])).color(pal().dim));
            if ui.add_sized([ui.available_width(), 28.0], egui::Button::new(format!("{}  {}", ic::WIFI_HIGH, tr("Pick better channels")))).on_hover_text(tr("Chooses non-overlapping channels for your access points, avoiding the neighbours")).clicked() {
                self.snapshot();
                let msg = self.net.auto_channels();
                let ctx = ui.ctx().clone();
                self.say(&ctx, msg);
            }
        }
    }

    fn check_space(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        let open = issues.iter().filter(|i| i.level != Level::Tip).count();
        if Self::section(ui, "Problems") {
            if issues.is_empty() {
                ui.label(RichText::new(format!("{}  {}", ic::CHECK_CIRCLE, tr("Everything looks good."))).color(pal().green));
            } else {
                ui.label(RichText::new(trf("Problems ({})", &[&open])).small().color(pal().dim));
            }
            let fixable = issues.iter().filter(|i| i.fix.is_some()).count();
            if fixable > 1 && ui.button(RichText::new(trf("{}  Apply all {} one-click fixes", &[&(ic::WRENCH), &fixable])).color(pal().accent)).on_hover_text(tr("Every fix below in one go (Ctrl+Z undoes it)")).clicked() {
                self.snapshot();
                let n = self.net.fix_all().len();
                self.issues = None;
                let ctx = ui.ctx().clone();
                self.say(&ctx, trf("{} fixes applied — see what's left in the list.", &[&n]));
            }
            // Problems first, then tips under their own heading. A tip that says the same about several devices
            // shows once, with the others folded under it.
            let tips = issues.iter().filter(|i| i.level == Level::Tip).count();
            let same = |i: &Issue| i.dev.map_or(i.text.clone(), |d| i.text.replace(&self.net.name(d), "\u{0}"));
            let mut groups: Vec<Vec<&Issue>> = vec![];
            for i in issues.iter().filter(|i| i.level != Level::Tip) {
                groups.push(vec![i]);
            }
            for i in issues.iter().filter(|i| i.level == Level::Tip) {
                let key = same(i);
                match groups.iter_mut().find(|g| g[0].level == Level::Tip && g[0].dev.is_some() && same(g[0]) == key) {
                    Some(g) if i.dev.is_some() => g.push(i),
                    _ => groups.push(vec![i]),
                }
            }
            let mut shown_tips = false;
            let mut act: Option<(Option<u32>, Option<u32>, Option<crate::issues::Fix>)> = None;
            for (gi, g) in groups.iter().enumerate() {
                if g[0].level == Level::Tip && !shown_tips {
                    shown_tips = true;
                    ui_kit::caption(ui, &trf("Tips ({})", &[&tips]));
                }
                for (k, i) in g.iter().enumerate() {
                    if k == 1 {
                        let names: Vec<String> = g[1..].iter().filter_map(|x| x.dev.map(|d| self.net.name(d))).collect();
                        let open = egui::CollapsingHeader::new(RichText::new(trf("The same for {}", &[&names.join(", ")])).small().color(pal().dim)).id_salt(("same", gi)).show(ui, |ui| {
                            for i in &g[1..] {
                                ui.add_space(2.0);
                                let (clicked, fix) = issue_row(ui, i);
                                if clicked || fix.is_some() {
                                    act = Some((i.dev, i.link, fix));
                                }
                            }
                        });
                        let _ = open;
                        break;
                    }
                    ui.add_space(2.0);
                    let (clicked, fix) = issue_row(ui, i);
                    if clicked || fix.is_some() {
                        act = Some((i.dev, i.link, fix));
                    }
                }
            }
            match act {
                Some((_, _, Some(f))) => {
                    self.snapshot();
                    let msg = self.net.apply_fix(&f);
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, msg);
                }
                Some((Some(d), _, None)) => self.focus(d),
                Some((None, l, None)) => self.sel = l.map(Sel::Link).unwrap_or(Sel::None),
                None => {}
            }
        }
        self.weak_spots(ui);
        if Self::section(ui, "Shopping list") {
            let (items, total) = shopping(&self.net);
            egui::Grid::new("shop").striped(true).num_columns(3).spacing(vec2(12.0, 3.0)).show(ui, |ui| {
                for (what, qty, cost) in &items {
                    ui.label(what);
                    ui.label(format!("× {qty}"));
                    ui.label(format!("{}{cost:.0}", self.net.currency));
                    ui.end_row();
                }
                ui.label(RichText::new(tr("Total (estimate)")).strong());
                ui.label("");
                ui.label(RichText::new(format!("{}{total:.0}", self.net.currency)).strong().color(pal().accent));
                ui.end_row();
            });
            ui.label(RichText::new(tr("Prices are rough estimates — set your own per device. Phones, computers and TVs count as already there; tick or untick \"I already have this\" on a device to change that.")).small().color(pal().dim));
            if self.net.devices.iter().any(|d| d.kind == Kind::HomeRouter && !d.owned()) {
                ui.label(RichText::new(tr("The home router is on the list: if your provider gave you one, tick \"I already have this\" on it.")).small().color(pal().amber));
            }
            if ui.small_button(format!("{}  {}", ic::COPY, tr("Copy list"))).on_hover_text(tr("As text, to paste into a message or a spreadsheet")).clicked() {
                let mut text: String = items.iter().map(|(what, qty, cost)| format!("{what}\t{qty}\t{}{cost:.0}\n", self.net.currency)).collect();
                text += &format!("{}\t\t{}{total:.0}\n", tr("Total (estimate)"), self.net.currency);
                ui.ctx().copy_text(text);
                let ctx = ui.ctx().clone();
                self.say(&ctx, tr("Shopping list copied."));
            }
        }
        self.power_section(ui);
        self.compare_section(ui);
    }

    fn traffic_space(&mut self, ui: &mut egui::Ui) {
        self.traffic_section(ui);
        if self.mode != Mode::Simulate && self.rows.is_empty() {
            return;
        }
        if Self::section(ui, "Packet log") {
            ui.horizontal_wrapped(|ui| {
                if self.mode == Mode::Simulate {
                    let mut stepping = self.step.is_some();
                    if ui.checkbox(&mut stepping, tr("Step through")).on_hover_text(tr("Packets stop at every device; Next step moves them on and shows what the packet carries there")).changed() {
                        self.step = stepping.then_some(0);
                        // The step window shows the layers itself; two such windows would cover the map.
                        self.pkt_view = None;
                    }
                }
                ui.checkbox(&mut self.only_selected, tr("Only selected device"));
                if ui.small_button(format!("{} {}", ic::BROOM, tr("Clear"))).clicked() {
                    self.rows.clear();
                }
                if ui.small_button(ic::ARROWS_OUT_SIMPLE).on_hover_text(tr("Open the log in a big window")).clicked() {
                    self.log_window = true;
                }
            });
            ui.label(RichText::new(trf("Packet log ({})", &[&self.rows.len()])).small().color(pal().dim));
            if self.rows.is_empty() {
                ui.label(RichText::new(tr("No traffic yet. Switch to Simulate and run a command.")).color(pal().dim));
            }
            let sel = if let Sel::Dev(d) = self.sel { Some(d) } else { None };
            let mut open = None;
            // The latest lines, newest last, as compact two-line entries (the big window has the full table).
            let rows: Vec<&Row> = self.rows.iter().filter(|r| !self.only_selected || sel.is_none() || Some(r.log.dev) == sel).collect();
            for r in rows.iter().skip(rows.len().saturating_sub(60)) {
                let resp = ui
                    .vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{:.2}s", r.t)).monospace().small().color(pal().dim));
                            ui.label(RichText::new(r.log.proto).strong().color(proto_color(r.log.proto)));
                            ui.label(RichText::new(format!("{} {} {}", r.log.src, ic::ARROW_RIGHT, r.log.dst)).monospace().small());
                        });
                        ui.label(RichText::new(&r.log.info).small().color(pal().dim));
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_text(tr("Click: this packet layer by layer"));
                if resp.clicked() {
                    open = Some(r.log.clone());
                }
            }
            if let Some(l) = open {
                self.pkt_view = Some(l);
            }
        }
    }

    /// The full packet log as a table, in a window of its own.
    pub(crate) fn log_window_ui(&mut self, ctx: &egui::Context) {
        if !self.log_window {
            return;
        }
        let mut open = true;
        let mut pick = None;
        egui::Window::new(tr("Packet log")).open(&mut open).default_size([900.0, 520.0]).resizable(true).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.only_selected, tr("Only selected device"));
                if ui.small_button(format!("{} {}", ic::BROOM, tr("Clear"))).clicked() {
                    self.rows.clear();
                }
            });
            let sel = if let Sel::Dev(d) = self.sel { Some(d) } else { None };
            let rows: Vec<&Row> = self.rows.iter().filter(|r| !self.only_selected || sel.is_none() || Some(r.log.dev) == sel).collect();
            egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                pick = packet_grid(ui, "pkwin", &rows);
            });
        });
        if let Some(l) = pick {
            self.pkt_view = Some(l);
        }
        self.log_window = open;
    }

    // ---------- floating over the map ----------

    /// The tool dock (Design): Move, Connect, Walls, Measure, snapping, and the add-device palette.
    pub(crate) fn dock(&mut self, ctx: &egui::Context, rect: Rect) {
        let r = ui_kit::float(ctx, "dock", rect.left_top() + vec2(12.0, 12.0), Align2::LEFT_TOP, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.vertical(|ui| {
                for (t, icon, name, hint) in [
                    (Tool::Select, ic::CURSOR, "Move", "Move and select (V). Drag devices, pins and wall ends. Right-click anything for more."),
                    (Tool::Connect, ic::PLUGS_CONNECTED, "Connect", "Connect devices (C)"),
                    (Tool::Wall, ic::WALL, "Walls", "Draw walls (W)"),
                    (Tool::Measure, ic::RULER, "Measure", "Measure a distance (M)"),
                ] {
                    if ui_kit::icon_button(ui, icon, self.tool == t, &format!("{}\n{}", tr(name), tr(hint))).clicked() {
                        self.tool = t;
                    }
                }
                ui.separator();
                if ui_kit::icon_button(ui, ic::MAGNET, self.snap, tr("Snap to the half-metre grid")).clicked() {
                    self.snap = !self.snap;
                }
                ui.separator();
                if ui_kit::icon_button(ui, ic::PLUS, self.add_open, tr("Add a device")).clicked() {
                    self.add_open = !self.add_open;
                }
            });
        });
        let dock = r.response.rect;
        if self.add_open {
            self.add_palette(ctx, dock.right_top() + vec2(8.0, 0.0));
        } else if self.tool != Tool::Select {
            // Under the dock, so it never covers the search at the top right of a narrow map.
            self.tool_options(ctx, dock.left_bottom() + vec2(0.0, 8.0));
        }
    }

    fn tool_options(&mut self, ctx: &egui::Context, at: Pos2) {
        ui_kit::float(ctx, "tool_opts", at, Align2::LEFT_TOP, |ui| {
            egui::Frame::new().inner_margin(Margin::same(6)).show(ui, |ui| {
                ui.set_max_width(270.0);
                match self.tool {
                    Tool::Wall => {
                        egui::ComboBox::from_id_salt("wallkind").selected_text(self.wall_kind.label()).width(250.0).show_ui(ui, |ui| {
                            for k in WallKind::ALL {
                                ui.selectable_value(&mut self.wall_kind, k, k.label());
                            }
                        });
                        ui.label(RichText::new(tr("Click to start, click again to place — each click continues the wall. Right-click, double-click or Esc to stop. Points lock onto corners (green) and walls (amber); Shift keeps it straight.")).small().color(pal().dim));
                    }
                    Tool::Connect => {
                        egui::ComboBox::from_id_salt("medium").selected_text(format!("{}  {}", ic::LINK, self.medium.label())).width(250.0).show_ui(ui, |ui| {
                            for m in Medium::ALL {
                                ui.selectable_value(&mut self.medium, m, m.label());
                            }
                        });
                        ui.label(RichText::new(tr("Drag from one device to another — or click one, then the other. Laptops join by Wi-Fi; hold Shift to cable one.")).small().color(pal().dim));
                    }
                    Tool::Measure => self.measure_panel(ui),
                    Tool::Select => {}
                }
            });
        });
    }

    fn add_palette(&mut self, ctx: &egui::Context, at: Pos2) {
        ui_kit::float(ctx, "add_palette", at, Align2::LEFT_TOP, |ui| {
            egui::Frame::new().inner_margin(Margin::same(6)).show(ui, |ui| {
                ui.set_width(300.0);
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut self.add_query).hint_text(format!("{}  {}", ic::MAGNIFYING_GLASS, tr("Add a device…"))).desired_width(250.0));
                    if ui.add(egui::Button::new(ic::X).frame(false)).on_hover_text(tr("Close")).clicked() {
                        self.add_open = false;
                    }
                });
                ui.label(RichText::new(tr("Click one to add it, or drag it to its place on the map.")).small().color(pal().dim));
                let q = self.add_query.trim().to_lowercase();
                for (g, title) in GROUPS.iter().enumerate() {
                    let kinds: Vec<Kind> = Kind::SHELF.iter().copied().filter(|k| group_of(*k) == g && (q.is_empty() || k.label().to_lowercase().contains(&q) || k.blurb().to_lowercase().contains(&q))).collect();
                    if kinds.is_empty() {
                        continue;
                    }
                    ui_kit::caption(ui, tr(title));
                    egui::Grid::new(("add", g)).num_columns(4).spacing(vec2(4.0, 4.0)).show(ui, |ui| {
                        for (i, k) in kinds.iter().enumerate() {
                            self.add_tile(ui, *k);
                            if i % 4 == 3 {
                                ui.end_row();
                            }
                        }
                    });
                }
            });
        });
    }

    fn add_tile(&mut self, ui: &mut egui::Ui, k: Kind) {
        let (rect, resp) = ui.allocate_exact_size(vec2(68.0, 62.0), Sense::click_and_drag());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, 8, pal().hover);
        }
        p.text(rect.center_top() + vec2(0.0, 17.0), Align2::CENTER_CENTER, kind_icon(k), FontId::proportional(20.0), kind_color(k));
        let label = k.label();
        // The name on up to two lines, centred under the icon.
        let mut job = egui::text::LayoutJob::simple(label.to_string(), FontId::proportional(10.5), pal().text, rect.width() - 4.0);
        job.wrap.max_rows = 2;
        job.halign = Align::Center;
        let g = p.layout_job(job);
        p.galley(pos2(rect.center().x, rect.top() + 32.0), g, pal().text);
        let resp = resp.on_hover_text(format!("{}\n{}", label, k.blurb()));
        if resp.drag_started() {
            self.drag_kind = Some(k);
        }
        if resp.clicked() {
            self.snapshot();
            let c = self.to_world(self.canvas_rect.center());
            let at = self.free_spot([c.x, c.y]);
            let id = self.net.add_device(k, at);
            self.net.dev_mut(id).unwrap().floor = self.floor;
            self.sel = Sel::Dev(id);
        }
    }

    /// The spot nearest `c` (map point) with no device within about a metre on this storey, so new devices
    /// don't land on top of each other.
    pub(crate) fn free_spot(&self, c: [f32; 2]) -> [f32; 2] {
        let clear = |p: [f32; 2]| self.net.devices.iter().filter(|d| d.floor == self.floor).all(|d| (d.pos[0] - p[0]).hypot(d.pos[1] - p[1]) >= PX_PER_M * 0.9);
        for ring in 0..12 {
            let r = ring as f32 * PX_PER_M;
            let steps = (ring * 8).max(1);
            for i in 0..steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                let p = [c[0] + r * a.cos(), c[1] + r * a.sin()];
                if clear(p) {
                    return p;
                }
            }
        }
        c
    }

    /// Top right of the map: find a device, and the Wi-Fi map switch.
    pub(crate) fn map_top_right(&mut self, ctx: &egui::Context, rect: Rect) {
        ui_kit::float(ctx, "map_tr", rect.right_top() + vec2(-12.0, 12.0), Align2::RIGHT_TOP, |ui| {
            ui.horizontal(|ui| {
                // Narrow maps get a shorter field, so it stays clear of the tool dock.
                self.search_overlay(ui, (rect.width() - 260.0).clamp(90.0, 190.0));
                let on = self.heat;
                let b = egui::Button::new(RichText::new(format!("{}  {}", ic::WIFI_HIGH, tr("Wi-Fi map"))).color(if on { pal().accent } else { pal().text }))
                    .fill(if on { pal().accent.gamma_multiply(0.16) } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE);
                if ui.add(b).on_hover_text(tr("Colour the floor by signal strength — walls, power, bands and antennas included")).clicked() {
                    self.heat = !self.heat;
                }
            });
        });
    }
}

/// The Traffic section's explanation, also shown next to the Traffic workspace's title.
pub(crate) const TRAFFIC_HINT: &str = "Each device's apps (in its panel) share the cables and the air fairly. Organic: everyday apps that follow the time of day; the chart shows the whole day, click it to pick a time. Custom: you choose what each device runs, all at once at full rate. In Simulate the traffic moves along the cables and comes and goes like real apps: a page loads when clicked, a video fetches a few seconds at a time.";

thread_local! {
    /// Set while a workspace is drawn: its sections show as plain headed groups, not fold-outs,
    /// and a section with the workspace's own name shows no second title.
    pub(crate) static FLAT: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}
