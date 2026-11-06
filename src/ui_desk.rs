//! Each computer's desktop: a window over the map with its installed apps, like sitting at that device. The web
//! browser and command line, files and a text editor, email, a firewall, the servers it runs (web, DNS, DHCP, mail,
//! file sharing, printing) and the software installer. Several desktops can be open side by side.
use super::*;
use crate::soft::{has_desktop, Mail, MailAccount, Soft};

/// What a desktop shows: the app grid, the installer, or one app.
#[derive(Default, PartialEq, Clone, Copy)]
pub(crate) enum Page {
    #[default]
    Home,
    Installer,
    App(Soft),
}

/// One desktop's state while it's open.
#[derive(Default)]
pub(crate) struct Desk {
    pub(crate) page: Page,
    /// Files: the folder shown; Editor: the file open (path, text as edited).
    dir: String,
    edit: Option<(String, String)>,
    /// A name being typed: new file, folder, page or mailbox.
    name: String,
    /// A new DNS record: name, type (0 = A, 1 = MX), value.
    rec: (String, usize, String),
    /// Another device's shared folder being looked at: (device, its files).
    share: Option<(u32, Vec<String>)>,
    /// Email: the mail fetched (server, index), the one being read, one being written (to, subject, text), the
    /// account form and whether it's open.
    mail: Vec<(u32, usize)>,
    reading: Option<(u32, usize)>,
    compose: Option<(String, String, String)>,
    account: (String, String),
    acct_edit: bool,
    /// A text field edit already made an undo step (until focus leaves it).
    snapped: bool,
}

impl App {
    /// Opens a device's desktop (or brings it to the front) on a page.
    pub(crate) fn open_desk(&mut self, id: u32, page: Page) {
        if !self.net.dev(id).is_some_and(|d| has_desktop(d.kind)) {
            return;
        }
        if !self.desks.contains(&id) {
            self.desks.push(id);
        }
        self.desk.entry(id).or_default().page = page;
    }

    /// The open desktops, each in its own window.
    pub(crate) fn desk_ui(&mut self, ctx: &egui::Context) {
        if self.mode == Mode::Learn {
            return;
        }
        for (n, id) in self.desks.clone().into_iter().enumerate() {
            let Some(d) = self.net.dev(id) else {
                self.desks.retain(|x| *x != id);
                continue;
            };
            let title = format!("{}  {}", kind_icon(d.kind), d.name);
            let mut open = true;
            let mut st = self.desk.remove(&id).unwrap_or_default();
            // Side by side, then a row lower.
            let w = ((self.canvas_rect.width() - 80.0) / 2.0).clamp(440.0, 640.0);
            let at = self.canvas_rect.left_top() + vec2(30.0 + (w + 16.0) * (n % 2) as f32 + 24.0 * (n / 2) as f32, 50.0 + 60.0 * (n / 2) as f32);
            egui::Window::new(title)
                .id(Id::new(("desk", id)))
                .open(&mut open)
                .default_pos(at)
                .default_size([w, 480.0])
                .min_size([420.0, 300.0])
                .collapsible(false)
                .resizable(true)
                .frame(egui::Frame::new().fill(pal().panel).stroke(Stroke::new(1.0, pal().line)).corner_radius(12).inner_margin(Margin::same(12)).shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(80) }))
                .show(ctx, |ui| self.desk_body(ui, id, &mut st));
            // An edit in a text field is one undo step until the field loses focus.
            if ctx.memory(|m| m.focused().is_none()) {
                st.snapped = false;
            }
            self.desk.insert(id, st);
            if !open {
                self.desks.retain(|x| *x != id);
            }
        }
    }

    /// An undo step before the first change in a text field (later keystrokes join it).
    fn snap_once(&mut self, st: &mut Desk, changed: bool) {
        if changed && !st.snapped {
            self.snapshot();
            st.snapped = true;
        }
    }

    fn desk_body(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        // A screen keeps its size whatever app is open.
        ui.set_min_size(vec2(400.0_f32.min(ui.available_width()), 380.0));
        let d = self.net.dev(id).unwrap().clone();
        // Top bar: back to the desktop, what's open, the device's address and power.
        ui.horizontal(|ui| {
            if st.page != Page::Home && ui.button(format!("{}  {}", ic::SQUARES_FOUR, tr("Desktop"))).clicked() {
                st.page = Page::Home;
            }
            let what = match st.page {
                Page::Home => tr("Desktop").to_string(),
                Page::Installer => tr("Software").to_string(),
                Page::App(s) => tr(s.label()).to_string(),
            };
            ui.label(RichText::new(what).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ip = d.first_ip().map_or_else(|| tr("no address yet").to_string(), |ip| ip.to_string());
                let (dot, word) = if d.off { (pal().red, tr("switched off")) } else { (pal().green, tr("on")) };
                ui.label(RichText::new(format!("{}  {ip}", ic::CIRCLE)).color(dot).small()).on_hover_text(word);
            });
        });
        ui.separator();
        if d.off {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(trf("{} is switched off.", &[&d.name])).color(pal().dim));
                if ui.button(format!("{}  {}", ic::POWER, tr("Switch on"))).clicked() {
                    self.snapshot();
                    self.net.dev_mut(id).unwrap().off = false;
                }
            });
            return;
        }
        match st.page {
            Page::Home => self.desk_home(ui, &d, st),
            Page::Installer => self.installer(ui, id),
            Page::App(s) if !d.has(s) => {
                ui.label(RichText::new(trf("{} isn't installed on this device.", &[&tr(s.label())])).color(pal().dim));
                if s.fits(d.kind) && ui.button(format!("{}  {}", ic::DOWNLOAD_SIMPLE, tr("Install"))).clicked() {
                    self.snapshot();
                    self.net.dev_mut(id).unwrap().install(s);
                }
            }
            Page::App(Soft::Browser) => self.browser(ui, id),
            Page::App(Soft::Terminal) => self.terminal(ui, id),
            Page::App(Soft::Files) => self.files_app(ui, id, st),
            Page::App(Soft::Editor) => self.editor_app(ui, id, st),
            Page::App(Soft::WebServer) => self.web_app(ui, id, st),
            Page::App(Soft::DnsServer) => self.dns_app(ui, id, st),
            Page::App(Soft::DhcpServer) => self.dhcp_app(ui, id, st),
            Page::App(Soft::MailServer) => self.mailsrv_app(ui, id, st),
            Page::App(Soft::MailClient) => self.mail_app(ui, id, st),
            Page::App(Soft::Firewall) => self.firewall_app(ui, id, st),
            Page::App(Soft::FileShare) => {
                self.server_head(ui, id, Soft::FileShare);
                ui_kit::caption(ui, tr("In the shared folder"));
                let files = self.net.dev(id).unwrap().files_in("shared");
                if files.is_empty() {
                    ui.label(RichText::new(tr("Nothing yet: put files into the 'shared' folder.")).color(pal().dim));
                }
                for (name, _) in files {
                    ui.label(format!("{}  {name}", ic::FILE_TEXT));
                }
                if ui.button(format!("{}  {}", ic::FOLDER_OPEN, tr("Open the shared folder"))).clicked() {
                    st.dir = "shared".into();
                    st.page = Page::App(Soft::Files);
                }
            }
            Page::App(Soft::PrintServer) => {
                self.server_head(ui, id, Soft::PrintServer);
                ui.label(RichText::new(tr("Takes print jobs over the network (IPP, port 631): try 'test <name> print' on another device's command line.")).color(pal().dim));
            }
        }
    }

    /// The app grid.
    fn desk_home(&mut self, ui: &mut egui::Ui, d: &net::Device, st: &mut Desk) {
        let apps: Vec<Soft> = Soft::ALL.into_iter().filter(|s| d.has(*s)).collect();
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
            for s in apps.iter().copied() {
                let state = s.is_server().then(|| d.running(s)).or((s == Soft::Firewall).then(|| d.running(s)));
                if app_tile(ui, s.icon(), tr(s.label()), state).on_hover_text(tr(s.blurb())).clicked() {
                    st.page = Page::App(s);
                }
            }
            if app_tile(ui, ic::PACKAGE, tr("Software"), None).on_hover_text(tr("Install and remove programs and servers")).clicked() {
                st.page = Page::Installer;
            }
        });
        if apps.is_empty() {
            ui.label(RichText::new(tr("Nothing installed yet: open Software to add programs.")).color(pal().dim));
        }
    }

    /// Install and remove software.
    fn installer(&mut self, ui: &mut egui::Ui, id: u32) {
        let d = self.net.dev(id).unwrap().clone();
        ui.label(RichText::new(tr("What this device can run. Servers start as soon as they're installed; removing one stops it.")).small().color(pal().dim));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for s in Soft::ALL.into_iter().filter(|s| s.fits(d.kind)) {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui_kit::icon_tile(ui, s.icon(), pal().accent, 32.0);
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - 120.0).max(120.0));
                        ui.label(RichText::new(tr(s.label())).strong());
                        ui.label(RichText::new(tr(s.blurb())).small().color(pal().dim));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if d.has(s) {
                            if ui.button(tr("Remove")).clicked() {
                                self.snapshot();
                                self.net.dev_mut(id).unwrap().uninstall(s);
                            }
                            ui.label(RichText::new(format!("{} {}", ic::CHECK, tr("installed"))).small().color(pal().green));
                        } else if ui.button(format!("{}  {}", ic::DOWNLOAD_SIMPLE, tr("Install"))).clicked() {
                            self.snapshot();
                            self.net.dev_mut(id).unwrap().install(s);
                        }
                    });
                });
            }
        });
    }

    /// A server's first line: running or stopped, Start/Stop, what it does.
    fn server_head(&mut self, ui: &mut egui::Ui, id: u32, s: Soft) {
        let on = self.net.dev(id).unwrap().running(s);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", ic::CIRCLE, if on { tr("Running") } else { tr("Stopped") })).color(if on { pal().green } else { pal().dim }));
            if ui.button(if on { tr("Stop") } else { tr("Start") }).clicked() {
                self.snapshot();
                self.net.dev_mut(id).unwrap().set_running(s, !on);
                self.issues = None;
            }
        });
        ui.label(RichText::new(tr(s.blurb())).small().color(pal().dim));
        ui.add_space(4.0);
    }

    /// The latest packet log lines about this device with these protocols, newest last.
    fn recent(&self, ui: &mut egui::Ui, id: u32, protos: &[&str], what: &str) {
        let ip = self.net.dev(id).and_then(|d| d.first_ip()).map(|i| i.to_string()).unwrap_or_default();
        let name = self.net.name(id);
        let rows: Vec<&Row> = self.rows.iter().filter(|r| protos.contains(&r.log.proto) && (r.log.dev == id || r.log.dst == ip || r.log.dst == name)).collect();
        ui_kit::caption(ui, what);
        if rows.is_empty() {
            ui.label(RichText::new(tr("Nothing yet — it shows up here as it happens in Simulate.")).small().color(pal().dim));
        }
        egui::ScrollArea::vertical().id_salt(("recent", id, what)).max_height(150.0).stick_to_bottom(true).show(ui, |ui| {
            for r in rows.iter().rev().take(40).rev() {
                ui.label(RichText::new(format!("{:>6.1}s  {} → {}  {}", r.t, r.log.src, r.log.dst, r.log.info)).monospace().small());
            }
        });
    }

    fn files_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        let d = self.net.dev(id).unwrap().clone();
        // Where we are: Home / folder / folder.
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(format!("{}  {}", ic::HOUSE, tr("Home"))).clicked() {
                st.dir.clear();
            }
            let parts: Vec<String> = st.dir.split('/').filter(|p| !p.is_empty()).map(String::from).collect();
            for (i, p) in parts.iter().enumerate() {
                ui.label(RichText::new("/").color(pal().dim));
                if ui.small_button(p).clicked() {
                    st.dir = parts[..=i].join("/");
                }
            }
        });
        let join = |dir: &str, name: &str| if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
        let mut open_file = None;
        egui::ScrollArea::vertical().id_salt(("files", id)).max_height((ui.available_height() - 110.0).max(80.0)).auto_shrink([false, true]).show(ui, |ui| {
            let (dirs, files) = (d.dirs_in(&st.dir), d.files_in(&st.dir));
            if dirs.is_empty() && files.is_empty() {
                ui.label(RichText::new(tr("This folder is empty.")).color(pal().dim));
            }
            for name in dirs {
                if ui.add(egui::Button::new(format!("{}  {name}", ic::FOLDER)).frame(false)).clicked() {
                    st.dir = join(&st.dir, &name);
                }
            }
            for (name, path) in files {
                ui.horizontal(|ui| {
                    let size = d.file(&path).map_or(0, |t| t.len());
                    if ui.add(egui::Button::new(format!("{}  {name}", ic::FILE_TEXT)).frame(false)).on_hover_text(tr("Open in the text editor")).clicked() {
                        open_file = Some(path.clone());
                    }
                    ui.label(RichText::new(format!("{size} B")).small().color(pal().dim));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(ic::TRASH).on_hover_text(tr("Delete")).clicked() {
                            self.snapshot();
                            self.net.dev_mut(id).unwrap().files.retain(|f| f.0 != path);
                        }
                        if !st.name.trim().is_empty() && ui.small_button(tr("Rename")).on_hover_text(trf("Call it {}", &[&st.name.trim()])).clicked() {
                            self.snapshot();
                            let to = join(&st.dir, st.name.trim());
                            if let Some(f) = self.net.dev_mut(id).unwrap().files.iter_mut().find(|f| f.0 == path) {
                                f.0 = crate::soft::clean(&to);
                            }
                            st.name.clear();
                        }
                    });
                });
            }
        });
        if let Some(p) = open_file {
            let text = d.file(&p).unwrap_or_default().to_string();
            st.edit = Some((p, text));
            st.page = Page::App(Soft::Editor);
            return;
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.add(TextEdit::singleline(&mut st.name).hint_text(tr("name")).desired_width(150.0));
            let name = crate::soft::clean(st.name.trim());
            if ui.add_enabled(!name.is_empty(), egui::Button::new(format!("{}  {}", ic::FILE_PLUS, tr("New file")))).clicked() {
                self.snapshot();
                let p = join(&st.dir, &name);
                self.net.dev_mut(id).unwrap().set_file(&p, "");
                st.edit = Some((crate::soft::clean(&p), String::new()));
                st.name.clear();
                st.page = Page::App(Soft::Editor);
            }
            if ui.add_enabled(!name.is_empty(), egui::Button::new(format!("{}  {}", ic::FOLDER_PLUS, tr("New folder")))).clicked() {
                self.snapshot();
                let p = join(&join(&st.dir, &name), ".keep");
                self.net.dev_mut(id).unwrap().set_file(&p, "");
                st.dir = crate::soft::clean(&join(&st.dir, &name));
                st.name.clear();
            }
        });
        // Other devices' shared folders, fetched over the network (SMB).
        let shares: Vec<(u32, String)> = self.net.devices.iter().filter(|x| x.id != id && x.running(Soft::FileShare)).map(|x| (x.id, x.name.clone())).collect();
        if !shares.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tr("Shared folders:")).color(pal().dim));
                for (sid, name) in &shares {
                    if ui.small_button(format!("{}  {name}", ic::FOLDER_SIMPLE_USER)).clicked() {
                        let host = self.net.dev(*sid).unwrap().hostname();
                        let mut o = net::Outcome::default();
                        let got = self.net.service(id, &host, "files", &mut o);
                        let now = ui.input(|i| i.time);
                        let err = o.lines.iter().find(|l| !l.is_empty()).cloned();
                        self.show_outcome(now, o);
                        match got {
                            Some(_) => st.share = Some((*sid, self.net.dev(*sid).unwrap().files.iter().filter_map(|f| f.0.strip_prefix("shared/")).filter(|n| !n.ends_with(".keep")).map(String::from).collect())),
                            None => {
                                st.share = None;
                                let ctx = ui.ctx().clone();
                                self.say(&ctx, err.unwrap_or_else(|| trf("Couldn't reach {}'s shared folder.", &[name])));
                            }
                        }
                    }
                }
            });
        }
        if let Some((sid, list)) = st.share.clone() {
            let owner = self.net.name(sid);
            egui::Frame::new().fill(pal().card).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(trf("\\\\{}\\shared", &[&self.net.dev(sid).map_or(String::new(), |x| x.hostname())])).monospace());
                    if ui.small_button(ic::X).clicked() {
                        st.share = None;
                    }
                });
                if list.is_empty() {
                    ui.label(RichText::new(tr("The shared folder is empty.")).color(pal().dim));
                }
                for name in list {
                    ui.horizontal(|ui| {
                        ui.label(format!("{}  {name}", ic::FILE_TEXT));
                        if ui.small_button(format!("{}  {}", ic::DOWNLOAD_SIMPLE, tr("Copy here"))).on_hover_text(trf("Copies it from {} into this folder", &[&owner])).clicked() {
                            let text = self.net.dev(sid).and_then(|x| x.file(&format!("shared/{name}"))).unwrap_or_default().to_string();
                            self.snapshot();
                            let p = join(&st.dir, &name);
                            self.net.dev_mut(id).unwrap().set_file(&p, &text);
                        }
                    });
                }
            });
        }
    }

    fn editor_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        let Some((path, text)) = st.edit.as_mut() else {
            ui.label(RichText::new(tr("No file open. Open one in Files, or start a new one:")).color(pal().dim));
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut st.name).hint_text(tr("e.g. notes.txt")).desired_width(180.0));
                let name = crate::soft::clean(st.name.trim());
                if ui.add_enabled(!name.is_empty(), egui::Button::new(tr("Create"))).clicked() {
                    st.edit = Some((name, String::new()));
                    st.name.clear();
                }
                if ui.button(format!("{}  {}", ic::FOLDER_OPEN, tr("Files"))).clicked() {
                    st.page = Page::App(Soft::Files);
                }
            });
            return;
        };
        let saved = self.net.dev(id).and_then(|d| d.file(path)).map(String::from);
        let changed = saved.as_deref() != Some(text.as_str());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {path}{}", ic::FILE_TEXT, if changed { " •" } else { "" })).monospace());
        });
        // Pages of the web server: say where they're served.
        let host = self.net.dev(id).map(|d| d.hostname()).unwrap_or_default();
        if let Some(rest) = path.strip_prefix("webserver/") {
            let page = if rest == "index.html" { String::new() } else { rest.to_string() };
            ui.label(RichText::new(trf("The web server serves this at http://{}/{}", &[&host, &page])).small().color(pal().accent));
        }
        let h = (ui.available_height() - 40.0).max(100.0);
        egui::ScrollArea::vertical().id_salt(("edit", id)).max_height(h).show(ui, |ui| {
            ui.add_sized([ui.available_width(), h], TextEdit::multiline(text).code_editor().desired_width(f32::INFINITY));
        });
        let (path, text) = (path.clone(), text.clone());
        ui.horizontal(|ui| {
            if ui.add_enabled(changed, egui::Button::new(format!("{}  {}", ic::FLOPPY_DISK, tr("Save")))).clicked() {
                self.snapshot();
                self.net.dev_mut(id).unwrap().set_file(&path, &text);
            }
            if ui.button(tr("Close")).clicked() {
                st.edit = None;
            }
            if changed {
                ui.label(RichText::new(tr("not saved yet")).small().color(pal().amber));
            }
        });
    }

    fn web_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        self.server_head(ui, id, Soft::WebServer);
        let d = self.net.dev(id).unwrap().clone();
        let ip = d.first_ip().map(|i| i.to_string()).unwrap_or_default();
        ui.label(trf("Answers at http://{}/ and http://{}/", &[&d.hostname(), &ip]));
        ui_kit::caption(ui, tr("Pages (in the webserver folder)"));
        for (name, path) in d.files_in("webserver") {
            ui.horizontal(|ui| {
                ui.label(format!("{}  {name}", ic::FILE_HTML));
                if ui.small_button(tr("Edit")).clicked() {
                    st.edit = Some((path.clone(), d.file(&path).unwrap_or_default().to_string()));
                    st.page = Page::App(Soft::Editor);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut st.name).hint_text(tr("new page, e.g. about")).desired_width(170.0));
            let name = crate::soft::clean(st.name.trim());
            if ui.add_enabled(!name.is_empty(), egui::Button::new(format!("{}  {}", ic::PLUS, tr("Add page")))).clicked() {
                let file = if name.contains('.') { name.clone() } else { format!("{name}.html") };
                let body = format!("<html><body><h1>{name}</h1><p><a href=\"/\">Back</a></p></body></html>");
                self.snapshot();
                self.net.dev_mut(id).unwrap().set_file(&format!("webserver/{file}"), &body);
                st.edit = Some((format!("webserver/{file}"), body));
                st.name.clear();
                st.page = Page::App(Soft::Editor);
            }
        });
        self.recent(ui, id, &["HTTP", "HTTPS"], tr("Requests"));
    }

    fn dns_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        self.server_head(ui, id, Soft::DnsServer);
        let recs = self.net.dev(id).unwrap().dns_records.clone();
        ui_kit::caption(ui, tr("Records"));
        let lines: Vec<&str> = recs.lines().collect();
        let mut drop = None;
        egui::Grid::new(("dns", id)).num_columns(4).striped(true).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
            for (i, l) in lines.iter().enumerate() {
                let w: Vec<&str> = l.split_whitespace().collect();
                let (name, kind, value) = match w.as_slice() {
                    [n, k, v] if k.eq_ignore_ascii_case("mx") => (*n, "MX", *v),
                    [n, v] => (*n, "A", *v),
                    _ => continue,
                };
                ui.label(RichText::new(name).monospace());
                ui.label(RichText::new(kind).small().color(pal().dim));
                ui.label(RichText::new(value).monospace());
                if ui.small_button(ic::TRASH).clicked() {
                    drop = Some(i);
                }
                ui.end_row();
            }
        });
        if lines.is_empty() {
            ui.label(RichText::new(tr("No records yet. Names of devices in the plan are known anyway; add others here.")).small().color(pal().dim));
        }
        if let Some(i) = drop {
            self.snapshot();
            let rest: Vec<&str> = lines.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, l)| *l).collect();
            self.net.dev_mut(id).unwrap().dns_records = rest.join("\n");
        }
        ui.horizontal_wrapped(|ui| {
            ui.add(TextEdit::singleline(&mut st.rec.0).hint_text(tr("name, e.g. shop.test")).desired_width(150.0));
            egui::ComboBox::from_id_salt(("dnstype", id)).selected_text(["A", "MX"][st.rec.1.min(1)]).width(56.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut st.rec.1, 0, "A").on_hover_text(tr("The name's address"));
                ui.selectable_value(&mut st.rec.1, 1, "MX").on_hover_text(tr("The mail server for a domain"));
            });
            ui.add(TextEdit::singleline(&mut st.rec.2).hint_text(if st.rec.1 == 1 { tr("mail server's name") } else { tr("address, e.g. 192.168.1.20") }).desired_width(150.0));
            let ok = !st.rec.0.trim().is_empty() && !st.rec.2.trim().is_empty();
            if ui.add_enabled(ok, egui::Button::new(format!("{}  {}", ic::PLUS, tr("Add")))).clicked() {
                self.snapshot();
                let line = if st.rec.1 == 1 { format!("{} MX {}", st.rec.0.trim(), st.rec.2.trim()) } else { format!("{} {}", st.rec.0.trim(), st.rec.2.trim()) };
                let d = self.net.dev_mut(id).unwrap();
                if !d.dns_records.is_empty() && !d.dns_records.ends_with('\n') {
                    d.dns_records.push('\n');
                }
                d.dns_records.push_str(&line);
                st.rec = (String::new(), st.rec.1, String::new());
            }
        });
        self.recent(ui, id, &["DNS"], tr("Questions it was asked"));
    }

    fn dhcp_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        self.server_head(ui, id, Soft::DhcpServer);
        let me = self.net.dev(id).unwrap().clone();
        egui::Grid::new(("dhcp", id)).num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(tr("Hands out")).color(pal().dim));
            let mut ch = false;
            ui.horizontal(|ui| {
                let d = self.net.dev_mut(id).unwrap();
                ch |= ui.add(TextEdit::singleline(&mut d.dhcp_from).desired_width(120.0).hint_text("192.168.1.100")).changed();
                ui.label("–");
                ch |= ui.add(TextEdit::singleline(&mut d.dhcp_to).desired_width(120.0).hint_text("192.168.1.199")).changed();
            });
            if ch {
                // The edit is already in; an undo step from just before it.
                let now = self.net.dev(id).unwrap().clone();
                if !st.snapped {
                    *self.net.dev_mut(id).unwrap() = me.clone();
                    self.snap_once(st, true);
                    *self.net.dev_mut(id).unwrap() = now;
                }
            }
            ui.end_row();
            ui.label(RichText::new(tr("Router and DNS it tells")).color(pal().dim));
            let gw = if me.kind.routes() { me.first_ip().map(|i| i.to_string()).unwrap_or_default() } else { me.gateway.clone() };
            ui.label(RichText::new(if gw.is_empty() { "—".to_string() } else { gw }).monospace());
            ui.end_row();
        });
        ui_kit::caption(ui, tr("Who has an address from it"));
        let leases = self.net.leases(id);
        if leases.is_empty() {
            ui.label(RichText::new(tr("Nobody yet: devices set to get their address automatically ask when they start.")).small().color(pal().dim));
        }
        for (dev, ip) in leases {
            ui.label(format!("{}  {}  {}", kind_icon(self.net.dev(dev).map_or(Kind::Desktop, |d| d.kind)), self.net.name(dev), ip));
        }
        self.recent(ui, id, &["DHCP"], tr("Requests"));
    }

    fn mailsrv_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        self.server_head(ui, id, Soft::MailServer);
        let d = self.net.dev(id).unwrap().clone();
        // Its domains: the MX records (on any DNS server) that point here.
        let names = [d.hostname(), d.name.to_lowercase(), d.first_ip().map(|i| i.to_string()).unwrap_or_default()];
        let domains: Vec<String> = self.net.devices.iter().filter(|s| s.services.dns).flat_map(|s| s.dns_records.lines().map(String::from).collect::<Vec<_>>()).filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            (w.len() == 3 && w[1].eq_ignore_ascii_case("mx") && names.contains(&w[2].to_lowercase())).then(|| w[0].to_string())
        }).collect();
        if domains.is_empty() {
            ui.label(RichText::new(trf("No domain points here yet. On a DNS server, add an MX record: <domain> MX {}", &[&d.hostname()])).small().color(pal().amber));
        } else {
            ui.label(trf("Mail for: {}", &[&domains.join(", ")]));
        }
        ui_kit::caption(ui, tr("Mailboxes"));
        ui.horizontal_wrapped(|ui| {
            for m in d.mailboxes.clone() {
                if ui.small_button(format!("{m}  {}", ic::X)).on_hover_text(tr("Remove this mailbox")).clicked() {
                    self.snapshot();
                    self.net.dev_mut(id).unwrap().mailboxes.retain(|x| *x != m);
                }
            }
            if d.mailboxes.is_empty() {
                ui.label(RichText::new(tr("none: it takes mail for any name")).small().color(pal().dim));
            }
        });
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut st.name).hint_text(tr("user name, e.g. anna")).desired_width(160.0));
            let name = st.name.trim().to_lowercase();
            if ui.add_enabled(!name.is_empty(), egui::Button::new(format!("{}  {}", ic::PLUS, tr("Add mailbox")))).clicked() {
                self.snapshot();
                self.net.dev_mut(id).unwrap().mailboxes.push(name);
                st.name.clear();
            }
        });
        ui_kit::caption(ui, &trf("Stored mail ({})", &[&d.inbox.len()]));
        egui::ScrollArea::vertical().id_salt(("stored", id)).max_height(140.0).show(ui, |ui| {
            for (i, m) in d.inbox.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} → {}", m.from, m.to)).small().monospace());
                    ui.label(RichText::new(if m.subject.is_empty() { &m.body } else { &m.subject }).small());
                    if ui.small_button(ic::TRASH).clicked() {
                        self.snapshot();
                        self.net.dev_mut(id).unwrap().inbox.remove(i);
                    }
                });
            }
        });
        self.recent(ui, id, &["SMTP", "IMAP"], tr("Deliveries and logins"));
    }

    fn mail_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        let d = self.net.dev(id).unwrap().clone();
        let servers: Vec<String> = self.net.devices.iter().filter(|s| s.running(Soft::MailServer) && s.id != id).map(|s| s.name.clone()).collect();
        let now = ui.input(|i| i.time);
        let ctx = ui.ctx().clone();
        // Account: the address it sends as and the server it uses.
        if d.mail_account.is_none() || st.acct_edit {
            if !st.acct_edit {
                // A first guess: this device's name at the domain whose MX names the first mail server.
                let server = servers.first().cloned().unwrap_or_default();
                let domain = self.net.lookup_name(&server).and_then(|s| {
                    let names = [s.hostname(), s.name.to_lowercase()];
                    self.net.devices.iter().filter(|x| x.services.dns).flat_map(|x| x.dns_records.lines().map(String::from).collect::<Vec<_>>()).find_map(|l| {
                        let w: Vec<&str> = l.split_whitespace().collect();
                        (w.len() == 3 && w[1].eq_ignore_ascii_case("mx") && names.contains(&w[2].to_lowercase())).then(|| w[0].to_string())
                    })
                });
                st.account = (format!("{}@{}", d.hostname(), domain.unwrap_or_else(|| "home.test".into())), server);
                st.acct_edit = true;
            }
            ui.label(RichText::new(tr("Set up the mail account")).strong());
            egui::Grid::new(("acct", id)).num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("Address")).color(pal().dim));
                ui.add(TextEdit::singleline(&mut st.account.0).hint_text("anna@school.test").desired_width(220.0));
                ui.end_row();
                ui.label(RichText::new(tr("Mail server")).color(pal().dim));
                egui::ComboBox::from_id_salt(("acctsrv", id)).selected_text(if st.account.1.is_empty() { tr("the domain's own (by MX record)").to_string() } else { st.account.1.clone() }).width(220.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.account.1, String::new(), tr("the domain's own (by MX record)"));
                    for s in &servers {
                        ui.selectable_value(&mut st.account.1, s.clone(), s);
                    }
                });
                ui.end_row();
            });
            if servers.is_empty() {
                ui.label(RichText::new(tr("No mail server in the plan yet: install one on a server (Software → Mail server).")).small().color(pal().amber));
            }
            ui.horizontal(|ui| {
                let ok = st.account.0.contains('@');
                if ui.add_enabled(ok, egui::Button::new(tr("Save account"))).clicked() {
                    self.snapshot();
                    self.net.dev_mut(id).unwrap().mail_account = Some(MailAccount { address: st.account.0.trim().to_lowercase(), server: st.account.1.clone() });
                    st.acct_edit = false;
                }
                if d.mail_account.is_some() && ui.button(tr("Cancel")).clicked() {
                    st.acct_edit = false;
                }
            });
            return;
        }
        let acct = d.mail_account.clone().unwrap();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", ic::USER_CIRCLE, acct.address)).strong());
            ui.label(RichText::new(if acct.server.is_empty() { tr("via the domain's mail server").to_string() } else { trf("via {}", &[&acct.server]) }).small().color(pal().dim));
            if ui.small_button(tr("Change")).clicked() {
                st.account = (acct.address.clone(), acct.server.clone());
                st.acct_edit = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.button(format!("{}  {}", ic::ARROWS_CLOCKWISE, tr("Check mail"))).on_hover_text(tr("Fetches the inbox from the mail server (IMAP, port 993)")).clicked() {
                let mut o = net::Outcome::default();
                st.mail = self.net.fetch_mail(id, "", &mut o);
                let msg = o.lines.iter().find(|l| !l.is_empty()).cloned();
                let failed = o.traces.iter().any(|t| !t.ok) || msg.is_some();
                self.show_outcome(now, o);
                st.reading = None;
                if failed {
                    self.say(&ctx, msg.unwrap_or_else(|| tr("Couldn't reach the mail server.").into()));
                }
            }
            if ui.button(format!("{}  {}", ic::PENCIL_SIMPLE, tr("Write"))).clicked() {
                st.compose = Some(Default::default());
                st.reading = None;
            }
        });
        ui.separator();
        if let Some((to, subject, body)) = st.compose.as_mut() {
            egui::Grid::new(("compose", id)).num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("To")).color(pal().dim));
                ui.add(TextEdit::singleline(to).hint_text("ben@school.test").desired_width(f32::INFINITY));
                ui.end_row();
                ui.label(RichText::new(tr("Subject")).color(pal().dim));
                ui.add(TextEdit::singleline(subject).desired_width(f32::INFINITY));
                ui.end_row();
            });
            ui.add_sized([ui.available_width(), (ui.available_height() - 40.0).max(80.0)], TextEdit::multiline(body));
            let (t, s, b) = (to.trim().to_string(), subject.clone(), body.clone());
            ui.horizontal(|ui| {
                if ui.add_enabled(t.contains('@') && !b.trim().is_empty(), egui::Button::new(format!("{}  {}", ic::PAPER_PLANE_TILT, tr("Send")))).on_hover_text(tr("Hands it to the mail server (SMTP, port 587), which delivers it")).clicked() {
                    let mut o = net::Outcome::default();
                    let ok = self.net.send_mail(id, &t, &s, &b, &mut o);
                    let msg = o.lines.last().cloned().unwrap_or_default();
                    self.show_outcome(now, o);
                    self.say(&ctx, msg);
                    if ok {
                        st.compose = None;
                    }
                }
                if ui.button(tr("Cancel")).clicked() {
                    st.compose = None;
                }
            });
            return;
        }
        if let Some((s, i)) = st.reading {
            let Some(m) = self.net.dev(s).and_then(|x| x.inbox.get(i)).cloned() else {
                st.reading = None;
                return;
            };
            ui.label(RichText::new(if m.subject.is_empty() { tr("(no subject)").to_string() } else { m.subject.clone() }).size(15.0).strong());
            ui.label(RichText::new(trf("From {} to {}", &[&m.from, &m.to])).small().color(pal().dim));
            ui.add_space(6.0);
            ui.label(&m.body);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Back")).clicked() {
                    st.reading = None;
                }
                if ui.button(format!("{}  {}", ic::ARROW_BEND_UP_LEFT, tr("Reply"))).clicked() {
                    st.compose = Some((m.from.clone(), format!("Re: {}", m.subject), String::new()));
                    st.reading = None;
                }
            });
            return;
        }
        let mails: Vec<(u32, usize, Mail)> = st.mail.iter().filter_map(|(s, i)| Some((*s, *i, self.net.dev(*s)?.inbox.get(*i)?.clone()))).collect();
        if mails.is_empty() {
            ui.label(RichText::new(tr("No mail fetched. 'Check mail' asks the server.")).color(pal().dim));
        }
        egui::ScrollArea::vertical().id_salt(("mails", id)).auto_shrink([false, false]).show(ui, |ui| {
            for (s, i, m) in mails.iter().rev() {
                let text = RichText::new(format!("{}   {}", m.from, if m.subject.is_empty() { m.body.chars().take(60).collect::<String>() } else { m.subject.clone() }));
                if ui.add(egui::Button::new(if m.read { text } else { text.strong() }).frame(false)).clicked() {
                    st.reading = Some((*s, *i));
                    if let Some(x) = self.net.dev_mut(*s).and_then(|x| x.inbox.get_mut(*i)) {
                        x.read = true;
                    }
                }
            }
        });
    }

    fn firewall_app(&mut self, ui: &mut egui::Ui, id: u32, st: &mut Desk) {
        let on = self.net.dev(id).unwrap().running(Soft::Firewall);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{}  {}", ic::SHIELD_CHECK, if on { tr("On") } else { tr("Off") })).color(if on { pal().green } else { pal().dim }));
            if ui.button(if on { tr("Turn off") } else { tr("Turn on") }).clicked() {
                self.snapshot();
                self.net.dev_mut(id).unwrap().set_running(Soft::Firewall, !on);
            }
        });
        ui.label(RichText::new(tr("One rule per line, the first that matches wins: allow|deny <from> <to> [port] [tcp|udp] [22:00-06:00]. from and to are an address, a network like 192.168.1.0/24, a device name or any. Replies to what this device started always get back in.")).small().color(pal().dim));
        let before = self.net.dev(id).unwrap().clone();
        let r = ui.add(TextEdit::multiline(&mut self.net.dev_mut(id).unwrap().rules).code_editor().desired_rows(6).desired_width(f32::INFINITY));
        if r.changed() && !st.snapped {
            let now = self.net.dev(id).unwrap().clone();
            *self.net.dev_mut(id).unwrap() = before;
            self.snap_once(st, true);
            *self.net.dev_mut(id).unwrap() = now;
        }
        ui_kit::caption(ui, tr("Turned away"));
        let blocked: Vec<&Row> = self.rows.iter().filter(|r| r.log.dev == id && r.log.info.contains("firewall")).collect();
        if blocked.is_empty() {
            ui.label(RichText::new(tr("Nothing blocked yet.")).small().color(pal().dim));
        }
        for r in blocked.iter().rev().take(30).rev() {
            ui.label(RichText::new(format!("{:>6.1}s  {} → {}  {}", r.t, r.log.src, r.log.dst, r.log.info)).monospace().small());
        }
    }
}

/// One app on the desktop: its icon in a tile, its name under it, and a dot for a server (green: running).
fn app_tile(ui: &mut egui::Ui, icon: &str, label: &str, running: Option<bool>) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(96.0, 84.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 10, pal().hover);
    }
    let tile = Rect::from_center_size(rect.center_top() + vec2(0.0, 26.0), Vec2::splat(42.0));
    p.rect_filled(tile, 11, pal().card);
    p.rect_stroke(tile, 11, Stroke::new(1.0, pal().line), StrokeKind::Inside);
    p.text(tile.center(), Align2::CENTER_CENTER, icon, FontId::proportional(22.0), pal().accent);
    if let Some(on) = running {
        p.circle_filled(tile.right_top() + vec2(-4.0, 4.0), 4.5, if on { pal().green } else { pal().dim });
    }
    let mut job = egui::text::LayoutJob::simple(label.to_string(), FontId::proportional(11.5), pal().text, rect.width() - 4.0);
    job.wrap.max_rows = 2;
    job.halign = Align::Center;
    let g = p.layout_job(job);
    p.galley(pos2(rect.center().x, tile.bottom() + 6.0), g, pal().text);
    resp
}
