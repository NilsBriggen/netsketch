// Release builds on Windows start without a console window next to the app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod companion;
mod extras;
mod fls;
mod html;
mod i18n;
mod issues;
mod lessons;
mod more;
mod net;
mod phone_link;
mod plan;
mod ports;
mod radio_ui;
mod report;
mod scenarios;
mod sim;
mod stp;
mod ui_companion;
mod ui_draw;
mod ui_extras;
mod ui_more;
mod wifi;

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke,
    StrokeKind, TextEdit, Vec2, pos2, vec2,
};
use egui_phosphor::regular as ic;
use net::{Issue, Kind, Level, Medium, Network, Security, WifiStd, PX_PER_M};

/// Closest zoom: 1 m of the map = 240 screen pixels, enough for a single room to fill the screen.
const MAX_ZOOM: f32 = 12.0;
use i18n::{tr, trf};
use radio_ui::{issue_row, quality_color, Walk};
use wifi::WallKind;
use std::collections::HashMap;
use std::path::PathBuf;

/// The colours of one theme.
struct Palette {
    bg: Color32,
    panel: Color32,
    card: Color32,
    line: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    red: Color32,
    amber: Color32,
    green: Color32,
    hover: Color32,
    press: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Dark (the default), light, and high contrast for low vision or bright rooms.
const PALETTES: [Palette; 3] = [
    Palette { bg: rgb(13, 17, 23), panel: rgb(20, 26, 34), card: rgb(28, 35, 46), line: rgb(44, 54, 68), text: rgb(220, 227, 236), dim: rgb(132, 146, 164), accent: rgb(45, 212, 191), red: rgb(248, 113, 113), amber: rgb(251, 191, 36), green: rgb(74, 222, 128), hover: rgb(38, 48, 62), press: rgb(48, 60, 76) },
    Palette { bg: rgb(246, 248, 250), panel: rgb(234, 238, 243), card: rgb(255, 255, 255), line: rgb(200, 208, 218), text: rgb(24, 31, 41), dim: rgb(84, 96, 112), accent: rgb(13, 128, 116), red: rgb(196, 38, 38), amber: rgb(161, 98, 7), green: rgb(21, 128, 61), hover: rgb(222, 228, 236), press: rgb(208, 216, 226) },
    Palette { bg: rgb(0, 0, 0), panel: rgb(10, 10, 10), card: rgb(26, 26, 26), line: rgb(160, 160, 160), text: rgb(255, 255, 255), dim: rgb(214, 214, 214), accent: rgb(0, 255, 230), red: rgb(255, 92, 92), amber: rgb(255, 214, 10), green: rgb(80, 255, 120), hover: rgb(50, 50, 50), press: rgb(70, 70, 70) },
];

/// Which of PALETTES is in use (saved in the settings file).
static THEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn pal() -> &'static Palette {
    &PALETTES[THEME.load(std::sync::atomic::Ordering::Relaxed).min(PALETTES.len() - 1)]
}

fn kind_color(k: Kind) -> Color32 {
    use Kind::*;
    match k {
        Internet => Color32::from_rgb(129, 140, 248),
        HomeRouter | Router => Color32::from_rgb(45, 212, 191),
        Switch | AccessPoint => Color32::from_rgb(56, 189, 248),
        Server | Nas => Color32::from_rgb(244, 114, 182),
        Printer | Tv | SmartHome | Camera | Console => Color32::from_rgb(251, 191, 36),
        Extender => Color32::from_rgb(251, 146, 60),
        Speaker => Color32::from_rgb(232, 121, 249),
        Neighbor => Color32::from_rgb(120, 134, 152),
        Interference => Color32::from_rgb(248, 113, 113),
        _ => Color32::from_rgb(167, 139, 250),
    }
}

fn kind_icon(k: Kind) -> &'static str {
    use Kind::*;
    match k {
        Internet => ic::GLOBE,
        HomeRouter => ic::WIFI_HIGH,
        Router => ic::ARROWS_LEFT_RIGHT,
        Switch => ic::TREE_STRUCTURE,
        AccessPoint => ic::BROADCAST,
        Desktop => ic::DESKTOP,
        Laptop => ic::LAPTOP,
        Phone => ic::DEVICE_MOBILE,
        Server => ic::HARD_DRIVES,
        Nas => ic::DATABASE,
        Printer => ic::PRINTER,
        Tv => ic::TELEVISION,
        SmartHome => ic::LIGHTBULB,
        Extender => ic::WIFI_MEDIUM,
        Speaker => ic::SPEAKER_HIGH,
        Neighbor => ic::BUILDINGS,
        Camera => ic::SECURITY_CAMERA,
        Console => ic::GAME_CONTROLLER,
        Interference => ic::LIGHTNING,
    }
}

fn medium_color(m: Medium) -> Color32 {
    match m {
        Medium::Cat5e => Color32::from_rgb(120, 134, 152),
        Medium::Cat6 => Color32::from_rgb(96, 165, 250),
        Medium::Cat6a => Color32::from_rgb(52, 211, 153),
        Medium::Fiber => Color32::from_rgb(251, 146, 60),
        Medium::WiFi => Color32::from_rgb(45, 212, 191),
        Medium::Cat5 => Color32::from_rgb(220, 120, 90),
        Medium::Powerline => Color32::from_rgb(234, 179, 8),
        Medium::Mesh => Color32::from_rgb(232, 121, 249),
        Medium::Cellular => Color32::from_rgb(250, 204, 21),
    }
}

fn level_color(l: Level) -> Color32 {
    match l {
        Level::Error => pal().red,
        Level::Warning => pal().amber,
        Level::Tip => pal().accent,
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Design,
    Simulate,
    Learn,
}

#[derive(PartialEq, Clone, Copy)]
enum Tool {
    Select,
    Connect,
    Wall,
    Measure,
}

#[derive(PartialEq, Clone, Copy)]
enum Sel {
    None,
    Dev(u32),
    Link(u32),
    Wall(u32),
    /// A measuring point from the companion app.
    Point(u32),
    Label(u32),
}

/// The keyboard shortcuts (handled in `ui` below), for the list in the top bar.
const SHORTCUTS: &[(&str, &str)] = &[
    ("V", "Move and select"),
    ("C", "Connect devices"),
    ("W", "Draw walls"),
    ("M", "Measure a distance"),
    ("F", "Fit the plan to the window"),
    ("Tab / Shift+Tab", "Pick the next or previous device"),
    ("Arrow keys", "Nudge the picked device by 25 cm"),
    ("Delete", "Remove what's selected"),
    ("Esc", "Deselect, end a wall"),
    ("Ctrl+Z / Ctrl+Y", "Undo, redo"),
    ("Ctrl+D", "Duplicate"),
    ("Ctrl+S / Ctrl+O", "Save, open"),
];

/// Look and text size, kept in the config folder next to the device library.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    /// Index into PALETTES.
    theme: usize,
    /// Everything drawn bigger or smaller (1 = normal).
    zoom: f32,
    /// Lessons whose question was answered right, one bit per lesson.
    quiz: u64,
    /// Index into i18n::LANGUAGES.
    lang: usize,
    /// Traffic drawn as dots moving along the cables in Simulate.
    flow: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { theme: 0, zoom: 1.0, quiz: 0, lang: 0, flow: true }
    }
}

impl Settings {
    fn path() -> PathBuf {
        extras::library_path().with_file_name("settings.json")
    }
    fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string(self).unwrap_or_default());
    }
    /// Colours, spacing and text sizes for the whole app.
    fn apply(&self, ctx: &egui::Context) {
        THEME.store(self.theme.min(PALETTES.len() - 1), std::sync::atomic::Ordering::Relaxed);
        i18n::set(self.lang);
        ctx.set_zoom_factor(self.zoom.clamp(0.7, 2.0));
        ctx.set_theme(if self.theme == 1 { egui::Theme::Light } else { egui::Theme::Dark });
        ctx.all_styles_mut(|s| {
        let v = &mut s.visuals;
        v.panel_fill = pal().panel;
        v.window_fill = pal().panel;
        v.extreme_bg_color = pal().bg;
        v.faint_bg_color = pal().card;
        v.override_text_color = Some(pal().text);
        v.selection.bg_fill = pal().accent.gamma_multiply(0.35);
        v.selection.stroke = Stroke::new(1.0, pal().accent);
        v.hyperlink_color = pal().accent;
        v.window_stroke = Stroke::new(1.0, pal().line);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::same(8);
        }
        v.widgets.inactive.weak_bg_fill = pal().card;
        // Checkboxes, radios and slider rails use bg_fill: the page colour keeps them visible on cards and menus.
        v.widgets.inactive.bg_fill = pal().bg;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal().line);
        v.widgets.hovered.weak_bg_fill = pal().hover;
        v.widgets.active.weak_bg_fill = pal().press;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal().line);
        s.spacing.item_spacing = vec2(8.0, 7.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.interact_size.y = 26.0;
        use egui::TextStyle as T;
        s.text_styles.insert(T::Body, FontId::proportional(14.0));
        s.text_styles.insert(T::Button, FontId::proportional(14.0));
        s.text_styles.insert(T::Heading, FontId::proportional(20.0));
        s.text_styles.insert(T::Small, FontId::proportional(12.0));
        s.text_styles.insert(T::Monospace, FontId::monospace(13.0));
        });
    }
}

/// What an in-place edit replaced, for its undo step (App::snapshot_before).
enum Before {
    Device(net::Device),
    Link(net::Link),
}

#[derive(PartialEq, Clone, Copy)]
enum Bottom {
    Problems,
    Packets,
    Shopping,
}

struct Row {
    t: f64,
    log: net::Log,
}

struct Anim {
    path: Vec<u32>,
    t0: f64,
    ok: bool,
    /// The packet's lines in the log, one per router hop (for the step-through view).
    hops: Vec<net::Log>,
}

/// Step `k` of the queued packets, counting every device each one passes: the packet and where it is.
fn step_at(anims: &[Anim], mut k: usize) -> Option<(&Anim, usize)> {
    for a in anims {
        if k < a.path.len() {
            return Some((a, k));
        }
        k -= a.path.len();
    }
    None
}

#[derive(PartialEq, Clone, Copy)]
enum SimTab {
    WiFi,
    Ports,
    Browser,
    Terminal,
    Traffic,
}

#[derive(Default)]
struct Browser {
    url: String,
    /// (address shown, page html) of the last load, and the page read once (real pages can be big).
    page: Option<(String, String)>,
    parsed: html::Page,
    error: Option<String>,
    /// A real page on its way, and a line about how long it took over this network.
    pending: Option<Fetch>,
    note: Option<String>,
}

/// A real web page being fetched. It's shown once both the real download is done and the time this
/// simulated network would need has passed — a slow network feels slow.
struct Fetch {
    shown: String,
    rx: std::sync::mpsc::Receiver<Result<String, String>>,
    started: f64,
    rtt_ms: f32,
    mbps: f32,
    /// Built-in pages (speed tests) take a fixed time instead.
    fixed: Option<f64>,
    got: Option<(Result<String, String>, f64)>,
}

/// Speed-test sites run JavaScript our browser doesn't have; they get a built-in test of this network instead.
const SPEED_TESTS: [&str; 6] = ["speedtest.net", "fast.com", "speed.cloudflare.com", "nperf.com", "speedof.me", "testmy.net"];

/// Downloads a real page (in a background thread). Sends only a generic client name.
/// Fingerprint of everything a plan saves.
fn plan_hash(net: &Network) -> u64 {
    use std::hash::Hasher;
    /// Feeds the JSON straight into the hasher: no copy of the whole plan (floor-plan pictures and all) in memory.
    struct Feed(std::collections::hash_map::DefaultHasher);
    impl std::io::Write for Feed {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.write(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut f = Feed(std::collections::hash_map::DefaultHasher::new());
    let _ = serde_json::to_writer(&mut f, net);
    f.0.finish()
}

fn recent_path() -> PathBuf {
    extras::library_path().with_file_name("recent.json")
}

/// Plans opened or saved lately, newest first (only those still there).
fn recent_files() -> Vec<PathBuf> {
    let list: Vec<PathBuf> = std::fs::read_to_string(recent_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.into_iter().filter(|p| p.exists()).collect()
}

fn remember_file(path: &std::path::Path) {
    let mut list = recent_files();
    list.retain(|p| p != path);
    list.insert(0, path.to_path_buf());
    list.truncate(6);
    let _ = std::fs::write(recent_path(), serde_json::to_string(&list).unwrap_or_default());
}

fn fetch_real(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(15))).build().into();
    let mut r = agent.get(url).header("User-Agent", "NetSketch/0.1 (network simulator)").call().map_err(|e| e.to_string())?;
    let bytes = r.body_mut().with_config().limit(3 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Default)]
struct Term {
    lines: Vec<String>,
    input: String,
}

struct App {
    net: Network,
    mode: Mode,
    tool: Tool,
    medium: Medium,
    sel: Sel,
    pan: Vec2,
    zoom: f32,
    connect_from: Option<u32>,
    drag_kind: Option<Kind>,
    shown_title: String,
    /// Live link to the phone app (running while Some), and when the floor plan was last handed over.
    phone: Option<phone_link::PhoneLink>,
    phone_plan_at: f64,
    /// What was under the pointer at the last right-click: wall, cable, and the spot on the map.
    ctx_at: Option<(Option<u32>, Option<u32>, [f32; 2])>,
    /// Measuring tape: start and (once placed) end; and the real length typed to scale a picture.
    measure: Option<([f32; 2], Option<[f32; 2]>)>,
    measure_real: f32,
    /// Where devices moved in Simulate (dragged or walking) stood before, with their Wi-Fi link then:
    /// trying things out doesn't change the plan.
    sim_home: HashMap<u32, ([f32; 2], Option<net::Link>)>,
    canvas_rect: Rect,
    bottom: Bottom,
    only_selected: bool,
    terms: HashMap<u32, Term>,
    browsers: HashMap<u32, Browser>,
    sim_tab: SimTab,
    rows: Vec<Row>,
    anims: Vec<Anim>,
    /// A packet log line opened in the layer view.
    pkt_view: Option<net::Log>,
    /// Step-through mode: packets wait at each device until "Next step"; the number is the step shown.
    step: Option<usize>,
    lesson: usize,
    heat: bool,
    /// Storey shown on the map.
    floor: i8,
    /// Live variation (fading, neighbours' traffic) in Simulate mode.
    live_var: bool,
    /// Floor plan pictures by file path (or why they couldn't be read).
    textures: HashMap<String, Result<egui::TextureHandle, String>>,
    library: Option<Vec<extras::Model>>,
    model_name: String,
    /// Search text of the product catalog in the Wi-Fi hardware section.
    catalog_query: String,
    /// A device that got new hardware: its cables go into the best of its new ports.
    replug: Option<u32>,
    /// A device or cable as it was before an edit rewrote it in place (new hardware, cable type, ports); next frame makes it an undo step.
    before: Option<Before>,
    settings: Settings,
    /// Tab (+1) or Shift+Tab (−1) pressed on the map: select the next or previous device.
    tab: Option<i32>,
    /// The answer picked for this lesson's question: (lesson, answer).
    quiz_pick: Option<(usize, usize)>,
    /// Another option to compare with: its name, the plan, and both summaries as worked out last.
    compare: Option<(String, Network, Vec<(&'static str, String, String)>)>,
    reading: f32,
    issues: Option<Vec<Issue>>,
    /// Earlier plans, each with the file it belonged to.
    undo: Vec<(Network, Option<PathBuf>)>,
    issues_at: f64,
    /// Fingerprint of the plan the problem list was made from: in Design an unchanged plan isn't checked again.
    issues_key: u64,
    /// Each device's real speed to the internet (whole path) and what limits it; refreshed with the problem list.
    e2e: HashMap<u32, (f32, String)>,
    /// Spanning tree as of the last refresh (it's costly to work out on big plans every frame).
    stp_cache: Option<crate::stp::Stp>,
    load_cache: Option<extras::Load>,
    /// The same as a long-run average, for the hover tag while traffic comes and goes.
    avg_cache: Option<extras::Load>,
    /// Wi-Fi map cells in map coordinates, with what they were computed for and when.
    /// (storey, cell size, band), the area covered in cells, the cells, when it was checked, and a fingerprint of the radios and walls.
    heat_cache: Option<((i8, i32, usize), [i32; 4], Vec<(Rect, Color32)>, f64, u64)>,
    heat_job: Option<radio_ui::HeatJob>,
    /// Wi-Fi map: 0 = best band, 1..=3 = 2.4 / 5 / 6 GHz only.
    heat_band: usize,
    /// Outdoor areas' coverage on the shown storey, and when it was worked out.
    outdoor_cache: Option<(f64, Vec<(u32, plan::Coverage)>)>,
    /// The day's load chart (time computed, every half hour: internet line, busiest link), refreshed once a second.
    day_cache: Option<(f64, Vec<(f32, f32)>)>,
    /// Coverage score of the shown storey (storey, when computed, score).
    cov_cache: Option<(i8, f64, Option<plan::Coverage>)>,
    /// Suggested spot for one more access point: storey, position, good share before → after.
    ap_hint: Option<(i8, [f32; 2], f32, f32)>,
    redo: Vec<(Network, Option<PathBuf>)>,
    snap: bool,
    search: String,
    export_pic: Option<PathBuf>,
    autosave_at: f64,
    /// Frame the plan on the next drawn frame (after opening one, once the map's size is known).
    fit_pending: bool,
    autosave_last: String,
    wall_kind: WallKind,
    wall_start: Option<[f32; 2]>,
    /// The cable whose route is being drawn on the map.
    route: Option<u32>,
    walks: HashMap<u32, Walk>,
    /// Signal history per device: (time, dBm, hand-over happened).
    hist: HashMap<u32, Vec<(f64, f32, bool)>>,
    last_sample: f64,
    file: Option<PathBuf>,
    /// Fingerprint of the plan as last opened or saved; a different one means unsaved changes.
    saved_hash: u64,
    /// Unsaved changes (checked about once a second), shown as • in the window title.
    dirty: bool,
    dirty_at: f64,
    /// Input arrived since the plan was last fingerprinted (for the problem list, and for "unsaved").
    touched: [bool; 2],
    /// Closing was asked for with unsaved changes: show "Save your changes?".
    ask_quit: bool,
    quit_ok: bool,
    /// When an arrow key last nudged a device: a run of nudges is one undo step.
    nudged: f64,
    toast: Option<(String, f64)>,
    screenshot: Option<PathBuf>,
    frame: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        // Screenshots always look the same, whatever this computer's settings are.
        let settings = if screenshot.is_some() { Settings::default() } else { Settings::load() };
        settings.apply(&cc.egui_ctx);
        let mut app = App {
            net: Network::default(),
            mode: Mode::Design,
            tool: Tool::Select,
            medium: Medium::Cat6,
            sel: Sel::None,
            pan: vec2(40.0, 20.0),
            zoom: 1.0,
            connect_from: None,
            drag_kind: None,
            shown_title: String::new(),
            phone: None,
            phone_plan_at: 0.0,
            ctx_at: None,
            measure: None,
            measure_real: 3.0,
            sim_home: HashMap::new(),
            canvas_rect: Rect::NOTHING,
            bottom: Bottom::Problems,
            only_selected: false,
            terms: HashMap::new(),
            browsers: HashMap::new(),
            sim_tab: SimTab::Browser,
            rows: vec![],
            anims: vec![],
            pkt_view: None,
            step: None,
            lesson: 0,
            heat: false,
            floor: 0,
            live_var: true,
            textures: HashMap::new(),
            library: None,
            model_name: String::new(),
            catalog_query: String::new(),
            replug: None,
            before: None,
            settings,
            tab: None,
            quiz_pick: None,
            compare: None,
            reading: -65.0,
            issues: None,
            undo: vec![],
            issues_at: 0.0,
            issues_key: 0,
            e2e: HashMap::new(),
            stp_cache: None,
            load_cache: None,
            avg_cache: None,
            heat_cache: None,
            heat_job: None,
            heat_band: 0,
            cov_cache: None,
            outdoor_cache: None,
            day_cache: None,
            ap_hint: None,
            redo: vec![],
            snap: true,
            search: String::new(),
            export_pic: None,
            autosave_at: 0.0,
            fit_pending: true,
            autosave_last: String::new(),
            wall_kind: WallKind::Brick,
            wall_start: None,
            route: None,
            walks: HashMap::new(),
            hist: HashMap::new(),
            last_sample: 0.0,
            file: None,
            saved_hash: 0,
            dirty: false,
            dirty_at: 0.0,
            touched: [true; 2],
            ask_quit: false,
            quit_ok: false,
            nudged: 0.0,
            toast: None,
            screenshot,
            frame: 0,
        };
        app.load(net::template("home"));
        let logs = app.net.run_dhcp();
        app.push_logs(logs);
        if let Some(path) = open {
            app.open_path(&cc.egui_ctx, path);
        }
        // Nothing to undo at start, and the autosave only writes once the plan really changed:
        // otherwise the start-up plan would replace last session's copy ("Recover last session").
        app.undo.clear();
        app.autosave_last = serde_json::to_string(&app.net).unwrap_or_default();
        app.saved_hash = plan_hash(&app.net);
        app
    }

    fn autosave_path() -> PathBuf {
        extras::library_path().with_file_name("autosave.netplan")
    }

    /// Every 30 s, if the plan changed, a copy goes to the config folder ("New → Recover last session").
    fn autosave(&mut self, ctx: &egui::Context) {
        let now = Self::now(ctx);
        if now - self.autosave_at < 30.0 || self.screenshot.is_some() {
            return;
        }
        self.autosave_at = now;
        let Ok(json) = serde_json::to_string(&self.net) else { return };
        if json != self.autosave_last {
            let p = Self::autosave_path();
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if std::fs::write(&p, &json).is_ok() {
                self.autosave_last = json;
            }
        }
    }

    /// Remembers the plan before a change that removes or rewrites things (Ctrl+Z goes back).
    fn snapshot(&mut self) {
        // An edit follows: nothing worked out earlier this frame may be reused, and the plan needs fingerprinting.
        self.net.memo.borrow_mut().reset(true);
        self.touched = [true; 2];
        self.redo.clear();
        // The file goes with each step, so undoing past an Open never lets Ctrl+S write over the other file.
        self.undo.push((self.net.clone(), self.file.clone()));
        if self.undo.len() > 40 {
            self.undo.remove(0);
        }
        self.issues = None;
    }

    /// An undo step for an edit already made in place: the plan as it was, with that device or cable put back.
    fn snapshot_before(&mut self, old: Before) {
        let now = self.net.clone();
        match old {
            Before::Device(o) => {
                if let Some(d) = self.net.dev_mut(o.id) {
                    *d = o;
                }
            }
            Before::Link(o) => {
                if let Some(l) = self.net.links.iter_mut().find(|l| l.id == o.id) {
                    *l = o;
                }
            }
        }
        self.snapshot();
        self.net = now;
    }

    fn load(&mut self, mut n: Network) {
        n.sanitize();
        let p = &self.net;
        if !(p.devices.is_empty() && p.walls.is_empty() && p.labels.is_empty() && p.points.is_empty() && p.plans.is_empty()) {
            self.snapshot();
        }
        self.net = n;
        self.saved_hash = plan_hash(&self.net);
        // The phone link belongs to the plan it was started for.
        self.phone = None;
        self.issues = None;
        self.heat_cache = None;
        self.heat_job = None;
        self.ap_hint = None;
        self.cov_cache = None;
        self.sel = Sel::None;
        self.terms.clear();
        self.browsers.clear();
        self.walks.clear();
        self.hist.clear();
        self.floor = 0;
        self.rows.clear();
        self.anims.clear();
        self.connect_from = None;
        self.sim_home.clear();
        self.fit_pending = true;
    }

    fn now(ctx: &egui::Context) -> f64 {
        ctx.input(|i| i.time)
    }

    fn say(&mut self, ctx: &egui::Context, msg: impl Into<String>) {
        let msg: String = msg.into();
        // Longer messages stay a bit longer (about 15 characters a second, 3–9 s).
        let secs = (msg.len() as f64 / 15.0).clamp(3.0, 9.0);
        self.toast = Some((msg, Self::now(ctx) + secs));
    }

    fn push_logs(&mut self, logs: Vec<net::Log>) {
        let base = self.rows.last().map(|r| r.t).unwrap_or(0.0);
        for (i, log) in logs.into_iter().enumerate() {
            self.rows.push(Row { t: base + 0.4 + i as f64 * 0.31, log });
        }
        // Keep the log to the latest 2000 lines so long sessions stay quick.
        if self.rows.len() > 2000 {
            self.rows.drain(..self.rows.len() - 2000);
        }
    }

    fn start_sim(&mut self, ctx: &egui::Context) {
        let logs = self.net.run_dhcp();
        let n = logs.len();
        self.push_logs(logs);
        self.say(ctx, if n > 0 { trf("Simulation running — DHCP handed out addresses ({} messages in the packet log).", &[&n]) } else { tr("Simulation running — click a device to use its browser or command line.").into() });
    }

    fn run_command(&mut self, ctx: &egui::Context, id: u32, cmd: String) {
        let mut out = self.net.exec(id, &cmd);
        let now = Self::now(ctx);
        let term = self.terms.entry(id).or_default();
        term.lines.push(format!("$ {cmd}"));
        if cmd.trim() == "clear" {
            term.lines.clear();
        }
        term.lines.extend(std::mem::take(&mut out.lines));
        self.show_outcome(now, out);
    }

    /// Animates the packets of a command and adds its log lines.
    fn show_outcome(&mut self, now: f64, out: net::Outcome) {
        let mut t0 = now;
        for t in out.traces {
            let len = t.path.len();
            let hops = t.log.into_iter().filter(|l| l.hdr.ip.is_some()).collect();
            self.anims.push(Anim { path: t.path, t0, ok: t.ok, hops });
            t0 += len as f64 * 0.28;
        }
        self.push_logs(out.log);
    }

    // ---------- file handling ----------

    /// Saves to the open file (or asks where, with `ask` or for a new plan); true when it was written.
    fn save(&mut self, ctx: &egui::Context, ask: bool) -> bool {
        let path = match (&self.file, ask) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new().add_filter(tr("Network plan"), &["netplan"]).set_file_name("network.netplan").save_file(),
        };
        let Some(path) = path else { return false };
        match serde_json::to_string_pretty(&self.net).map_err(|e| e.to_string()).and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.say(ctx, trf("Saved {}", &[&(path.display())]));
                remember_file(&path);
                self.file = Some(path);
                self.saved_hash = plan_hash(&self.net);
                self.dirty = false;
                true
            }
            Err(e) => {
                self.say(ctx, trf("Couldn't save: {}", &[&e]));
                false
            }
        }
    }

    fn open(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr("Network plan"), &["netplan", "json", "fls"]).add_filter(tr("Older .fls project"), &["fls"]).pick_file() else { return };
        self.open_path(ctx, path);
    }

    /// Opens a plan (or an old .fls project) from a file: picked, or given on the command line.
    fn open_path(&mut self, ctx: &egui::Context, path: PathBuf) {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("fls")) {
            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| fls::import(&b)) {
                Ok((mut n, skipped)) => {
                    n.title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(n.title);
                    let msg = trf("Imported {} devices and {} connections{}. Save keeps it as a new plan; the .fls file is left untouched.", &[&(n.devices.len()), &(n.links.len()), &(if skipped > 0 { format!(" ({})", trf("{} connections couldn't be brought over", &[&skipped])) } else { String::new() })]);
                    self.load(n);
                    self.say(ctx, msg);
                    remember_file(&path);
                    // Never write our format over the old project.
                    self.file = None;
                }
                Err(e) => self.say(ctx, trf("Couldn't open that file: {}", &[&e])),
            }
            return;
        }
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| Network::from_json(&s)) {
            Ok(n) => {
                let newer = n.version > net::FORMAT;
                self.load(n);
                remember_file(&path);
                self.say(ctx, if newer { trf("Opened {}. It was saved by a newer NetSketch: settings this version doesn't know are left out, and saving here drops them.", &[&(path.display())]) } else { trf("Opened {}", &[&(path.display())]) });
                self.file = Some(path);
            }
            Err(e) => self.say(ctx, trf("Couldn't open that file: {}", &[&e])),
        }
    }

    /// "Save your changes?" when the window is closed with unsaved changes.
    fn quit_dialog(&mut self, ctx: &egui::Context) {
        let mut quit = false;
        egui::Modal::new(Id::new("quit")).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.heading(tr("Save your changes?"));
            ui.label(trf("\"{}\" has changes that aren't saved yet.", &[&self.net.title]));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new(tr("Save")).strong()).clicked() && self.save(ctx, false) {
                    quit = true;
                }
                if ui.button(tr("Don't save")).on_hover_text(tr("New → Recover last session still brings it back")).clicked() {
                    // Keep the latest state as the recovery copy.
                    self.autosave_at = f64::MIN;
                    self.autosave(ctx);
                    quit = true;
                }
                if ui.button(tr("Cancel")).clicked() {
                    self.ask_quit = false;
                }
            });
        });
        if quit {
            self.ask_quit = false;
            self.quit_ok = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn export(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr("Web page (opens in any browser, prints to PDF)"), &["html"]).set_file_name("network-plan.html").save_file() else { return };
        match std::fs::write(&path, report::html(&self.net)) {
            Ok(()) => self.say(ctx, trf("Exported {}. Open it in a browser; Print → Save as PDF makes a PDF.", &[&(path.display())])),
            Err(e) => self.say(ctx, trf("Couldn't export: {}", &[&e])),
        }
    }

    // ---------- top bar ----------

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // Narrow windows: icon-only buttons (names on hover) so nothing overlaps.
        let compact = ui.available_width() < 1500.0;
        let tiny = ui.available_width() < 1150.0;
        let lab = |icon: &str, text: &str| if compact { icon.to_string() } else { format!("{icon}  {}", tr(text)) };
        ui.horizontal(|ui| {
            ui.label(RichText::new(ic::SHARE_NETWORK).size(22.0).color(pal().accent));
            if !tiny {
                ui.label(RichText::new(tr("NetSketch")).size(18.0).strong());
            }
            ui.add(TextEdit::singleline(&mut self.net.title).desired_width(if compact { 130.0 } else { 180.0 }).frame(egui::Frame::NONE).text_color(pal().dim));
            ui.add_space(if compact { 6.0 } else { 16.0 });
            let before = self.mode;
            for (m, icon, label) in [(Mode::Design, ic::PENCIL_SIMPLE, tr("Design")), (Mode::Simulate, ic::PLAY, tr("Simulate")), (Mode::Learn, ic::BOOK_OPEN, tr("Learn"))] {
                let on = self.mode == m;
                let text = if tiny && !on { icon.to_string() } else { format!("{icon}  {}", tr(label)) };
                let b = egui::Button::new(RichText::new(text).color(if on { pal().bg } else { pal().text }))
                    .fill(if on { pal().accent } else { pal().card })
                    .corner_radius(14);
                if ui.add(b).clicked() {
                    self.mode = m;
                }
            }
            if before != self.mode && self.mode == Mode::Simulate {
                self.start_sim(&ctx);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let was = self.settings;
                ui.menu_button(ic::GEAR, |ui| {
                    ui.set_min_width(240.0);
                    ui.label(RichText::new(tr("Language")).strong());
                    for (i, name) in i18n::LANGUAGES.into_iter().enumerate() {
                        ui.radio_value(&mut self.settings.lang, i, name);
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(tr("Look")).strong());
                    for (i, name) in [tr("Dark"), tr("Light"), tr("High contrast")].into_iter().enumerate() {
                        ui.radio_value(&mut self.settings.theme, i, tr(name));
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(tr("Text and buttons")).strong());
                    ui.add(egui::Slider::new(&mut self.settings.zoom, 0.8..=1.8).step_by(0.1).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                    ui.label(RichText::new(tr("Ctrl + and Ctrl − change the size too.")).small().color(pal().dim));
                })
                .response
                .on_hover_text(tr("Settings: colours and text size"));
                if self.settings != was {
                    self.settings.apply(ui.ctx());
                    self.settings.save();
                }
                ui.menu_button(ic::KEYBOARD, |ui| {
                    ui.set_min_width(300.0);
                    ui.label(RichText::new(tr("Keyboard shortcuts")).strong());
                    egui::Grid::new("keys").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                        for (k, what) in SHORTCUTS {
                            ui.label(RichText::new(*k).monospace().color(pal().accent));
                            ui.label(tr(what));
                            ui.end_row();
                        }
                    });
                    ui.label(RichText::new(tr("Right-click anything on the map for more. Hold Shift while connecting to cable a laptop.")).small().color(pal().dim));
                })
                .response
                .on_hover_text(tr("Keyboard shortcuts"));
                if ui.button(lab(ic::EXPORT, tr("Export"))).on_hover_text(tr("Export a report: each storey's Wi-Fi map, the access points and channels, the shopping list, open problems and addresses (a web page that prints to PDF)")).clicked() {
                    self.export(&ctx);
                }
                if ui.button(lab(ic::FLOPPY_DISK, tr("Save"))).on_hover_text(tr("Save (Ctrl+S)")).clicked() {
                    self.save(&ctx, false);
                }
                if ui.button(ic::FLOPPY_DISK_BACK).on_hover_text(tr("Save as a new file (Ctrl+Shift+S)")).clicked() {
                    self.save(&ctx, true);
                }
                // Right to left: the recent-plans arrow shows just right of Open.
                ui.menu_button(ic::CARET_DOWN, |ui| {
                    let files = recent_files();
                    if files.is_empty() {
                        ui.label(RichText::new(tr("No recent plans yet")).color(pal().dim));
                    }
                    for f in files {
                        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        if ui.button(name).on_hover_text(f.display().to_string()).clicked() {
                            self.open_path(&ctx, f);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tr("Recent plans"));
                if ui.button(lab(ic::FOLDER_OPEN, tr("Open"))).on_hover_text(tr("Open a plan or an older .fls project (Ctrl+O). Plans, phone surveys and floor-plan pictures can also be dropped on the window.")).clicked() {
                    self.open(&ctx);
                }
                if ui.add_enabled(!self.redo.is_empty(), egui::Button::new(ic::ARROW_CLOCKWISE)).on_hover_text(tr("Redo (Ctrl+Y)")).clicked() {
                    self.redo_once(&ctx);
                }
                if ui.add_enabled(!self.undo.is_empty(), egui::Button::new(ic::ARROW_COUNTER_CLOCKWISE)).on_hover_text(tr("Undo (Ctrl+Z)")).clicked() {
                    self.undo_once(&ctx);
                }
                ui.menu_button(lab(ic::FILE_PLUS, tr("New")), |ui| {
                    ui.set_min_width(340.0);
                    if Self::autosave_path().exists() && ui.button(RichText::new(format!("{}  {}", ic::CLOCK_COUNTER_CLOCKWISE, tr("Recover last session"))).strong()).on_hover_text(tr("The plan as it was at the last automatic save")).clicked() {
                        match std::fs::read_to_string(Self::autosave_path()).map_err(|e| e.to_string()).and_then(|t| Network::from_json(&t)) {
                            Ok(n) => {
                                self.load(n);
                                // A recovered copy must not overwrite whatever file was open before, and it isn't saved yet.
                                self.file = None;
                                self.saved_hash = 0;
                                self.mode = Mode::Design;
                            }
                            Err(e) => self.say(&ctx, trf("Couldn't read the automatic save: {}", &[&e])),
                        }
                        ui.close();
                    }
                    ui.separator();
                    for (key, label, desc) in scenarios::SCENARIOS {
                        if ui.button(RichText::new(tr(label)).strong()).on_hover_text(tr(desc)).clicked() {
                            self.load(net::template(key));
                            self.file = None;
                            if self.mode == Mode::Learn {
                                self.mode = Mode::Design;
                            }
                            ui.close();
                        }
                    }
                });
                let mut used = self.net.used_floors();
                used.extend([0, self.floor]);
                let (lo, hi) = (*used.first().unwrap(), *used.last().unwrap());
                egui::ComboBox::from_id_salt("floor").selected_text(format!("{}  {}", ic::STACK, self.net.storey(self.floor))).width(if compact { 120.0 } else { 150.0 }).show_ui(ui, |ui| {
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
                }).response.on_hover_text(tr("Storeys: each one has its own walls; floors between them cost signal"));
                ui.toggle_value(&mut self.heat, lab(ic::WIFI_HIGH, "Wi-Fi map")).on_hover_text(tr("Colour the floor by signal strength — walls, power, bands and antennas included"));
            });
        });
    }

    // ---------- left: device shelf ----------

    fn shelf(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new(tr("TOOLS")).small().color(pal().dim));
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.tool, Tool::Select, format!("{} {}", ic::CURSOR, tr("Move")));
            ui.selectable_value(&mut self.tool, Tool::Connect, format!("{} {}", ic::PLUGS_CONNECTED, tr("Connect")));
            ui.selectable_value(&mut self.tool, Tool::Wall, format!("{} {}", ic::WALL, tr("Walls")));
            ui.selectable_value(&mut self.tool, Tool::Measure, format!("{} {}", ic::RULER, tr("Measure")));
        });
        if self.tool == Tool::Measure {
            self.measure_panel(ui);
        }
        ui.checkbox(&mut self.snap, tr("Snap to the half-metre grid"));
        if self.tool == Tool::Wall {
            egui::ComboBox::from_id_salt("wallkind").selected_text(self.wall_kind.label()).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                for k in WallKind::ALL {
                    ui.selectable_value(&mut self.wall_kind, k, k.label());
                }
            });
            ui.label(RichText::new(tr("Click to start, click again to place — each click continues the wall. Right-click, double-click or Esc to stop. Points lock onto corners (green) and walls (amber); Shift keeps it straight.")).small().color(pal().dim));
            ui.add_space(8.0);
        }
        if self.tool == Tool::Connect {
            egui::ComboBox::from_id_salt("medium")
                .selected_text(format!("{}  {}", ic::LINK, self.medium.label()))
                .width(ui.available_width() - 8.0)
                .show_ui(ui, |ui| {
                    for m in Medium::ALL {
                        ui.selectable_value(&mut self.medium, m, m.label());
                    }
                });
            ui.label(RichText::new(tr("Drag from one device to another — or click one, then the other. Laptops join by Wi-Fi; hold Shift to cable one.")).small().color(pal().dim));
        }
        if self.tool == Tool::Select {
            ui.label(RichText::new(tr("Drag devices, pins and wall ends. Right-click anything for more.")).small().color(pal().dim));
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tr("ADD DEVICES")).small().color(pal().dim));
        egui::Grid::new("shelf").num_columns(2).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
            for (i, k) in Kind::SHELF.iter().enumerate() {
                // Smaller cards in narrow windows so the map keeps its room.
                let narrow = ui.ctx().content_rect().width() < 1200.0;
                let (rect, resp) = ui.allocate_exact_size(if narrow { vec2(78.0, 54.0) } else { vec2(96.0, 62.0) }, Sense::click_and_drag());
                let hovered = resp.hovered();
                let p = ui.painter();
                p.rect_filled(rect, 10, if hovered { Color32::from_rgb(36, 45, 58) } else { pal().card });
                if hovered {
                    p.rect_stroke(rect, 10, Stroke::new(1.0, kind_color(*k)), StrokeKind::Inside);
                }
                p.text(rect.center() - vec2(0.0, 9.0), Align2::CENTER_CENTER, kind_icon(*k), FontId::proportional(22.0), kind_color(*k));
                p.text(rect.center() + vec2(0.0, if narrow { 15.0 } else { 17.0 }), Align2::CENTER_CENTER, k.label(), FontId::proportional(if narrow { 10.0 } else { 11.5 }), pal().text);
                let resp = resp.on_hover_text(k.blurb());
                if resp.drag_started() {
                    self.drag_kind = Some(*k);
                }
                if resp.clicked() {
                    self.snapshot();
                    let c = self.canvas_rect.center();
                    let world = self.to_world(c) + vec2((self.net.devices.len() % 5) as f32 * 30.0, 0.0);
                    let id = self.net.add_device(*k, [world.x, world.y]);
                    self.net.dev_mut(id).unwrap().floor = self.floor;
                    self.sel = Sel::Dev(id);
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new(tr("PLAN")).small().color(pal().dim));
        if ui.add_sized([ui.available_width(), 30.0], egui::Button::new(format!("{}  {}", ic::MAGIC_WAND, tr("Plan addresses for me")))).on_hover_text(tr("Gives every network a sensible address range, turns on DHCP and fills in routes")).clicked() {
            self.snapshot();
            let logs = self.net.auto_address();
            self.push_logs(logs);
            let ctx = ui.ctx().clone();
            self.say(&ctx, tr("Addresses planned. Check the problem list for anything left."));
        }
        if ui.add_sized([ui.available_width(), 30.0], egui::Button::new(format!("{}  {}", ic::WIFI_HIGH, tr("Pick better channels")))).on_hover_text(tr("Chooses non-overlapping channels for your access points, avoiding the neighbours")).clicked() {
            self.snapshot();
            let msg = self.net.auto_channels();
            let ctx = ui.ctx().clone();
            self.say(&ctx, msg);
        }
    }

    // ---------- canvas ----------

    fn to_screen(&self, p: Vec2) -> Pos2 {
        self.canvas_rect.min + self.pan + p * self.zoom
    }

    fn to_world(&self, p: Pos2) -> Vec2 {
        (p - self.canvas_rect.min - self.pan) / self.zoom
    }

    /// Dot grid spacing in metres: the finest of 0.5 / 1 / 2 / 5 / 10 / 20 / 50 m with dots at least ~22 px apart.
    fn grid_m(&self) -> f32 {
        [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|m| m * PX_PER_M * self.zoom >= 22.0).unwrap_or(100.0)
    }

    fn dev_pos(&self, id: u32) -> Pos2 {
        self.net.dev(id).map(|d| self.to_screen(vec2(d.pos[0], d.pos[1]))).unwrap_or_default()
    }

    fn canvas(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        // Screen readers get the map as one element: what's on this storey and what's picked (Tab walks the devices).
        let picked = match self.sel {
            Sel::Dev(id) => self.net.dev(id).map(|d| format!("{}, {}", d.name, d.kind.label())),
            _ => None,
        };
        let count = self.net.devices.iter().filter(|d| d.floor == self.floor).count();
        let about = trf("Network map, {}: {} devices. {}", &[&self.net.storey(self.floor), &count, &picked.map_or_else(|| tr("Press Tab to pick a device.").to_string(), |p| trf("Picked: {}.", &[&p]))]);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, about.clone()));
        self.canvas_rect = rect;
        // Panels settle during the first frames; wait for them before measuring.
        if self.fit_pending && self.frame >= 2 && rect.width() > 50.0 {
            self.fit_pending = false;
            self.fit();
        }
        let painter = ui.painter_at(rect);
        let now = ui.input(|i| i.time);
        // Walls, distances and the grid keep the real scale.
        // A device icon covers about 0.6 m of floor, like the real box: a living room fits a TV, speakers,
        // a switch and the router. Zoomed out it stays big enough to see and click (24 px), zoomed in
        // it stops growing at 58 px. Overviews of whole sites (below 42%) shrink them like before.
        let z = (self.zoom * 0.6 * PX_PER_M / 58.0).max(self.zoom.min(0.42)).min(1.0);
        painter.rect_filled(rect, 0, pal().bg);
        let ctx = ui.ctx().clone();
        self.draw_plan(&ctx, &painter);
        // Dot grid; its spacing is shown next to the zoom level.
        let step = self.grid_m() * PX_PER_M * self.zoom;
        {
            let off = vec2(self.pan.x.rem_euclid(step), self.pan.y.rem_euclid(step));
            let mut y = rect.top() + off.y;
            while y < rect.bottom() {
                let mut x = rect.left() + off.x;
                while x < rect.right() {
                    painter.circle_filled(pos2(x, y), 1.0, Color32::from_rgb(34, 42, 54));
                    x += step;
                }
                y += step;
            }
        }
        // Zoom with the wheel, around the pointer.
        if resp.hovered() {
            // The wheel, and a trackpad pinch (or Ctrl + wheel), which egui reports as a zoom factor.
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            if scroll != 0.0 || pinch != 1.0 {
                if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                    let before = self.to_world(p);
                    self.zoom = (self.zoom * (1.0 + scroll * 0.0015) * pinch).clamp(0.1, MAX_ZOOM);
                    self.pan += (self.to_world(p) - before) * self.zoom;
                }
            }
        }

        let worst: HashMap<u32, Level> = issues.iter().filter_map(|i| i.dev.map(|d| (d, i.level))).fold(HashMap::new(), |mut m, (d, l)| {
            let e = m.entry(d).or_insert(l);
            if l < *e {
                *e = l;
            }
            m
        });
        let bad_links: HashMap<u32, Level> = issues.iter().filter_map(|i| i.link.map(|l| (l, i.level))).collect();

        if self.heat {
            self.draw_heatmap(&painter, rect, now);
        }
        let pointer = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        let hovered_wall = self.draw_walls(&painter, pointer);
        if self.mode == Mode::Design && self.tool == Tool::Select {
            self.wall_handles(ui, &painter);
        }
        if let Some((_, p, ..)) = self.ap_hint.filter(|h| h.0 == self.floor) {
            let c = self.to_screen(vec2(p[0], p[1]));
            let pulse = 1.0 + 0.15 * (now * 3.0).sin() as f32;
            painter.circle_stroke(c, 22.0 * z.max(0.6) * pulse, Stroke::new(2.5, pal().green));
            painter.text(c, Align2::CENTER_CENTER, ic::PLUS, FontId::proportional(20.0 * z.max(0.6)), pal().green);
            painter.text(c + vec2(0.0, 28.0 * z.max(0.6)), Align2::CENTER_TOP, tr("new access point here?"), FontId::proportional(11.5), pal().green);
            ui.ctx().request_repaint();
        }
        let over_device = pointer.is_some_and(|p| self.net.devices.iter().any(|d| Rect::from_center_size(self.dev_pos(d.id), Vec2::splat(58.0 * z)).contains(p)));
        if let Some(t) = self.draw_survey(&painter, pointer).filter(|_| !over_device) {
            egui::Tooltip::always_open(ui.ctx().clone(), ui.layer_id(), Id::new("survey_tip"), egui::PopupAnchor::Pointer).show(|ui| ui.label(t));
        }
        let stp = self.stp();
        let load = self.load_cache.take();

        // Links.
        let mut hovered_link = None;
        for l in &self.net.links {
            let (a, b) = (self.dev_pos(l.a), self.dev_pos(l.b));
            // A drawn cable route: through its points.
            let pts: Vec<Pos2> = std::iter::once(a).chain(l.path.iter().map(|p| self.to_screen(vec2(p[0], p[1])))).chain(std::iter::once(b)).collect();
            let selected = self.sel == Sel::Link(l.id);
            // Cables entirely on other storeys are faded like their devices and can't be picked.
            let elsewhere = [l.a, l.b].iter().all(|x| self.net.dev(*x).is_some_and(|d| d.floor != self.floor));
            let painter = {
                let mut p = painter.clone();
                if elsewhere {
                    p.multiply_opacity(0.3);
                }
                p
            };
            let near = !elsewhere && pointer.is_some_and(|p| pts.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) < 6.0));
            if near {
                hovered_link = Some(l.id);
            }
            let dead = !self.net.link_ok(l);
            let standby = stp.blocked.contains(&l.id) || dead;
            let storm = stp.storm.contains(&l.id);
            let sig = self.net.link_signal(l);
            let mut col = medium_color(l.medium);
            if let Some(lv) = bad_links.get(&l.id) {
                col = level_color(*lv);
            }
            if standby {
                col = pal().dim.gamma_multiply(0.6);
            }
            if storm {
                col = pal().red;
            }
            let w = if selected { 3.5 } else if near { 2.8 } else { 2.0 } * z.max(0.6) + if storm { 2.0 } else { 0.0 };
            if selected {
                painter.line(pts.clone(), Stroke::new(w + 6.0, pal().accent.gamma_multiply(0.2)));
            }
            if sig.is_some_and(|s| !s.alive()) {
                painter.extend(Shape::dashed_line(&pts, Stroke::new(w, pal().red.gamma_multiply(0.7)), 2.0 * z, 6.0 * z));
            } else if l.medium.wireless() || standby {
                painter.extend(Shape::dashed_line(&pts, Stroke::new(w, col.gamma_multiply(0.85)), 6.0 * z, 5.0 * z));
            } else {
                painter.line(pts.clone(), Stroke::new(w, col));
            }
            // Traffic moving along the link, one stream per direction. Spacing, size and speed follow the Mbit/s on a
            // log scale (0.3 Mbit/s: a dot now and then; 500: a fast, thick stream), so one-sided traffic looks one-sided.
            if let Some((to_a, to_b)) = load.as_ref().and_then(|ld| ld.per_link.get(&l.id)).filter(|_| self.settings.flow && !standby && !storm) {
                let (u, w) = load.as_ref().map_or((0.0, 0.0), |ld| (self.net.utilisation(ld, l.id), self.net.wanted(ld, l.id)));
                // Red: the apps want more than it carries; amber: nearly full.
                let c = if w > 1.02 { pal().red } else if u > 0.8 { pal().amber } else { pal().accent };
                let lens: Vec<f32> = pts.windows(2).map(|w| (w[1] - w[0]).length()).collect();
                let total: f32 = lens.iter().sum();
                let (zz, t) = (z.max(0.6), ui.input(|i| i.time) as f32);
                for (mbps, forward) in [(*to_b, true), (*to_a, false)] {
                    if mbps < 0.05 || total < 1.0 {
                        continue;
                    }
                    let lg = (1.0 + mbps).log10();
                    let gap = (90.0 / (1.0 + mbps).powf(0.45)).clamp(5.0, 120.0) * zz;
                    let speed = (25.0 + 25.0 * lg) * zz;
                    let r = (1.3 + 0.7 * lg).min(3.6) * zz;
                    let mut d = (t * speed).rem_euclid(gap);
                    while d < total {
                        let (mut rest, mut i) = (if forward { d } else { total - d }, 0);
                        while i + 1 < lens.len() && rest > lens[i] {
                            rest -= lens[i];
                            i += 1;
                        }
                        let dir = (pts[i + 1] - pts[i]) / lens[i].max(0.01);
                        let side = if forward { 1.0 } else { -1.0 };
                        painter.circle_filled(pts[i] + dir * rest + dir.rot90() * side * (1.0 * zz + r), r, c);
                        d += gap;
                    }
                }
                ui.ctx().request_repaint();
            }
            let k = (pts.len() - 1) / 2;
            let m = pts[k] + (pts[k + 1] - pts[k]) * 0.5;
            if standby || storm {
                let (t, c) = if storm {
                    (format!("{} {}", ic::FIRE, tr("loop!")), pal().red)
                } else if l.unplugged {
                    (format!("{} {}", ic::PLUGS, tr("unplugged")), pal().amber)
                } else if dead {
                    ("no power".to_string(), pal().dim)
                } else {
                    ("standby".to_string(), pal().dim)
                };
                let g = painter.layout_no_wrap(t, FontId::proportional(10.5 * z.max(0.7)), c);
                let r = Rect::from_center_size(m, g.size() + vec2(10.0, 4.0));
                painter.rect_filled(r, 6, pal().bg);
                painter.galley(r.min + vec2(5.0, 2.0), g, c);
            } else if let Some(u) = load.as_ref().map(|ld| self.net.utilisation(ld, l.id)).filter(|u| *u > 0.5 && sig.is_none() && !(selected || near)) {
                let c = if u > 1.0 { pal().red } else if u > 0.8 { pal().amber } else { pal().dim };
                // On a background: a busy link's dots would hide it.
                let g = painter.layout_no_wrap(trf("{}% busy", &[&format!("{:.0}", u * 100.0)]), FontId::proportional(10.0 * z.max(0.8)), c);
                let r = Rect::from_center_size(m, g.size() + vec2(8.0, 2.0));
                painter.rect_filled(r, 5, pal().bg);
                painter.galley(r.min + vec2(4.0, 1.0), g, c);
            }
            if selected || near {
                let mut txt = format!("{} · {} m · ≈ {:.0} Mbit/s", l.medium.label(), self.net.link_len(l), self.net.link_rate(l));
                if let Some(s) = sig {
                    txt = trf("Wi-Fi {} dBm on {} · {} wall(s) · ≈ {} Mbit/s", &[&format!("{:.0}", s.rssi), &s.band.label(), &s.walls, &format!("{:.0}", s.mbps)]);
                    txt += "\n";
                    txt += &if self.net.rejoining(l.id) { tr("Lost the access point: joining again…").to_string() } else { self.net.steadiness(l).unwrap_or_default() };
                }
                if let Some(ld) = load.as_ref() {
                    let (a, b) = ld.per_link.get(&l.id).copied().unwrap_or_default();
                    let mbps = |x: f32| if x < 10.0 { format!("{x:.1}") } else { format!("{x:.0}") };
                    let (u, w) = (self.net.utilisation(ld, l.id), self.net.wanted(ld, l.id));
                    txt += "\n";
                    txt += &if a + b < 0.01 {
                        tr("Now: no app traffic").to_string()
                    } else {
                        trf("Now {} Mbit/s to {}, {} to {} · {}% full", &[&mbps(b), &self.net.name(l.b), &mbps(a), &self.net.name(l.a), &format!("{:.0}", u * 100.0)])
                    };
                    if w > u + 0.02 {
                        txt += &trf(" · the apps want {}%", &[&format!("{:.0}", w * 100.0)]);
                    }
                    // While traffic comes and goes, the long-run average too.
                    if let Some((a, b)) = self.avg_cache.as_ref().filter(|_| self.net.live && (!self.net.custom_traffic || self.net.custom_bursty)).map(|av| av.per_link.get(&l.id).copied().unwrap_or_default()) {
                        txt += "\n";
                        txt += &trf("On average {} Mbit/s to {}, {} to {}", &[&mbps(b), &self.net.name(l.b), &mbps(a), &self.net.name(l.a)]);
                    }
                }
                let g = painter.layout_no_wrap(txt, FontId::proportional(11.5), pal().text);
                let r = Rect::from_center_size(m, g.size() + vec2(12.0, 6.0));
                painter.rect_filled(r, 6, pal().card);
                painter.galley(r.min + vec2(6.0, 3.0), g, pal().text);
            }
        }

        // Thread/Zigbee gadgets: a dotted line to the hub or the gadget that relays for them.
        for d in self.net.devices.iter().filter(|d| d.on_802154() && d.floor == self.floor) {
            if let Some((_, h, _, _)) = self.net.smart_route(d) {
                painter.extend(Shape::dotted_line(&[self.dev_pos(d.id), self.dev_pos(h)], kind_color(Kind::SmartHome).gamma_multiply(0.7), 7.0 * z.max(0.6), 1.3));
            }
        }
        // Directional antennas: draw their beam when selected or on the Wi-Fi map.
        for d in self.net.devices.iter().filter(|d| d.beam > 0.0 && d.floor == self.floor && (self.heat || self.sel == Sel::Dev(d.id))) {
            let c = self.dev_pos(d.id);
            let len = 60.0 * PX_PER_M * self.zoom;
            let pts: Vec<Pos2> = std::iter::once(c)
                .chain((0..=12).map(|i| {
                    let a = (d.aim - d.beam / 2.0 + d.beam * i as f32 / 12.0).to_radians();
                    c + vec2(a.cos(), a.sin()) * len
                }))
                .collect();
            painter.add(Shape::convex_polygon(pts, pal().accent.gamma_multiply(0.06), Stroke::new(1.0, pal().accent.gamma_multiply(0.3))));
        }
        // Rubber band while connecting.
        if let (Some(from), Some(p)) = (self.connect_from, pointer) {
            painter.line_segment([self.dev_pos(from), p], Stroke::new(2.0, medium_color(self.medium)));
        }
        self.draw_labels(ui, &painter);
        if self.tool == Tool::Measure && self.mode == Mode::Design {
            self.measure_tool(&resp, &painter, pointer);
        }
        if let Some(lid) = self.route.filter(|_| self.mode == Mode::Design) {
            self.route_tool(lid, &resp, &painter, pointer);
        } else if self.tool == Tool::Wall && self.mode == Mode::Design {
            self.wall_tool(&resp, &painter, pointer);
            if pointer.is_some() && hovered_link.is_none() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }

        // Devices.
        let mut clicked_dev = None;
        let mut released_on = None;
        let ids: Vec<u32> = self.net.devices.iter().map(|d| d.id).collect();
        let size = 58.0 * z;
        for id in &ids {
            let c = self.dev_pos(*id);
            let r = Rect::from_center_size(c, Vec2::splat(size));
            let r_resp = ui.interact(r, Id::new(("dev", *id)), Sense::click_and_drag());
            if r_resp.clicked() {
                clicked_dev = Some(*id);
            }
            if r_resp.drag_started() && (self.tool == Tool::Connect && self.mode == Mode::Design) {
                self.connect_from = Some(*id);
            }
            if r_resp.drag_stopped() && self.snap && self.connect_from.is_none() && self.mode == Mode::Design {
                if let Some(d) = self.net.dev_mut(*id) {
                    d.pos = radio_ui::snap(vec2(d.pos[0], d.pos[1]));
                }
            }
            // Devices on other storeys are shown faded and stay put.
            let here = self.net.dev(*id).is_some_and(|d| d.floor == self.floor);
            if r_resp.drag_started() && here && self.connect_from.is_none() {
                if self.mode == Mode::Simulate {
                    self.remember_home(*id);
                } else {
                    self.snapshot();
                }
            }
            if r_resp.dragged() && here && self.connect_from.is_none() {
                let delta = r_resp.drag_delta() / self.zoom;
                if let Some(d) = self.net.dev_mut(*id) {
                    d.pos[0] += delta.x;
                    d.pos[1] += delta.y;
                }
                self.sel = Sel::Dev(*id);
                self.walks.remove(id);
                if self.mode == Mode::Simulate {
                    let ctx = ui.ctx().clone();
                    self.after_move(&ctx, *id);
                }
            }
            let r_resp = r_resp.on_hover_ui(|ui| self.device_tooltip(ui, *id, &stp));
            r_resp.context_menu(|ui| self.device_menu(ui, *id));
            if pointer.is_some_and(|p| r.contains(p)) {
                released_on = Some(*id);
            }
            if r_resp.hovered() {
                ui.ctx().set_cursor_icon(if self.tool == Tool::Connect { egui::CursorIcon::Crosshair } else { egui::CursorIcon::Grab });
            }
        }
        // Labels go where there's room: one that would cover another label or a device icon is left out
        // (hover or select the device to see it).
        let mut taken: Vec<Rect> = self.net.devices.iter().filter(|d| d.floor == self.floor).map(|d| Rect::from_center_size(self.dev_pos(d.id), Vec2::splat(size))).collect();
        let mut place = |painter: &egui::Painter, at: Pos2, text: String, font: FontId, col: Color32, force: bool| {
            let g = painter.layout_no_wrap(text, font, col);
            let r = Rect::from_center_size(at, g.size());
            if force || !taken.iter().any(|t| t.intersects(r.shrink(1.0))) {
                taken.push(r);
                painter.galley(r.min, g, col);
            }
        };
        for d in &self.net.devices {
            // Devices on other storeys stay visible but faded, like a see-through floor plan.
            let painter = {
                let mut p = painter.clone();
                if d.floor != self.floor {
                    p.multiply_opacity(0.3);
                }
                p
            };
            let c = self.dev_pos(d.id);
            let r = Rect::from_center_size(c, Vec2::splat(size));
            let col = kind_color(d.kind);
            if d.floor != self.floor {
                painter.text(r.center_top() + vec2(0.0, -2.0 * z), Align2::CENTER_BOTTOM, net::floor_name(d.floor), FontId::proportional(10.0 * z), pal().text);
            }
            let selected = self.sel == Sel::Dev(d.id);
            let hovered = pointer.is_some_and(|p| r.contains(p));
            if selected {
                painter.rect_filled(r.expand(6.0 * z), 18.0 * z, pal().accent.gamma_multiply(0.18));
            }
            painter.rect_filled(r, 14.0 * z, if d.kind == Kind::Neighbor { pal().card.gamma_multiply(0.6) } else { pal().card });
            let painter = {
                let mut p = painter.clone();
                if d.off {
                    p.multiply_opacity(0.35);
                }
                p
            };
            if d.off {
                painter.text(r.right_bottom() - vec2(4.0, 4.0) * z, Align2::RIGHT_BOTTOM, ic::POWER, FontId::proportional(13.0 * z.max(0.7)), pal().red);
            }
            painter.rect_stroke(r, 14.0 * z, Stroke::new(if selected { 2.0 } else { 1.2 }, if selected || hovered { col } else { col.gamma_multiply(0.45) }), StrokeKind::Inside);
            if self.walks.contains_key(&d.id) {
                painter.text(r.left_top() + vec2(2.0, 2.0) * z, Align2::LEFT_TOP, ic::PERSON_SIMPLE_WALK, FontId::proportional(14.0 * z), pal().accent);
            }
            painter.text(c, Align2::CENTER_CENTER, kind_icon(d.kind), FontId::proportional(28.0 * z), col);
            let status = match worst.get(&d.id) {
                Some(Level::Error) => pal().red,
                Some(Level::Warning) => pal().amber,
                _ => pal().green,
            };
            if d.kind != Kind::Neighbor {
                painter.circle_filled(r.right_top() + vec2(-6.0, 6.0) * z, 4.5 * z, status);
            }
            if stp.root == Some(d.id) {
                painter.text(r.center_top() + vec2(0.0, -2.0 * z), Align2::CENTER_BOTTOM, ic::CROWN, FontId::proportional(16.0 * z), pal().amber);
            }
            if let Some(v) = d.ifaces.first().map(|f| f.vlan).filter(|v| *v != 1 && d.kind.is_host()) {
                let hue = Color32::from(egui::ecolor::Hsva::new((v as f32 * 0.137).fract(), 0.6, 0.9, 1.0));
                let chip = Rect::from_min_size(r.left_bottom() + vec2(-6.0, -14.0) * z, vec2(30.0, 14.0) * z);
                painter.rect_filled(chip, 4.0 * z, hue);
                painter.text(chip.center(), Align2::CENTER_CENTER, format!("V{v}"), FontId::proportional(9.5 * z), pal().bg);
            }
            // Zoomed out, labels would pile on top of each other: show them for the device in focus only.
            let focus = selected || hovered;
            let names = self.zoom >= 0.45 || focus;
            let subs = self.zoom >= 0.65 || focus;
            if names {
                place(&painter, c + vec2(0.0, size * 0.5 + 9.0 * z.max(0.7)), d.name.clone(), FontId::proportional((12.5 * z).max(10.0)), pal().text, focus);
            }
            let ip = d.ifaces.get(if d.kind == Kind::HomeRouter { 0 } else { d.dhcp_iface() }).map(|f| f.ip.clone()).unwrap_or_default();
            let chans = |d: &net::Device| wifi::Band::ALL.iter().filter(|b| d.bands[b.i()]).map(|b| self.net.channel(d, *b).to_string()).collect::<Vec<_>>().join("/");
            let wifi_sig = self.net.wifi_link_of(d.id).and_then(|l| self.net.link_signal(l));
            let (sub, font, sub_col) = if d.kind.transmits() && (self.mode == Mode::Design || d.kind == Kind::Neighbor) {
                let what = if d.kind == Kind::Interference { format!("{} ch {}", ic::LIGHTNING, d.ch[0]) } else { format!("{} {} · ch {}", ic::WIFI_HIGH, d.ssid, chans(d)) };
                (what, FontId::proportional(11.0 * z), pal().dim)
            } else if let (Mode::Simulate, Some(s)) = (self.mode, wifi_sig.filter(|_| !d.kind.radio())) {
                // The speed it really gets (the whole way to the internet), not just its own Wi-Fi link.
                let mbps = self.e2e.get(&d.id).map_or(s.mbps, |e| e.0.min(s.mbps));
                (trf("{} {} dBm · {} Mbit/s", &[&(ic::WIFI_HIGH), &format!("{:.0}", s.rssi), &format!("{:.0}", mbps)]), FontId::proportional(11.0 * z), quality_color(s.rssi))
            } else {
                (ip, FontId::monospace(10.5 * z), pal().dim)
            };
            if !sub.is_empty() && subs {
                place(&painter, c + vec2(0.0, size * 0.5 + 24.0 * z.max(0.7)), sub, FontId::new(font.size.max(9.5), font.family), sub_col, focus);
            }
        }

        self.draw_points(ui, &painter);

        // Moving packets; in step-through mode the one packet waits at its device.
        if let Some((a, i)) = self.step.and_then(|k| step_at(&self.anims, k)) {
            let p = self.dev_pos(a.path[i]);
            let col = if a.ok || i + 1 < a.path.len() { pal().accent } else { pal().red };
            painter.circle_filled(p - vec2(0.0, size * 0.5 + 10.0 * z), 11.0 * z, col.gamma_multiply(0.3));
            painter.circle_filled(p - vec2(0.0, size * 0.5 + 10.0 * z), 6.0 * z, col);
        }
        if self.step.is_none() {
            self.anims.retain(|a| now < a.t0 + a.path.len() as f64 * 0.28 + 0.6);
        }
        for a in self.anims.iter().filter(|_| self.step.is_none()) {
            let t = ((now - a.t0) / 0.28).max(0.0);
            let i = t.floor() as usize;
            if now < a.t0 {
                continue;
            }
            if i + 1 < a.path.len() {
                let (p, q) = (self.dev_pos(a.path[i]), self.dev_pos(a.path[i + 1]));
                let pos = p + (q - p) * (t.fract() as f32);
                let col = if a.ok || i + 2 < a.path.len() { pal().accent } else { pal().red };
                painter.circle_filled(pos, 9.0 * z, col.gamma_multiply(0.25));
                painter.circle_filled(pos, 5.0 * z, col);
            } else if let Some(last) = a.path.last() {
                let p = self.dev_pos(*last);
                let (icon, col) = if a.ok { (ic::CHECK_CIRCLE, pal().green) } else { (ic::X, pal().red) };
                painter.text(p - vec2(0.0, size * 0.5 + 12.0 * z), Align2::CENTER_CENTER, icon, FontId::proportional(18.0 * z), col);
            }
        }
        if !self.anims.is_empty() && self.step.is_none() {
            ui.ctx().request_repaint();
        }

        // Finish a connection drag.
        if self.connect_from.is_some() && ui.input(|i| i.pointer.any_released()) {
            let from = self.connect_from.take().unwrap();
            if let Some(to) = released_on.filter(|t| *t != from) {
                self.try_connect(ui.ctx(), from, to);
            }
        }
        let drawing = (self.tool == Tool::Wall || self.route.is_some()) && self.mode == Mode::Design;
        if let Some(id) = clicked_dev.filter(|_| !drawing) {
            if self.tool == Tool::Connect && self.mode == Mode::Design {
                match self.sel {
                    Sel::Dev(prev) if prev != id => {
                        self.try_connect(ui.ctx(), prev, id);
                    }
                    _ => self.sel = Sel::Dev(id),
                }
            } else {
                self.sel = Sel::Dev(id);
            }
        } else if resp.clicked() && !drawing {
            self.sel = hovered_link.map(Sel::Link).or(hovered_wall.filter(|_| self.mode == Mode::Design).map(Sel::Wall)).unwrap_or(Sel::None);
        }
        // Right-click: a menu for the wall or cable under the pointer, or for the empty spot (the wall tool uses right-click to stop).
        if !drawing {
            if resp.secondary_clicked() {
                let at = pointer.map(|p| self.to_world(p)).unwrap_or_default();
                self.ctx_at = Some((hovered_wall.filter(|_| hovered_link.is_none()), hovered_link, [at.x, at.y]));
            }
            resp.context_menu(|ui| self.map_menu(ui));
        }
        if resp.dragged() && (!drawing || !resp.dragged_by(egui::PointerButton::Primary)) {
            self.pan += resp.drag_delta();
        }

        // Drop from the shelf.
        if let Some(k) = self.drag_kind {
            if let Some(p) = pointer {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                let lp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("ghost")));
                lp.rect_filled(Rect::from_center_size(p, Vec2::splat(52.0)), 12, pal().card.gamma_multiply(0.9));
                lp.text(p, Align2::CENTER_CENTER, kind_icon(k), FontId::proportional(26.0), kind_color(k));
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.drag_kind = None;
                if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
                    self.snapshot();
                    let w = self.to_world(p);
                    let id = self.net.add_device(k, [w.x, w.y]);
                    self.net.dev_mut(id).unwrap().floor = self.floor;
                    self.sel = Sel::Dev(id);
                }
            }
        }

        if self.heat {
            self.heat_legend(&painter, rect);
            // Which band the map shows.
            let lift = if rect.width() < 810.0 { 44.0 } else { 0.0 };
            let r = Rect::from_min_size(rect.left_bottom() + vec2(12.0, -74.0 - lift), vec2(250.0, 28.0));
            painter.rect_filled(r.expand(2.0), 8, pal().card.gamma_multiply(0.92));
            ui.put(r.shrink2(vec2(6.0, 0.0)), |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    for (i, t) in [tr("Best band"), "2.4", "5", "6 GHz"].iter().enumerate() {
                        ui.selectable_value(&mut self.heat_band, i, tr(t));
                    }
                })
                .response
            });
        }
        // Zoom controls.
        let zr = Rect::from_min_size(rect.right_bottom() - vec2(312.0, 44.0), vec2(300.0, 32.0));
        painter.rect_filled(zr.expand2(vec2(4.0, 0.0)), 8, pal().card.gamma_multiply(0.92));
        ui.put(zr, |ui: &mut egui::Ui| {
            ui.horizontal(|ui| {
                if ui.small_button(ic::MAGNIFYING_GLASS_MINUS).clicked() {
                    self.zoom = (self.zoom / 1.2).max(0.1);
                }
                ui.label(RichText::new(format!("{:.0}%", self.zoom * 100.0)).small().color(pal().dim)).on_hover_text(tr("Zoom level"));
                ui.label(RichText::new(trf("· dots {} m", &[&(self.grid_m())])).small().color(pal().dim)).on_hover_text(tr("Distance between two dots of the grid"));
                if ui.small_button(ic::MAGNIFYING_GLASS_PLUS).clicked() {
                    self.zoom = (self.zoom * 1.2).min(MAX_ZOOM);
                }
                if ui.small_button(ic::CORNERS_OUT).on_hover_text(tr("Fit everything")).clicked() {
                    self.fit();
                }
                if ui.small_button(ic::CAMERA).on_hover_text(tr("Save the map as a picture (PNG)")).clicked() {
                    let ctx = ui.ctx().clone();
                    self.export_picture(&ctx);
                }
            })
            .response
        });
        self.search_overlay(ui, rect);
        self.load_cache = load;
    }

    /// Zoom and pan so the devices, walls and floor plans of the shown storey fill the map.
    fn fit(&mut self) {
        let f = self.floor;
        let pts: Vec<Vec2> = self
            .net
            .devices
            .iter()
            .filter(|d| d.floor == f || self.net.devices.iter().all(|x| x.floor != f))
            .map(|d| vec2(d.pos[0], d.pos[1]))
            .chain(self.net.walls.iter().filter(|w| w.floor == f).flat_map(|w| [vec2(w.a[0], w.a[1]), vec2(w.b[0], w.b[1])]))
            .chain(self.net.plans.iter().filter(|p| p.floor == f).flat_map(|p| [vec2(p.pos[0], p.pos[1]), vec2(p.pos[0] + p.width_m * PX_PER_M, p.pos[1])]))
            .collect();
        if pts.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (vec2(f32::MAX, f32::MAX), vec2(f32::MIN, f32::MIN));
        for p in &pts {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        // Room for device icons and labels around the edge, in screen pixels.
        let size = (hi - lo).max(vec2(PX_PER_M, PX_PER_M));
        let avail = self.canvas_rect.size();
        let room = (avail - vec2(140.0, 140.0)).max(avail * 0.5);
        self.zoom = (room.x / size.x).min(room.y / size.y).clamp(0.1, MAX_ZOOM);
        self.pan = avail * 0.5 - (lo + (hi - lo) * 0.5) * self.zoom;
    }

    /// The latest spanning tree (refreshed with the problem list).
    fn stp(&self) -> crate::stp::Stp {
        self.stp_cache.clone().unwrap_or_else(|| self.net.stp())
    }

    fn undo_once(&mut self, ctx: &egui::Context) {
        match self.undo.pop() {
            Some((n, f)) => {
                self.redo.push((std::mem::replace(&mut self.net, n), std::mem::replace(&mut self.file, f)));
                self.sel = Sel::None;
                self.connect_from = None;
                self.ap_hint = None;
                self.heat_cache = None;
                self.heat_job = None;
                self.issues = None;
                self.say(ctx, tr("Undone."));
            }
            None => self.say(ctx, tr("Nothing to undo.")),
        }
    }

    fn redo_once(&mut self, ctx: &egui::Context) {
        match self.redo.pop() {
            Some((n, f)) => {
                self.undo.push((std::mem::replace(&mut self.net, n), std::mem::replace(&mut self.file, f)));
                self.sel = Sel::None;
                self.connect_from = None;
                self.ap_hint = None;
                self.heat_cache = None;
                self.heat_job = None;
                self.issues = None;
                self.say(ctx, tr("Redone."));
            }
            None => self.say(ctx, tr("Nothing to redo.")),
        }
    }

    /// Selects a device and brings it into view, switching to its storey.
    pub(crate) fn focus(&mut self, id: u32) {
        let Some(d) = self.net.dev(id) else { return };
        self.floor = d.floor;
        let c = self.canvas_rect.size() * 0.5;
        self.pan = c - vec2(d.pos[0], d.pos[1]) * self.zoom;
        self.sel = Sel::Dev(id);
    }

    /// Notes where a device stood (and its Wi-Fi link) before Simulate moves it.
    pub(crate) fn remember_home(&mut self, id: u32) {
        let Some(d) = self.net.dev(id) else { return };
        let link = self.net.wifi_link_of(id).cloned();
        self.sim_home.entry(id).or_insert((d.pos, link));
    }

    /// Puts devices back where they stood before Simulate moved them.
    pub(crate) fn restore_home(&mut self, only: Option<u32>) {
        let ids: Vec<u32> = self.sim_home.keys().copied().filter(|i| only.is_none_or(|o| o == *i)).collect();
        for id in ids {
            let (pos, link) = self.sim_home.remove(&id).unwrap();
            if let Some(d) = self.net.dev_mut(id) {
                d.pos = pos;
            }
            if let Some(l) = link {
                if let Some(x) = self.net.links.iter_mut().find(|x| x.id == l.id) {
                    *x = l;
                }
            }
        }
        self.issues = None;
    }

    fn try_connect(&mut self, ctx: &egui::Context, a: u32, b: u32) {
        let (Some(ka), Some(kb)) = (self.net.dev(a).map(|d| d.kind), self.net.dev(b).map(|d| d.kind)) else { return };
        // Phones and gadgets have no socket, and laptops usually join by Wi-Fi too — hold Shift to cable a laptop.
        let shift = ctx.input(|i| i.modifiers.shift);
        let wireless = |k: Kind| matches!(k, Kind::Phone | Kind::SmartHome) || (k == Kind::Laptop && !shift);
        let medium = if (ka.radio() || kb.radio()) && (wireless(ka) || wireless(kb)) { Medium::WiFi } else { self.medium };
        self.snapshot();
        match self.net.connect(a, b, medium) {
            Ok(id) => self.sel = Sel::Link(id),
            Err(e) => {
                self.undo.pop();
                self.say(ctx, e)
            }
        }
    }

    // ---------- right: inspector / terminal ----------

    fn inspector(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        match self.sel {
            Sel::Dev(id) if self.net.dev(id).is_some() => {
                if self.mode == Mode::Simulate {
                    self.device_sim(ui, id);
                } else {
                    self.device_editor(ui, id, issues);
                }
            }
            Sel::Link(id) if self.net.links.iter().any(|l| l.id == id) => self.link_editor(ui, id),
            Sel::Wall(id) if self.net.walls.iter().any(|w| w.id == id) => self.wall_editor(ui, id),
            Sel::Point(id) if self.net.points.iter().any(|p| p.id == id) => self.point_editor(ui, id),
            Sel::Label(id) if self.net.labels.iter().any(|l| l.id == id) => self.label_editor(ui, id),
            _ => self.overview(ui, issues),
        }
    }

    fn header(ui: &mut egui::Ui, icon: &str, col: Color32, title: &str, sub: &str) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
            ui.painter().rect_filled(r, 10, col.gamma_multiply(0.15));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, FontId::proportional(22.0), col);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(16.0).strong());
                ui.label(RichText::new(tr(sub)).small().color(pal().dim));
            });
        });
    }

    /// A fold-out section: a title row that opens and closes. Returns whether it's open.
    /// The open/closed choice is remembered per title, so it stays the same from device to device.
    fn section(ui: &mut egui::Ui, title: &str) -> bool {
        Self::section_hint(ui, title, "")
    }

    /// A section whose explanation sits behind an info icon instead of filling the panel.
    fn section_hint(ui: &mut egui::Ui, title: &str, hint: &str) -> bool {
        // The essentials start open; details start folded away.
        const OPEN: &[&str] = &["Measured here", "Real measurements", "Joined network", "Test load", "Addresses", "Internet plan", "Wi-Fi hardware", "Their Wi-Fi", "Speaker mesh radio", "What it is", "Bands", "Material", "Type", "Length", "Ports", "How it performs", "At a glance", "Wi-Fi coverage", "Connected devices", "Channels", "Overlaps with", "Networks in the air here", "Try it", "About this setup", "How to"];
        let id = Id::new(("section", title));
        let mut open = ui.ctx().data_mut(|d| *d.get_persisted_mut_or(id, OPEN.contains(&title)));
        ui.add(egui::Separator::default().spacing(6.0));
        let clicked = ui
            .horizontal(|ui| {
                let caret = if open { ic::CARET_DOWN } else { ic::CARET_RIGHT };
                let r = ui.add(egui::Label::new(RichText::new(format!("{caret}  {}", tr(title))).strong().color(if open { pal().text } else { pal().dim })).sense(Sense::click()));
                if !hint.is_empty() {
                    ui.label(RichText::new(ic::INFO).color(pal().dim)).on_hover_text(tr(hint));
                }
                // The rest of the row is clickable too.
                let rest = ui.allocate_response(vec2(ui.available_width(), r.rect.height()), Sense::click());
                r.clicked() || rest.clicked()
            })
            .inner;
        if clicked {
            open = !open;
            ui.ctx().data_mut(|d| d.insert_persisted(id, open));
        }
        if open {
            ui.add_space(2.0);
        }
        open
    }

    fn device_editor(&mut self, ui: &mut egui::Ui, id: u32, issues: &[Issue]) {
        let currency = self.net.currency.clone();
        let mut delete = false;
        let mut renew = false;
        let self_wired = self.net.links.iter().any(|l| (l.a == id || l.b == id) && !l.medium.wireless());
        {
            // Header: icon, editable name, kind.
            let d = self.net.dev_mut(id).unwrap();
            let host = (d.kind.is_host() || d.kind == Kind::HomeRouter).then(|| trf("On the network as {}.home.arpa", &[&(d.hostname())]));
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::hover());
                ui.painter().rect_filled(r, 10, kind_color(d.kind).gamma_multiply(0.15));
                ui.painter().text(r.center(), Align2::CENTER_CENTER, kind_icon(d.kind), FontId::proportional(22.0), kind_color(d.kind));
                resp.on_hover_text(d.kind.blurb());
                ui.vertical(|ui| {
                    let name = ui.add(TextEdit::singleline(&mut d.name).font(FontId::proportional(16.0)).desired_width(f32::INFINITY));
                    if let Some(h) = &host {
                        name.on_hover_text(h);
                    }
                    ui.label(RichText::new(d.kind.label()).small().color(pal().dim));
                });
            });
        }
        // Actions, then this device's problems: the things you came for, before the details.
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(format!("{}  {}", ic::PLAY, tr("Try it"))).on_hover_text(tr("See this device in Simulate mode")).clicked() {
                self.mode = Mode::Simulate;
                let ctx = ui.ctx().clone();
                self.start_sim(&ctx);
            }
            if ui.small_button(format!("{}  {}", ic::COPY, tr("Duplicate"))).on_hover_text(tr("A copy next to it, same hardware, fresh addresses (Ctrl+D)")).clicked() {
                self.snapshot();
                if let Some(new) = self.net.duplicate(id) {
                    self.sel = Sel::Dev(new);
                }
            }
            self.power_toggle(ui, id);
            if ui.small_button(RichText::new(format!("{}  {}", ic::TRASH, tr("Delete"))).color(pal().red)).on_hover_text(tr("Remove this device (Delete key)")).clicked() {
                delete = true;
            }
        });
        let mine: Vec<&Issue> = issues.iter().filter(|i| i.dev == Some(id)).collect();
        if !mine.is_empty() {
            ui.add_space(4.0);
            for i in mine {
                if let (_, Some(f)) = issue_row(ui, i) {
                    self.snapshot();
                    let msg = self.net.apply_fix(&f);
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, msg);
                }
            }
        }
        let plugged = self.net.port_users(id);
        {
            // On the mobile line the router's own address comes from the provider's shared range.
            let mobile = self.net.dev(id).and_then(|d| self.net.cgnat(d));
            let d = self.net.dev_mut(id).unwrap();
            let kind = d.kind;
            // Thread and Zigbee gadgets get no address from DHCP (their hub speaks for them; see Smart-home radio).
            if !d.ifaces.is_empty() && !kind.bridges() && !d.on_802154() && Self::section(ui, "Addresses") {
                if kind.is_host() || kind == Kind::HomeRouter {
                    let label = tr(if kind == Kind::HomeRouter { tr("Get internet address automatically (DHCP)") } else { tr("Get address automatically (DHCP)") });
                    if ui.checkbox(&mut d.dhcp_client, label).changed() && d.dhcp_client {
                        renew = true;
                    }
                }
                let dhcp_if = d.dhcp_iface();
                let auto = d.dhcp_client;
                egui::Grid::new("ifaces").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    for (i, f) in d.ifaces.iter_mut().enumerate().filter(|(_, f)| f.parent.is_none()) {
                        ui.label(RichText::new(&f.name).monospace().color(pal().dim));
                        ui.horizontal(|ui| {
                            let locked = auto && i == dhcp_if;
                            ui.add_enabled(!locked, TextEdit::singleline(&mut f.ip).desired_width(118.0).hint_text(tr("e.g. 192.168.1.20")).font(egui::TextStyle::Monospace));
                            ui.label(tr("/"));
                            ui.add_enabled(!locked, egui::DragValue::new(&mut f.prefix).range(0..=32));
                        });
                        ui.end_row();
                        if let (Some(cg), true) = (&mobile, kind == Kind::HomeRouter && i == 1) {
                            ui.label("");
                            ui.label(RichText::new(trf("Mobile line: the router itself has {} (carrier-grade NAT); {} is the provider's address shared by many customers.", &[cg, &f.ip])).small().color(pal().amber));
                            ui.end_row();
                        }
                    }
                    // A VLAN number is set on wired ports; Wi-Fi devices get theirs from the network they join.
                    if kind.is_host() && self_wired {
                        ui.label(tr("VLAN"));
                        ui.add(egui::DragValue::new(&mut d.ifaces[0].vlan).range(1..=4094)).on_hover_text(tr("1 = the normal, untagged network. Other numbers need a router interface in that VLAN."));
                        ui.end_row();
                    }
                    if kind != Kind::Internet {
                        ui.label(tr("Gateway"));
                        ui.add_enabled(!auto, TextEdit::singleline(&mut d.gateway).desired_width(150.0).hint_text(tr("router address")).font(egui::TextStyle::Monospace));
                        ui.end_row();
                        ui.label(tr("DNS"));
                        ui.add_enabled(!auto, TextEdit::singleline(&mut d.dns).desired_width(150.0).hint_text(tr("name server")).font(egui::TextStyle::Monospace));
                        ui.end_row();
                    }
                });
            }
            if kind == Kind::Router && d.ifaces.is_empty() && Self::section(ui, "Addresses") {
                ui.label(RichText::new(tr("Connect cables first: each cable uses one of the router's ports, which then gets its own address.")).small().color(pal().dim));
            }
            if kind == Kind::Internet && Self::section(ui, "Internet plan") {
                ui_extras::plan_picker(ui, d, &mut self.catalog_query);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("plan").selected_text(format!("{:.0} Mbit/s", d.plan_mbps)).width(150.0).show_ui(ui, |ui| {
                        // Typical down/up speeds: DSL and cable upload far slower, fibre is symmetric.
                        for (n, v, up) in [("DSL 50", 50.0, 10.0), (tr("DSL vectoring 250"), 250.0, 40.0), ("Cable 500", 500.0, 50.0), (tr("Fibre 1 Gbit/s"), 1000.0, 1000.0), (tr("Fibre 10 Gbit/s"), 10000.0, 10000.0), (tr("5G home internet"), 300.0, 50.0), (tr("Satellite (low orbit)"), 150.0, 20.0)] {
                            if ui.selectable_label(d.plan_mbps == v && d.plan_up_mbps == up, format!("{} — {v:.0} / {up:.0} Mbit/s", tr(n))).clicked() {
                                (d.plan_mbps, d.plan_up_mbps) = (v, up);
                            }
                        }
                    });
                    ui.add(egui::DragValue::new(&mut d.plan_mbps).range(1.0..=25000.0).prefix(format!("{} ", ic::ARROW_DOWN)).suffix(tr(" Mbit/s")));
                    let mut up = if d.plan_up_mbps > 0.0 { d.plan_up_mbps } else { d.plan_mbps };
                    if ui.add(egui::DragValue::new(&mut up).range(1.0..=25000.0).prefix(format!("{} ", ic::ARROW_UP)).suffix(tr(" Mbit/s"))).on_hover_text(tr("Upload speed: backups, video calls and cameras send this way")).changed() {
                        d.plan_up_mbps = up;
                    }
                });
            }
            if kind.serves() && Self::section(ui, "Services") {
                if kind.offers("dhcp") {
                    ui.checkbox(&mut d.services.dhcp, tr("DHCP server")).on_hover_text(tr("Hands out addresses"));
                }
                if d.services.dhcp && kind.offers("dhcp") {
                    ui.horizontal(|ui| {
                        let pool = ui.label(tr("Pool"));
                        if kind == Kind::Router {
                            pool.on_hover_text(tr("Leave the pool empty: every VLAN interface then hands out .100–.199 of its own network."));
                        }
                        ui.add(TextEdit::singleline(&mut d.dhcp_from).desired_width(110.0).font(egui::TextStyle::Monospace));
                        ui.label(tr("–"));
                        ui.add(TextEdit::singleline(&mut d.dhcp_to).desired_width(110.0).font(egui::TextStyle::Monospace));
                    });
                }
                if kind.offers("dns") {
                    ui.checkbox(&mut d.services.dns, tr("DNS server")).on_hover_text(tr("Answers name lookups"));
                }
                if d.services.dns && kind.offers("dns") && kind != Kind::Internet {
                    ui.add(TextEdit::multiline(&mut d.dns_records).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(tr("www.home.test 192.168.1.10"))).on_hover_text(tr("Name records, one per line: name address. A mail server for a domain: school.test MX mail.school.test"));
                }
                if kind.offers("web") {
                    ui.checkbox(&mut d.services.web, tr("Web server (HTTPS)"));
                    if d.services.web {
                        ui.add(TextEdit::multiline(&mut d.web_page).desired_rows(3).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(tr("Page text or HTML (index.html)")));
                    }
                }
                if kind.offers("files") {
                    ui.checkbox(&mut d.services.files, tr("File sharing (SMB 3)"));
                }
                if kind.offers("print") {
                    ui.checkbox(&mut d.services.print, tr("Printing (IPP)"));
                }
                if kind.offers("mail") {
                    ui.checkbox(&mut d.services.mail, tr("Mail server (SMTP + IMAP)")).on_hover_text(tr("Takes in email for the domain whose name record points here; read it with the inbox command"));
                }
            }
            if kind.routes() && kind != Kind::Internet && Self::section(ui, "Routes") {
                let was = d.services.rip;
                ui.checkbox(&mut d.services.rip, tr("RIP: share routes with neighbour routers")).on_hover_text(tr("Routers with RIP on tell each other which networks they reach and fill their routing tables themselves, up to 15 routers away"));
                if d.services.rip && !was {
                    // Its routes spread one update (30 s) at a time from now on.
                    d.services.rip_since = Some(Self::now(ui.ctx()));
                }
                ui.add(TextEdit::multiline(&mut d.routes).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(tr("10.0.2.0/24 via 10.0.1.2"))).on_hover_text(tr("Extra routes, one per line: network/prefix via gateway"));
            }
            // The internet cloud is the outside world: no ports or storey to plan.
            if kind != Kind::Internet && Self::section(ui, "Hardware") {
            // Plain wired routers come from the catalog here (Wi-Fi gear does in "Wi-Fi hardware").
            if kind == Kind::Router {
                if let Some(old) = radio_ui::catalog_picker(ui, d, &mut self.catalog_query) {
                    self.before = Some(Before::Device(old));
                    self.replug = Some(id);
                }
            }
            egui::Grid::new(("hw", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                // Phones and gadgets have no socket; hosts have one, so just its speed.
                if kind.is_host() && !matches!(kind, Kind::Phone | Kind::SmartHome) {
                    ui.label(tr("Ethernet"));
                    if d.ports.is_empty() {
                        d.ports.push(ports::PortGroup::rj45(1, 1000));
                    }
                    d.ports.truncate(1);
                    ui_extras::speed_combo(ui, ("pspeed", id), &mut d.ports[0].mbps);
                    ui.end_row();
                }
                ui.label(tr("Storey"));
                ui.add(egui::DragValue::new(&mut d.floor).range(-3..=20).custom_formatter(|v, _| net::floor_name(v as i8)));
                ui.end_row();
                if kind.transmits() || kind.wifi_client() {
                    ui.label(tr("Mounted at"));
                    ui.add(egui::DragValue::new(&mut d.mount).range(0.0..=10.0).speed(0.05).max_decimals(2).suffix(tr(" m above the floor"))).on_hover_text(tr("Antenna height: desks and hands about 1 m, a ceiling access point 2.5 m"));
                    ui.end_row();
                }
                if !kind.foreign() {
                    ui.label(tr("Power draw"));
                    ui.add(egui::DragValue::new(&mut d.watts).range(0.0..=2000.0).speed(0.5).suffix(tr(" W average"))).on_hover_text(tr("Average over a day, standby included. Devices powered over the cable count at their switch."));
                    ui.end_row();
                    ui.label(tr("UPS battery"));
                    ui.add(egui::DragValue::new(&mut d.ups_wh).range(0.0..=5000.0).speed(1.0).suffix(" Wh")).on_hover_text(tr("Plugged into an uninterruptible power supply with this much battery (0 = none). It also carries what this device powers over its cables."));
                    ui.end_row();
                }
            });
            if !kind.foreign() && !kind.is_host() {
                ui.add_space(4.0);
                ui.label(RichText::new(tr("Ports")).color(pal().dim));
                let old = d.clone();
                if ui_extras::port_editor(ui, id, &mut d.ports, &plugged) {
                    self.before = Some(Before::Device(old));
                }
            }
            }
            if Self::section(ui, "Price and notes") {
                if !kind.foreign() && kind != Kind::Internet {
                    ui.horizontal(|ui| {
                        ui.label(tr("Price"));
                        ui.add(egui::DragValue::new(&mut d.price).range(0.0..=100000.0).prefix(currency.clone()).speed(5.0));
                    });
                    let mut owned = d.owned();
                    if ui.checkbox(&mut owned, tr("I already have this")).on_hover_text(tr("Left off the shopping list")).changed() {
                        d.owned = Some(owned);
                    }
                }
                ui.add(TextEdit::multiline(&mut d.notes).desired_rows(2).desired_width(f32::INFINITY).hint_text(tr("Notes: room, model, serial number…")));
            }
        }
        let kind = self.net.dev(id).unwrap().kind;
        if kind.transmits() {
            self.radio_editor(ui, id);
            if kind.radio() {
                self.names_editor(ui, id);
            }
        } else if kind.wifi_client() {
            self.adapter_editor(ui, id);
        }
        if kind.bridges() || kind == Kind::HomeRouter {
            let stp = self.stp();
            self.stp_editor(ui, id, stp.root, stp.cost.get(&id).copied());
        }
        if kind == Kind::Router {
            self.vlan_editor(ui, id);
        }
        self.device_extras(ui, id);
        if renew {
            let mut log = vec![];
            self.net.dhcp_one(id, &mut log);
            self.push_logs(log);
        }
        if delete {
            self.snapshot();
            self.net.remove_device(id);
            self.sel = Sel::None;
        }
    }

    fn link_editor(&mut self, ui: &mut egui::Ui, id: u32) {
        let (a, b) = {
            let l = self.net.links.iter().find(|l| l.id == id).unwrap();
            (self.net.name(l.a), self.net.name(l.b))
        };
        let measured = {
            let l = self.net.links.iter().find(|l| l.id == id).unwrap().clone();
            let mut l2 = l.clone();
            l2.length_m = None;
            self.net.link_len(&l2)
        };
        let kinds = {
            let l = self.net.links.iter().find(|l| l.id == id).unwrap();
            (self.net.dev(l.a).unwrap().kind, self.net.dev(l.b).unwrap().kind)
        };
        // Other cables between the same two switches: they can be bundled (LACP).
        let (parallel, next_lag) = {
            let l = self.net.links.iter().find(|l| l.id == id).unwrap();
            let p: Vec<u32> = self.net.links.iter().filter(|o| o.id != id && o.medium.cable() && ((o.a, o.b) == (l.a, l.b) || (o.a, o.b) == (l.b, l.a))).map(|o| o.id).collect();
            (p, self.net.links.iter().map(|l| l.lag).max().unwrap_or(0).saturating_add(1))
        };
        let both_switches = kinds.0 == Kind::Switch && kinds.1 == Kind::Switch;
        let mut route = false;
        let mut bundle = None;
        let design = self.mode == Mode::Design;
        let l = self.net.links.iter_mut().find(|l| l.id == id).unwrap();
        let old = l.clone();
        Self::header(ui, if l.medium == Medium::WiFi { ic::WIFI_HIGH } else { ic::LINK }, medium_color(l.medium), tr("Connection"), &format!("{a}  {}  {b}", ic::ARROWS_LEFT_RIGHT));
        // Only link types these two devices can actually use.
        let (ka, kb) = (kinds.0, kinds.1);
        let radio_pair = (ka.radio() && (kb.wifi_client() || kb.radio())) || (kb.radio() && (ka.wifi_client() || ka.radio()));
        let wifi_only = [ka, kb].iter().any(|k| matches!(k, Kind::Phone | Kind::SmartHome));
        ui.add_space(4.0);
        egui::Grid::new(("link", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
            ui.label(RichText::new(tr("Type")).color(pal().dim));
            egui::ComboBox::from_id_salt(("medium", id)).selected_text(l.medium.label()).width(200.0).show_ui(ui, |ui| {
                for m in Medium::ALL.into_iter().filter(|m| match m {
                    Medium::Mesh => ka == Kind::Speaker && kb == Kind::Speaker,
                    Medium::WiFi => radio_pair,
                    Medium::Cellular => [ka, kb].contains(&Kind::Internet) && (ka.routes() && kb.routes()),
                    _ => !wifi_only,
                }) {
                    ui.selectable_value(&mut l.medium, m, format!("{} — {}", m.label(), m.speed()));
                }
            });
            if l.medium != old.medium {
                self.before = Some(Before::Link(old.clone()));
            }
            ui.end_row();
            if l.medium == Medium::Powerline && !catalog::powerline().is_empty() {
                ui.label(RichText::new(tr("Kit")).color(pal().dim));
                let shown = if l.kit.is_empty() { tr("A typical kit (≈ 300 Mbit/s up close)").to_string() } else { l.kit.clone() };
                egui::ComboBox::from_id_salt(("kit", id)).selected_text(shown).width(200.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut l.kit, String::new(), tr("A typical kit (≈ 300 Mbit/s up close)"));
                    for k in catalog::powerline() {
                        ui.selectable_value(&mut l.kit, k.title(), trf("{} — {}, ≈ {} Mbit/s up close", &[&(k.title()), &k.standard, &format!("{:.0}", k.real_mbps)])).on_hover_text(trf("{}\n{}\nSpecs from {}", &[&(k.summary()), &k.note, &k.source]));
                    }
                });
                if l.kit != old.kit {
                    self.before = Some(Before::Link(old.clone()));
                }
                ui.end_row();
            }
            ui.label(RichText::new(tr("Length")).color(pal().dim));
            ui.horizontal(|ui| {
                let mut manual = l.length_m.is_some();
                if let Some(m) = &mut l.length_m {
                    ui.add(egui::DragValue::new(m).range(0.5..=2000.0).suffix(" m").speed(0.5));
                } else {
                    ui.label(format!("{measured} m"));
                }
                if ui.checkbox(&mut manual, tr("by hand")).on_hover_text(tr("Off: measured from the drawing")).changed() {
                    l.length_m = manual.then_some(measured);
                }
            });
            ui.end_row();
            if l.medium.cable() && design {
                ui.label(RichText::new(tr("Route")).color(pal().dim));
                ui.horizontal(|ui| {
                    if ui.small_button(format!("{}  {}", ic::PATH, tr("Draw its route"))).on_hover_text(tr("Click points along the walls where the cable runs; its length then follows the route")).clicked() {
                        route = true;
                    }
                    if !l.path.is_empty() && ui.small_button(tr("Straight")).on_hover_text(tr("Forget the drawn route")).clicked() {
                        self.before = Some(Before::Link(old.clone()));
                        l.path.clear();
                    }
                });
                ui.end_row();
            }
            if l.medium.cable() && both_switches && !parallel.is_empty() {
                ui.label(RichText::new(tr("Bundle")).color(pal().dim));
                let mut on = l.lag != 0;
                if ui.checkbox(&mut on, tr("Link aggregation (LACP)")).on_hover_text(tr("All cables between these two switches act as one faster link, and spanning tree doesn't see a loop. Both switches must support LACP.")).changed() {
                    bundle = Some(on);
                }
                ui.end_row();
            }
        });
        if route {
            self.snapshot();
            self.net.links.iter_mut().find(|l| l.id == id).unwrap().path.clear();
            self.route = Some(id);
        }
        if let Some(on) = bundle {
            self.snapshot();
            let lag = if on { next_lag } else { 0 };
            for l in self.net.links.iter_mut().filter(|l| l.id == id || (on && parallel.contains(&l.id))) {
                l.lag = lag;
            }
        }
        let l = self.net.links.iter().find(|l| l.id == id).unwrap().clone();
        let mesh_hint = if l.medium == Medium::Mesh { tr("Speaker mesh: slow in reality, but announced to spanning tree with a low cost — that's how speakers end up carrying the network.") } else { "" };
        if Self::section_hint(ui, "How it performs", mesh_hint) {
            if let Some(s) = self.net.link_signal(&l) {
                ui.label(RichText::new(trf("{} dBm ({}) on {} {} MHz · {}×{} streams", &[&format!("{:.0}", s.rssi), &(s.quality()), &(s.band.label()), &s.width, &s.streams, &s.streams])).color(quality_color(s.rssi)));
                if let Some(b) = s.also {
                    ui.label(RichText::new(trf("Multi-Link: {} carries data at the same time", &[&b.label()])).color(pal().green));
                }
                ui.label(RichText::new(trf("Answer back {} dBm · {} wall(s) · airtime ×{}", &[&format!("{:.0}", s.up), &s.walls, &format!("{:.1}", s.contention)])).color(pal().dim));
            }
            ui.label(trf("≈ {} Mbit/s real throughput", &[&format!("{:.0}", self.net.link_rate(&l))]));
            let stp = self.stp();
            if stp.blocked.contains(&id) {
                ui.label(RichText::new(tr("Spanning tree keeps this link on standby (backup path).")).color(pal().amber));
            } else if stp.storm.contains(&id) {
                ui.label(RichText::new(tr("This link closes a loop nobody blocks: broadcast storm!")).color(pal().red));
            }
        }
        self.link_ports(ui, id);
        self.link_extras(ui, id);
        ui.add_space(6.0);
        if ui.small_button(RichText::new(format!("{}  {}", ic::TRASH, tr("Remove connection"))).color(pal().red)).clicked() {
            self.snapshot();
            self.net.remove_link(id);
            self.sel = Sel::None;
        }
    }

    fn overview(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        Self::header(ui, ic::HOUSE_LINE, pal().accent, &self.net.title.clone(), tr("Plan overview"));
        let stp = self.stp();
        let n = &self.net;
        let errors = issues.iter().filter(|i| i.level == Level::Error).count();
        let warns = issues.iter().filter(|i| i.level == Level::Warning).count();
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            stat(ui, &n.devices.len().to_string(), "devices", pal().accent);
            stat(ui, &n.links.len().to_string(), "connections", Color32::from_rgb(96, 165, 250));
            stat(ui, &errors.to_string(), "errors", if errors > 0 { pal().red } else { pal().green });
            stat(ui, &warns.to_string(), "warnings", if warns > 0 { pal().amber } else { pal().green });
        });
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
        let cable: f32 = n.links.iter().filter(|l| !l.medium.wireless() && l.medium != Medium::Powerline).map(|l| n.link_len(l)).sum();
        let wifi = n.devices.iter().filter(|d| !d.kind.radio() && n.wifi_link_of(d.id).is_some()).count();
        let cur = n.currency.clone();
        let root = stp.root.map(|r| n.name(r)).unwrap_or_else(|| "—".into());
        let vl: std::collections::BTreeSet<u16> = n.devices.iter().flat_map(|d| d.ifaces.iter().map(|f| f.vlan)).filter(|v| *v != 1).collect();
        let vlans = if vl.is_empty() { tr("none").to_string() } else { vl.len().to_string() };
        if Self::section(ui, "At a glance") {
            egui::Grid::new("glance").num_columns(2).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                for (k, v) in [(tr("Estimated cost"), format!("{cur}{total:.0}")), (tr("Cable to lay"), format!("{cable:.0} m")), ("Wi-Fi devices", wifi.to_string()), (tr("Spanning-tree root"), root), (tr("VLANs in use"), vlans)] {
                    ui.label(RichText::new(tr(k)).color(pal().dim));
                    ui.label(v);
                    ui.end_row();
                }
            });
        }
        self.resilience_and_power(ui);
        if Self::section(ui, "Settings") {
            egui::Grid::new("settings").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
                ui.label(RichText::new(tr("Floors made of")).color(pal().dim)).on_hover_text(tr("What the floors between storeys are built from: it decides how much Wi-Fi gets through to the next storey"));
                egui::ComboBox::from_id_salt("slab").selected_text(self.net.slab.label()).width(170.0).show_ui(ui, |ui| {
                    for k in WallKind::ALL {
                        ui.selectable_value(&mut self.net.slab, k, k.label());
                    }
                });
                ui.end_row();
                ui.label(RichText::new(tr("Hardware")).color(pal().dim)).on_hover_text(tr("Real devices differ: MAC addresses, how sticky each phone is, how busy the neighbours are, how good the house wiring is. A fixed seed makes the same plan always behave the same."));
                if ui.small_button(trf("{} Shuffle  ·  {}", &[&(ic::SHUFFLE), &format!("{:08x}", self.net.seed as u32)])).on_hover_text(tr("Same plan, other physical devices: new MAC addresses, different neighbours' habits and wiring quality")).clicked() {
                    self.snapshot();
                    self.net.shuffle_hardware(sim::new_seed());
                }
                ui.end_row();
                ui.label(RichText::new(tr("Live variation")).color(pal().dim));
                ui.checkbox(&mut self.live_var, tr("in Simulate")).on_hover_text(tr("Fading and changing traffic while simulating. Off: Simulate uses the same steady averages as planning."));
                ui.end_row();
                ui.label(RichText::new(tr("Currency")).color(pal().dim));
                ui.add(TextEdit::singleline(&mut self.net.currency).desired_width(40.0));
                ui.end_row();
            });
        }
        self.plan_extras(ui);
        if Self::section(ui, "How to") {
            for (icon, t) in [
                (ic::CURSOR, tr("Drag devices from the left onto the map, or click one to add it.")),
                (ic::PLUGS_CONNECTED, tr("Pick Connect, then drag from one device to another.")),
                (ic::WALL, tr("Walls: click corner after corner; right-click or Esc ends the wall.")),
                (ic::PLAY, tr("Simulate: click a device to use its browser, command line or Wi-Fi.")),
                (ic::MAP_PIN, tr("Measure the real air with the NetSketch Survey phone app: Real measurements → Link a phone, and every spot you measure lands here as a pin.")),
                (ic::KEYBOARD, tr("Keyboard shortcuts: the keyboard button at the top right lists them all.")),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(icon).color(pal().accent));
                    ui.label(RichText::new(tr(t)).color(pal().dim));
                });
            }
        }
    }

    /// A device in Simulate mode: its little desktop of apps.
    fn device_sim(&mut self, ui: &mut egui::Ui, id: u32) {
        let d = self.net.dev(id).unwrap();
        let sub = match d.ifaces.get(d.dhcp_iface()).map(|f| f.ip.as_str()).filter(|s| !s.is_empty()) {
            Some(ip) => ip.to_string(),
            // Switches, access points, speakers and disturbers have no address of their own here.
            None if !d.kind.is_host() && !d.kind.routes() => if d.model.is_empty() { d.kind.label().to_string() } else { d.model.clone() },
            None => "no address yet".into(),
        };
        Self::header(ui, kind_icon(d.kind), kind_color(d.kind), &d.name, &sub);
        self.power_toggle(ui, id);
        self.apps_picker(ui, id);
        let d = self.net.dev(id).unwrap();
        ui.add_space(4.0);
        // Only the apps this kind of device really has.
        let k = d.kind;
        let wifi = k.transmits() || self.net.wifi_link_of(id).is_some();
        let tabs: Vec<(SimTab, &str, &str)> = [
            (SimTab::WiFi, ic::WIFI_HIGH, "Wi-Fi", wifi),
            (SimTab::Ports, ic::PLUGS_CONNECTED, tr("Ports"), k.has_ports_view()),
            (SimTab::Browser, ic::GLOBE, tr("Browser"), k.has_browser()),
            (SimTab::Terminal, ic::TERMINAL_WINDOW, tr("Command line"), k.has_shell()),
            (SimTab::Traffic, ic::LIST_BULLETS, tr("Traffic"), !k.foreign()),
        ]
        .into_iter()
        .filter(|t| t.3)
        .map(|t| (t.0, t.1, t.2))
        .collect();
        if !tabs.iter().any(|t| t.0 == self.sim_tab) {
            self.sim_tab = tabs[0].0;
        }
        ui.horizontal(|ui| {
            for (t, icon, label) in &tabs {
                ui.selectable_value(&mut self.sim_tab, *t, format!("{icon} {}", tr(label)));
            }
        });
        ui.separator();
        match self.sim_tab {
            SimTab::WiFi => self.wifi_tab(ui, id),
            SimTab::Ports => self.ports_tab(ui, id),
            SimTab::Browser => self.browser(ui, id),
            SimTab::Terminal => self.terminal(ui, id),
            SimTab::Traffic => {
                let d = self.net.dev(id).unwrap();
                let ips: Vec<String> = d.ifaces.iter().filter(|f| !f.ip.is_empty()).map(|f| f.ip.clone()).collect();
                let me = |a: &str| a == d.name || ips.iter().any(|ip| a == ip || a.strip_prefix(ip.as_str()).is_some_and(|r| r.starts_with(':')));
                let rows: Vec<&Row> = self.rows.iter().filter(|r| r.log.dev == id || me(&r.log.src) || me(&r.log.dst)).collect();
                ui.label(RichText::new(trf("Everything {} sent, received or handled — like a packet capture.", &[&d.name])).small().color(pal().dim));
                egui::ScrollArea::vertical().id_salt("traffic").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                    if rows.is_empty() {
                        ui.label(RichText::new(tr("Nothing yet. Open a page or ping from any device.")).color(pal().dim));
                    }
                    for r in rows {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{:.2}s", r.t)).monospace().color(pal().dim));
                            ui.label(RichText::new(r.log.proto).strong().color(proto_color(r.log.proto)));
                            ui.label(RichText::new(format!("{} {} {}", r.log.src, ic::ARROW_RIGHT, r.log.dst)).monospace());
                        });
                        ui.label(RichText::new(&r.log.info).small().color(pal().dim));
                        ui.add_space(2.0);
                    }
                });
            }
        }
    }

    fn browser(&mut self, ui: &mut egui::Ui, id: u32) {
        let home = self.net.devices.iter().find(|x| x.id != id && x.services.web && x.first_ip().is_some()).and_then(|x| x.first_ip()).map(|ip| ip.to_string()).unwrap_or_else(|| "example.com".into());
        let b = self.browsers.entry(id).or_insert_with(|| Browser { url: home, ..Default::default() });
        let mut go = None;
        ui.horizontal(|ui| {
            if ui.small_button(ic::ARROW_CLOCKWISE).on_hover_text(tr("Reload")).clicked() {
                go = Some(b.url.clone());
            }
            let r = ui.add(TextEdit::singleline(&mut b.url).font(egui::TextStyle::Monospace).desired_width(ui.available_width() - 44.0).hint_text(tr("address or name")));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                go = Some(b.url.clone());
            }
            if ui.button(ic::ARROW_RIGHT).on_hover_text(tr("Go")).clicked() {
                go = Some(b.url.clone());
            }
        });
        let mut follow = None;
        if let Some(err) = &b.error {
            ui.label(RichText::new(format!("{}  {err}", ic::WARNING)).color(pal().red));
        } else if let Some(f) = &b.pending {
            ui.horizontal(|ui| {
                ui.spinner();
                let what = if f.fixed.is_some() { trf("Testing this network's speed on {} …", &[&f.shown]) } else { format!("Loading {} …", f.shown) };
                ui.label(RichText::new(what).color(pal().dim));
            });
        } else if let Some((addr, _)) = &b.page {
            let page = &b.parsed;
            let bg = page.bg.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(Color32::WHITE);
            let fg = if bg.r() as u32 + bg.g() as u32 + bg.b() as u32 > 380 { Color32::from_rgb(20, 24, 32) } else { pal().text };
            if !page.title.is_empty() {
                ui.label(RichText::new(&page.title).strong());
            }
            egui::Frame::new().fill(bg).corner_radius(8).inner_margin(Margin::same(14)).show(ui, |ui| {
                ui.set_min_size(vec2(ui.available_width(), 220.0));
                let body = |ui: &mut egui::Ui| {
                    for blk in &page.blocks {
                        match blk {
                            html::Block::Heading(l, t) => {
                                ui.label(RichText::new(t).size(26.0 - *l as f32 * 3.0).strong().color(fg));
                            }
                            html::Block::Text(spans) => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    for sp in spans {
                                        let t = RichText::new(&sp.text).color(fg);
                                        let t = if sp.bold { t.strong() } else { t };
                                        match &sp.href {
                                            Some(h) => {
                                                if ui.link(t.color(Color32::from_rgb(29, 78, 216)).underline()).on_hover_text(h).clicked() {
                                                    follow = Some(h.clone());
                                                }
                                            }
                                            None => {
                                                ui.label(t);
                                            }
                                        }
                                    }
                                });
                            }
                            html::Block::Rule => {
                                ui.separator();
                            }
                            html::Block::Image(alt) => {
                                ui.label(RichText::new(format!("{}  {alt}", ic::IMAGE)).color(fg.gamma_multiply(0.6)));
                            }
                        }
                    }
                };
                if page.center { ui.vertical_centered(body) } else { ui.vertical(body) };
            });
            ui.label(RichText::new(format!("{}  {addr}", ic::LOCK)).small().color(pal().dim));
            if let Some(n) = &b.note {
                ui.label(RichText::new(n).small().color(pal().dim));
            }
        } else {
            ui.label(RichText::new(tr("Type an address — a device's IP, its name, or a real website like wikipedia.org — and press Enter.")).color(pal().dim));
        }
        if let Some(h) = follow.filter(|h| !h.starts_with('#') && !h.starts_with("mailto:") && !h.starts_with("javascript:")) {
            let here = b.page.as_ref().map(|p| p.0.clone()).unwrap_or_default();
            go = Some(html::join_url(&here, &h));
        }
        // A real page arriving: show it when the simulated network would have delivered it too.
        let now = ui.input(|i| i.time);
        let mut ready = None;
        if let Some(f) = b.pending.as_mut() {
            if f.got.is_none() {
                if let Ok(r) = f.rx.try_recv() {
                    f.got = Some((r, now));
                }
            }
            match &f.got {
                Some((r, _)) => {
                    let bytes = r.as_ref().map(|h| h.len()).unwrap_or(0) as f64;
                    // DNS, TCP and TLS handshakes plus the request: about four round trips, then the bytes.
                    let sim = f.fixed.unwrap_or(4.0 * f.rtt_ms as f64 / 1000.0 + bytes * 8.0 / (f.mbps.max(0.01) as f64 * 1e6));
                    if now >= f.started + sim {
                        ready = Some((sim, bytes));
                    } else {
                        ui.ctx().request_repaint();
                    }
                }
                None => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
            }
        }
        if let Some((sim, bytes)) = ready {
            let f = b.pending.take().unwrap();
            let (r, at) = f.got.unwrap();
            match r {
                Ok(html) => {
                    b.parsed = html::parse(&html);
                    // shortcut: very long real pages are cut off after 400 blocks; scrolling all of it would be slow.
                    b.parsed.blocks.truncate(400);
                    b.note = Some(match f.fixed {
                        Some(_) => "Measured over this simulated network.".into(),
                        None => format!(
                            "Real page, {:.0} kB · {:.1} s over this network (≈ {:.0} Mbit/s, {:.0} ms round trip){}",
                            bytes / 1000.0,
                            sim,
                            f.mbps,
                            f.rtt_ms,
                            if at - f.started > sim + 0.3 { trf(" — your real connection needed {} s", &[&format!("{:.1}", at - f.started)]) } else { String::new() }
                        ),
                    });
                    b.page = Some((f.shown, html));
                    b.error = None;
                }
                Err(e) => b.error = Some(trf("The simulated network reached the internet, but the real website didn't answer: {}", &[&e])),
            }
        }
        if let Some(url) = go {
            let mut out = net::Outcome::default();
            let res = self.net.service(id, &url, "web", &mut out);
            let b = self.browsers.get_mut(&id).unwrap();
            let shown = url.trim_start_matches("https://").trim_start_matches("http://").to_string();
            b.url = shown.clone();
            b.pending = None;
            b.note = None;
            match (res, out.fetch.clone()) {
                (Some(_), Some((real_url, rtt))) => {
                    let host = shown.split('/').next().unwrap_or("").trim_start_matches("www.").to_string();
                    let (tx, rx) = std::sync::mpsc::channel();
                    let mbps = self.e2e.get(&id).map(|e| e.0).or_else(|| self.net.internet_rate(id, &self.net.stp()).map(|e| e.0)).unwrap_or(10.0).min(10_000.0);
                    let started = ui.input(|i| i.time);
                    let mut fixed = None;
                    if SPEED_TESTS.contains(&host.as_str()) {
                        // Our own test page, with the simulated result.
                        let res = self.net.exec(id, &format!("speedtest {host}"));
                        let mut page = format!("<html><head><title>Speed test</title></head><body><h1>Speed test — {host}</h1>");
                        for l in res.lines.iter().skip(1) {
                            page += &format!("<p>{}</p>", l.trim().replace('<', "&lt;"));
                        }
                        page += tr("<p>The real site needs JavaScript; this page shows what the simulated network delivers.</p></body></html>");
                        let _ = tx.send(Ok(page));
                        fixed = Some(3.0);
                    } else {
                        let ctx = ui.ctx().clone();
                        std::thread::spawn(move || {
                            let _ = tx.send(fetch_real(&real_url));
                            ctx.request_repaint();
                        });
                    }
                    let b = self.browsers.get_mut(&id).unwrap();
                    b.error = None;
                    b.pending = Some(Fetch { shown, rx, started, rtt_ms: rtt, mbps, fixed, got: None });
                }
                (Some(html), None) => {
                    b.parsed = html::parse(&html);
                    b.page = Some((shown, html));
                    b.error = None;
                }
                (None, _) => b.error = Some(out.lines.last().cloned().unwrap_or_else(|| "Couldn't load the page.".into())),
            }
            let now = Self::now(ui.ctx());
            self.show_outcome(now, out);
        }
    }

    fn terminal(&mut self, ui: &mut egui::Ui, id: u32) {
        let d = self.net.dev(id).unwrap().clone();
        let suggestions: Vec<String> = {
            let other = self.net.devices.iter().find(|x| x.id != id && x.kind.is_host() && x.first_ip().is_some()).map(|x| x.hostname());
            // Only commands that make sense from this device.
            let mut s = vec!["ipconfig".to_string()];
            if d.kind == Kind::Internet {
                let wan = self.net.devices.iter().find(|r| r.kind == Kind::HomeRouter).and_then(|r| r.ifaces.get(1)).map(|f| f.ip.clone()).filter(|ip| !ip.is_empty());
                if let Some(w) = wan {
                    s.push(format!("ping {w}"));
                    s.push(format!("open {w}"));
                }
                if let Some(o) = &other {
                    s.push(format!("ping6 {o}"));
                }
            } else {
                s.push(tr("ping example.com").into());
                s.push("speedtest".into());
                if d.kind.has_browser() {
                    s.push(tr("open example.com").into());
                }
                if let Some(o) = &other {
                    s.push(format!("ping {o}"));
                    s.push(format!("traceroute {o}"));
                }
                if let Some(nas) = self.net.devices.iter().find(|x| x.id != id && x.services.files && x.first_ip().is_some()) {
                    s.push(format!("test {} files", nas.hostname()));
                }
                if self.net.wifi_link_of(id).is_some() {
                    s.push("wifi".into());
                }
            }
            s.push("help".into());
            s
        };
        let mut run = None;
        ui.horizontal_wrapped(|ui| {
            for s in &suggestions {
                if ui.small_button(RichText::new(s).monospace()).clicked() {
                    run = Some(s.clone());
                }
            }
        });
        let term = self.terms.entry(id).or_insert_with(|| Term { lines: vec![trf("{} — type help for commands", &[&d.name])], input: String::new() });
        let h = ui.available_height() - 64.0;
        egui::Frame::new().fill(pal().bg).corner_radius(8).inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.set_min_height(h.max(120.0));
            egui::ScrollArea::vertical().stick_to_bottom(true).max_height(h.max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
                for l in &term.lines {
                    let col = if l.starts_with("$ ") { pal().accent } else if l.contains("timeout") || l.contains("Can't") || l.contains("refused") || l.contains("NXDOMAIN") { pal().red } else { pal().text };
                    ui.label(RichText::new(l).monospace().color(col));
                }
            });
        });
        let resp = ui.add(TextEdit::singleline(&mut term.input).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).hint_text(tr("type a command, e.g. ping example.com")));
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            run = Some(std::mem::take(&mut term.input));
            resp.request_focus();
        }
        if let Some(cmd) = run.filter(|c| !c.trim().is_empty()) {
            let ctx = ui.ctx().clone();
            self.run_command(&ctx, id, cmd);
        }
    }

    // ---------- bottom ----------

    fn bottom_panel(&mut self, ui: &mut egui::Ui, issues: &[Issue]) {
        let errs = issues.iter().filter(|i| i.level != Level::Tip).count();
        ui.horizontal(|ui| {
            let tab = |ui: &mut egui::Ui, me: &mut Bottom, t: Bottom, label: String| {
                if ui.selectable_label(*me == t, label).clicked() {
                    *me = t;
                }
            };
            // Narrow: icons and counts only, so the tabs and the log filter fit on one line.
            let narrow = ui.available_width() < 700.0;
            let t = |icon: &str, long: String, short: String| if narrow { format!("{icon} {short}") } else { format!("{icon}  {long}") };
            tab(ui, &mut self.bottom, Bottom::Problems, t(ic::WARNING, trf("Problems ({})", &[&errs]), errs.to_string()));
            // Packets only travel in Simulate: while designing, the empty log stays out of the way.
            if self.mode == Mode::Simulate || !self.rows.is_empty() {
                tab(ui, &mut self.bottom, Bottom::Packets, t(ic::LIST_BULLETS, trf("Packet log ({})", &[&self.rows.len()]), self.rows.len().to_string()));
            } else if self.bottom == Bottom::Packets {
                self.bottom = Bottom::Problems;
            }
            tab(ui, &mut self.bottom, Bottom::Shopping, t(ic::SHOPPING_CART, tr("Shopping list").into(), String::new()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.mode == Mode::Simulate {
                    let mut stepping = self.step.is_some();
                    if ui.checkbox(&mut stepping, tr("Step through")).on_hover_text(tr("Packets stop at every device; Next step moves them on and shows what the packet carries there")).changed() {
                        self.step = stepping.then_some(0);
                        // The step window shows the layers itself; two such windows would cover the map.
                        self.pkt_view = None;
                    }
                }
                if self.bottom == Bottom::Packets {
                    if ui.small_button(format!("{} {}", ic::BROOM, tr("Clear"))).clicked() {
                        self.rows.clear();
                    }
                    ui.checkbox(&mut self.only_selected, tr("Only selected device"));
                }
            });
        });
        ui.separator();
        egui::ScrollArea::both().auto_shrink([false, false]).stick_to_bottom(self.bottom == Bottom::Packets).show(ui, |ui| match self.bottom {
            Bottom::Problems => {
                if issues.is_empty() {
                    ui.label(RichText::new(format!("{}  {}", ic::CHECK_CIRCLE, tr("Everything looks good."))).color(pal().green));
                }
                let fixable = issues.iter().filter(|i| i.fix.is_some()).count();
                if fixable > 1 && ui.button(RichText::new(trf("{}  Apply all {} one-click fixes", &[&(ic::WRENCH), &fixable])).color(pal().accent)).on_hover_text(tr("Every fix below in one go (Ctrl+Z undoes it)")).clicked() {
                    self.snapshot();
                    let n = self.net.fix_all().len();
                    self.issues = None;
                    let ctx = ui.ctx().clone();
                    self.say(&ctx, trf("{} fixes applied — see what's left in the list.", &[&n]));
                }
                for i in issues {
                    let (clicked, fix) = issue_row(ui, i);
                    if clicked {
                        match i.dev {
                            Some(d) => self.focus(d),
                            None => self.sel = i.link.map(Sel::Link).unwrap_or(Sel::None),
                        }
                    }
                    if let Some(f) = fix {
                        self.snapshot();
                        let msg = self.net.apply_fix(&f);
                        let ctx = ui.ctx().clone();
                        self.say(&ctx, msg);
                    }
                }
            }
            Bottom::Packets => {
                let sel = if let Sel::Dev(d) = self.sel { Some(d) } else { None };
                if self.rows.is_empty() {
                    ui.label(RichText::new(tr("No traffic yet. Switch to Simulate and run a command.")).color(pal().dim));
                }
                let rows: Vec<&Row> = self.rows.iter().filter(|r| !self.only_selected || sel.is_none() || Some(r.log.dev) == sel).collect();
                if let Some(l) = packet_grid(ui, "pk", &rows) {
                    self.pkt_view = Some(l);
                }
            }
            Bottom::Shopping => {
                let (items, total) = shopping(&self.net);
                egui::Grid::new("shop").striped(true).num_columns(3).spacing(vec2(24.0, 3.0)).show(ui, |ui| {
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
            }
        });
    }

    // ---------- learn ----------

    fn learn(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("lessons").exact_size(270.0).frame(egui::Frame::new().fill(pal().panel).inner_margin(Margin::same(12))).show(ui, |ui| {
            let done = (0..lessons::LESSONS.len()).filter(|i| self.settings.quiz >> i & 1 == 1).count();
            ui.label(RichText::new(trf("LESSONS · {} of {} checked", &[&done, &lessons::LESSONS.len()])).small().color(pal().dim));
            egui::ScrollArea::vertical().show(ui, |ui| {
                for i in 0..lessons::LESSONS.len() {
                    let title = lessons::lesson(i).0;
                    let on = self.lesson == i;
                    let mark = if self.settings.quiz >> i & 1 == 1 { ic::CHECK_CIRCLE } else { "" };
                    let b = egui::Button::new(RichText::new(format!("{:>2}  {title}  {mark}", i + 1)).color(if on { pal().accent } else { pal().text })).fill(if on { pal().card } else { Color32::TRANSPARENT }).min_size(vec2(ui.available_width(), 30.0));
                    if ui.add(b).clicked() {
                        self.lesson = i;
                    }
                }
                ui.add_space(10.0);
                ui.label(RichText::new(tr("FIND THE FAULT")).small().color(pal().dim));
                for (key, goal) in scenarios::EXERCISES {
                    let label = scenarios::SCENARIOS.iter().find(|s| s.0 == *key).map_or(*key, |s| s.1);
                    if ui.add(egui::Button::new(format!("{}  {}", ic::TARGET, tr(label))).fill(Color32::TRANSPARENT).min_size(vec2(ui.available_width(), 28.0))).on_hover_text(tr(goal)).clicked() {
                        self.load(net::template(key));
                        self.file = None;
                        self.mode = Mode::Design;
                    }
                }
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg).inner_margin(Margin::symmetric(48, 32))).show(ui, |ui| {
            let (title, body, try_it) = lessons::lesson(self.lesson);
            let scenario = lessons::LESSONS[self.lesson].3;
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(760.0);
                ui.label(RichText::new(trf("Lesson {} of {}", &[&(self.lesson + 1), &lessons::LESSONS.len()])).color(pal().dim));
                ui.label(RichText::new(title).size(28.0).strong());
                ui.add_space(12.0);
                for p in body.split("\n\n") {
                    ui.label(RichText::new(p).size(15.5).line_height(Some(24.0)));
                    ui.add_space(8.0);
                }
                egui::Frame::new().fill(pal().card).corner_radius(10).inner_margin(Margin::same(14)).show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{}  {}", ic::LIGHTNING, tr("Try it"))).strong().color(pal().accent));
                        ui.label(try_it);
                    });
                });
                if let Some((question, answers, right, why)) = lessons::quiz(self.lesson) {
                    ui.add_space(10.0);
                    egui::Frame::new().fill(pal().card).corner_radius(10).inner_margin(Margin::same(14)).show(ui, |ui| {
                        ui.label(RichText::new(format!("{}  {}", ic::QUESTION, tr("Check yourself"))).strong().color(pal().accent));
                        ui.label(question);
                        let picked = self.quiz_pick.filter(|p| p.0 == self.lesson).map(|p| p.1);
                        for (i, a) in answers.iter().enumerate() {
                            if ui.radio(picked == Some(i), *a).clicked() {
                                self.quiz_pick = Some((self.lesson, i));
                                if i == right {
                                    self.settings.quiz |= 1 << self.lesson;
                                    self.settings.save();
                                }
                            }
                        }
                        if let Some(p) = picked {
                            let (icon, verdict, col) = if p == right { (ic::CHECK_CIRCLE, tr("Right."), pal().green) } else { (ic::X_CIRCLE, tr("Not quite — try another answer."), pal().amber) };
                            ui.label(RichText::new(format!("{icon}  {verdict}")).strong().color(col));
                            if p == right {
                                ui.label(RichText::new(why).color(pal().dim));
                            }
                        }
                    });
                }
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if self.lesson > 0 && ui.button(format!("{}  {}", ic::ARROW_LEFT, tr("Previous"))).clicked() {
                        self.lesson -= 1;
                    }
                    if self.lesson + 1 < lessons::LESSONS.len() && ui.button(format!("{}  {}", tr("Next"), ic::ARROW_RIGHT)).clicked() {
                        self.lesson += 1;
                    }
                    if ui.button(format!("{}  {}", ic::PENCIL_SIMPLE, tr("Open the planner"))).clicked() {
                        self.mode = Mode::Design;
                    }
                    // The example plan this lesson's exercise talks about, one click away.
                    if let Some((key, label, _)) = scenarios::SCENARIOS.iter().find(|s| s.0 == scenario) {
                        if ui.button(format!("{}  {}", ic::FOLDER_OPEN, trf("Open \"{}\"", &[&tr(label.split(" (").next().unwrap_or(label))]))).clicked() {
                            self.load(net::template(key));
                            self.file = None;
                            self.mode = Mode::Design;
                        }
                    }
                });
            });
        });
    }
}

fn stat(ui: &mut egui::Ui, value: &str, label: &str, col: Color32) {
    egui::Frame::new().fill(pal().card).corner_radius(8).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.set_min_width(54.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(value).size(18.0).strong().color(col));
            ui.label(RichText::new(tr(label)).small().color(pal().dim));
        });
    });
}

/// The packet log as a table. Returns the line clicked, for the layer view.
fn packet_grid(ui: &mut egui::Ui, id: &str, rows: &[&Row]) -> Option<net::Log> {
    let mut picked = None;
    egui::Grid::new(id).striped(true).num_columns(6).spacing(vec2(14.0, 3.0)).show(ui, |ui| {
        for h in [tr("Time"), tr("Source"), tr("Destination"), tr("Protocol"), tr("Layer"), tr("What happens")] {
            ui.label(RichText::new(tr(h)).small().color(pal().dim));
        }
        ui.end_row();
        for r in rows {
            ui.label(RichText::new(format!("{:.2} s", r.t)).monospace().color(pal().dim));
            ui.label(RichText::new(&r.log.src).monospace());
            ui.label(RichText::new(&r.log.dst).monospace());
            ui.label(RichText::new(r.log.proto).strong().color(proto_color(r.log.proto)));
            ui.label(RichText::new(r.log.layer).color(pal().dim));
            let info = ui.add(egui::Label::new(&r.log.info).sense(egui::Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tr("Click: this packet layer by layer"));
            if info.clicked() {
                picked = Some(r.log.clone());
            }
            ui.end_row();
        }
    });
    picked
}

/// A packet's headers, layer by layer (Ethernet, IP, transport, application), outermost first.
fn layer_list(ui: &mut egui::Ui, log: &net::Log) {
    let layers = log.layers();
    if layers.is_empty() {
        ui.label(RichText::new(tr("This line is a note from a device, not a packet with headers.")).color(pal().dim));
    }
    // Stacked, not a grid: a wrapping grid sizes itself far too tall on its first frame.
    for (name, text) in layers {
        ui.label(RichText::new(tr(name)).strong().color(pal().accent));
        ui.label(RichText::new(text).monospace());
    }
}

impl App {
    /// The layer view of a clicked log line, and the step-through controls.
    fn packet_windows(&mut self, ctx: &egui::Context) {
        if let Some(log) = self.pkt_view.clone() {
            let mut open = true;
            egui::Window::new(format!("{}  {}", ic::STACK, tr("Packet layers"))).id(Id::new("pkt_view")).pivot(Align2::RIGHT_TOP).default_pos(self.canvas_rect.right_top() + vec2(-12.0, 56.0)).open(&mut open).resizable(false).default_width(460.0).show(ctx, |ui| {
                ui.label(RichText::new(format!("{}  {} → {}", log.proto, log.src, log.dst)).strong().monospace());
                ui.label(RichText::new(&log.info).color(pal().dim));
                ui.add_space(4.0);
                layer_list(ui, &log);
            });
            if !open {
                self.pkt_view = None;
            }
        }
        let Some(k) = self.step.filter(|_| self.mode == Mode::Simulate) else { return };
        let total: usize = self.anims.iter().map(|a| a.path.len()).sum();
        let mut next = false;
        egui::Window::new(format!("{}  {}", ic::FOOTPRINTS, tr("Step through"))).id(Id::new("step")).pivot(Align2::LEFT_BOTTOM).default_pos(self.canvas_rect.left_bottom() + vec2(12.0, -12.0)).resizable(false).default_width(460.0).show(ctx, |ui| {
            let Some((a, i)) = step_at(&self.anims, k) else {
                ui.label(RichText::new(tr("No packet on its way. Run a command (ping, open…) and step through it here.")).color(pal().dim));
                return;
            };
            let at = self.net.name(a.path[i]);
            let state = if i + 1 < a.path.len() { tr("on its way") } else if a.ok { tr("arrived") } else { tr("stopped here") };
            ui.label(RichText::new(trf("Step {} of {}: the packet is at {} ({})", &[&(k + 1), &total, &at, &state])).strong());
            // The newest hop line sent from a device the packet has already passed.
            if let Some(h) = a.hops.iter().rev().find(|h| a.path[..=i].contains(&h.dev)) {
                ui.label(RichText::new(&h.info).color(pal().dim));
                layer_list(ui, h);
            }
            ui.add_space(4.0);
            next = ui.button(format!("{}  {}", ic::ARROW_RIGHT, tr("Next step"))).clicked();
        });
        if next {
            self.step = Some(k + 1);
        } else if k >= total && !self.anims.is_empty() {
            // Everything has arrived: start the next command's packets from their first step.
            self.anims.clear();
            self.step = Some(0);
        }
    }
}

fn proto_color(p: &str) -> Color32 {
    match p {
        "DHCP" => Color32::from_rgb(167, 139, 250),
        "DNS" => Color32::from_rgb(96, 165, 250),
        "ARP" => pal().dim,
        "NAT" => pal().amber,
        "TLS" | "HTTP" => pal().green,
        "802.11" => Color32::from_rgb(45, 212, 191),
        "802.1Q" => Color32::from_rgb(232, 121, 249),
        _ => pal().accent,
    }
}

/// (item, quantity, cost) rows and the total.
fn shopping(n: &Network) -> (Vec<(String, usize, f32)>, f32) {
    let mut items: Vec<(String, usize, f32)> = vec![];
    // One line per model picked (or per kind without one); what's already in the home is left out.
    for k in Kind::SHELF.into_iter().filter(|k| *k != Kind::Internet && !k.foreign()) {
        let mut models: Vec<(String, usize, f32)> = vec![];
        for d in n.devices.iter().filter(|d| d.kind == k && !d.owned()) {
            let what = if d.model.is_empty() { k.label().to_string() } else { format!("{} ({})", d.model, k.label()) };
            match models.iter_mut().find(|m| m.0 == what) {
                Some(m) => (m.1, m.2) = (m.1 + 1, m.2 + d.price),
                None => models.push((what, 1, d.price)),
            }
        }
        items.extend(models);
    }
    for m in Medium::ALL.into_iter().filter(|m| !m.wireless()) {
        let ls: Vec<_> = n.links.iter().filter(|l| l.medium == m).collect();
        if !ls.is_empty() {
            let len: f32 = ls.iter().map(|l| n.link_len(l) + 1.0).sum();
            let what = if m.cable() { trf("{} ({} m incl. slack)", &[&(m.label()), &format!("{:.0}", len)]) } else { m.label().to_string() };
            let each: f32 = ls.iter().map(|l| catalog::find_kit(&l.kit).map_or(m.per_link_price(), |k| k.price)).sum();
            items.push((what, ls.len(), len * m.price_per_m() + each));
        }
    }
    let ups: Vec<f32> = n.devices.iter().map(|d| d.ups_wh).filter(|w| *w > 0.0).collect();
    if !ups.is_empty() {
        // shortcut: a rough street price from the battery size; one UPS per device that has one set.
        items.push(("UPS".to_string(), ups.len(), ups.iter().map(|w| 60.0 + 0.5 * w).sum()));
    }
    let total = items.iter().map(|i| i.2).sum();
    (items, total)
}

fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

impl eframe::App for App {
    /// Screenshots ignore the real mouse, so a pointer resting on the window adds no hover or tooltip.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if self.screenshot.is_some() {
            raw.events.retain(|e| !matches!(e, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. }));
        }
        // With nothing focused, Tab walks through the devices on the map instead of the buttons.
        if self.mode != Mode::Learn && ctx.memory(|m| m.focused().is_none()) {
            raw.events.retain(|e| match e {
                egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. } => {
                    self.tab = Some(if modifiers.shift { -1 } else { 1 });
                    false
                }
                egui::Event::Key { key: egui::Key::Tab, .. } => false,
                _ => true,
            });
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;
        // Within one frame the plan doesn't change: every link's signal, airtime and the spanning tree are worked
        // out once and reused (see wifi::Memo).
        self.net.memo.borrow_mut().reset(true);
        // A map picture the user asked for.
        if self.export_pic.is_some() {
            if let Some(img) = ctx.input(|i| i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })) {
                self.save_picture(&ctx, &img);
            }
        }
        if let Some(path) = self.screenshot.clone() {
            ctx.request_repaint();
            if self.frame == 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let shot = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None })
            });
            if let Some(img) = shot {
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = image::save_buffer(&path, &bytes, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8) {
                    eprintln!("screenshot failed: {e}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        let typing = ctx.egui_wants_keyboard_input();
        let redo_key = ctx.input(|i| i.modifiers.command && (i.key_pressed(egui::Key::Y) || (i.modifiers.shift && i.key_pressed(egui::Key::Z))));
        if !typing && redo_key {
            self.redo_once(&ctx);
        } else if !typing && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo_once(&ctx);
        }
        // Single keys for the tools and the view (not while typing).
        if !typing && self.mode == Mode::Design && !ctx.input(|i| i.modifiers.command) {
            for (k, t) in [(egui::Key::V, Tool::Select), (egui::Key::C, Tool::Connect), (egui::Key::W, Tool::Wall), (egui::Key::M, Tool::Measure)] {
                if ctx.input(|i| i.key_pressed(k)) {
                    self.tool = t;
                }
            }
        }
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::F) && !i.modifiers.command) {
            self.fit();
        }
        // Keyboard use of the map: Tab picks devices on this storey in turn, the arrow keys nudge the picked one.
        if let Some(dir) = self.tab.take() {
            let ids: Vec<u32> = self.net.devices.iter().filter(|d| d.floor == self.floor).map(|d| d.id).collect();
            if !ids.is_empty() {
                let at = ids.iter().position(|i| self.sel == Sel::Dev(*i)).map_or(if dir > 0 { ids.len() - 1 } else { 0 }, |p| p);
                let id = ids[(at as i32 + dir).rem_euclid(ids.len() as i32) as usize];
                self.sel = Sel::Dev(id);
                let at = self.to_screen(Vec2::from(self.net.dev(id).unwrap().pos));
                if !self.canvas_rect.shrink(40.0).contains(at) {
                    self.pan += self.canvas_rect.center() - at;
                }
                self.say(&ctx, trf("{} — arrow keys move it, Delete removes it.", &[&(self.net.name(id))]));
            }
        }
        if let (false, Mode::Design, Sel::Dev(id)) = (typing, self.mode, self.sel) {
            let step = PX_PER_M / 4.0;
            let d = ctx.input(|i| [(egui::Key::ArrowLeft, [-step, 0.0]), (egui::Key::ArrowRight, [step, 0.0]), (egui::Key::ArrowUp, [0.0, -step]), (egui::Key::ArrowDown, [0.0, step])].into_iter().filter(|(k, _)| i.key_pressed(*k)).map(|x| x.1).next());
            if let Some([dx, dy]) = d {
                let now = Self::now(&ctx);
                if now - self.nudged > 1.0 {
                    self.snapshot();
                }
                self.nudged = now;
                if let Some(dev) = self.net.dev_mut(id) {
                    dev.pos = [dev.pos[0] + dx, dev.pos[1] + dy];
                }
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.open(&ctx);
        }
        if !typing && self.mode == Mode::Design && self.sel != Sel::None && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            self.snapshot();
            self.connect_from = None;
            self.say(&ctx, tr("Deleted — Ctrl+Z brings it back."));
            match self.sel {
                Sel::Dev(id) => self.net.remove_device(id),
                Sel::Link(id) => self.net.remove_link(id),
                Sel::Wall(id) => self.net.walls.retain(|w| w.id != id),
                Sel::Point(id) => self.net.points.retain(|p| p.id != id),
                Sel::Label(id) => self.net.labels.retain(|l| l.id != id),
                Sel::None => {}
            }
            self.sel = Sel::None;
        }
        // Esc in the browser's address bar or the terminal only leaves the text field.
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.connect_from = None;
            self.wall_start = None;
            self.measure = None;
            if self.route.take().is_none() {
                self.sel = Sel::None;
            }
        }
        // Planning and problem checks use long-run averages; only Simulate runs live variation.
        let live = self.mode == Mode::Simulate && self.live_var;
        self.net.clock = Self::now(&ctx);
        self.net.live = false;
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            let again = ctx.input(|i| i.modifiers.shift);
            self.save(&ctx, again);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.load(net::template("blank"));
            self.file = None;
            if self.mode == Mode::Learn {
                self.mode = Mode::Design;
            }
        }
        if !typing && self.mode == Mode::Design && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            if let Sel::Dev(id) = self.sel {
                self.snapshot();
                if let Some(new) = self.net.duplicate(id) {
                    self.sel = Sel::Dev(new);
                }
            }
        }
        self.autosave(&ctx);
        self.phone_tick(&ctx);
        // Files dropped on the window: plans open, surveys import, pictures go under the map.
        for path in ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect::<Vec<_>>()) {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let survey = ext == "json" && std::fs::read_to_string(&path).is_ok_and(|t| t.contains("\"netsketch-survey\""));
            match ext.as_str() {
                "netsurvey" | "csv" | "tsv" | "txt" => self.import_measurements(&ctx, &path),
                "json" if survey => self.import_measurements(&ctx, &path),
                "netplan" | "json" | "fls" => self.open_path(&ctx, path),
                "png" | "jpg" | "jpeg" => self.add_picture(&ctx, &path),
                _ => self.say(&ctx, trf("NetSketch can't use {} files.", &[&ext])),
            }
        }
        // Fingerprinting serialises the whole plan: only after input (every edit comes from input or a snapshot).
        if ctx.input(|i| !i.events.is_empty() || i.pointer.any_down()) {
            self.touched = [true; 2];
        }
        if Self::now(&ctx) - self.dirty_at > 1.0 && (self.touched[0] || self.issues.is_none()) {
            self.dirty_at = Self::now(&ctx);
            self.touched[0] = false;
            self.dirty = plan_hash(&self.net) != self.saved_hash;
        }
        // Closing with unsaved changes asks first (the screenshot mode closes by itself).
        if ctx.input(|i| i.viewport().close_requested()) && self.screenshot.is_none() && !self.quit_ok && plan_hash(&self.net) != self.saved_hash {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.ask_quit = true;
        }
        if self.ask_quit {
            self.quit_dialog(&ctx);
        }
        // The window title names the plan (and its file); • means unsaved changes.
        let mark = if self.dirty { "• " } else { "" };
        let title = match &self.file {
            Some(f) => format!("{mark}{} — {} — NetSketch", self.net.title, f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            None => format!("{mark}{} — NetSketch", self.net.title),
        };
        if title != self.shown_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shown_title = title;
        }

        if let Some(old) = self.before.take() {
            self.snapshot_before(old);
        }
        // Cables follow port edits and new hardware before anything reads their speed.
        if let Some(id) = self.replug.take() {
            self.net.replug(id);
        }
        self.net.assign_ports();
        // The problem list and busy-hour traffic refresh a few times a second, not every frame (big plans stay smooth).
        let now = Self::now(&ctx);
        let due = self.issues_at + 0.3 <= now || self.issues.is_none();
        // Simulate moves things that aren't saved (walks, roaming), so it always refreshes; Design only after an edit.
        let key = if due && self.mode != Mode::Simulate {
            if self.touched[1] || self.issues.is_none() {
                self.touched[1] = false;
                plan_hash(&self.net)
            } else {
                self.issues_key
            }
        } else {
            0
        };
        if due && (key == 0 || key != self.issues_key || self.issues.is_none()) {
            self.issues_key = key;
            self.issues = Some(self.net.check());
            // Traffic right now comes and goes with live variation; the problem list above stays on averages.
            self.load_cache = (self.mode == Mode::Simulate).then(|| {
                self.net.live = live;
                let now = self.net.loads_now();
                self.net.live = false;
                now
            });
            self.avg_cache = (self.mode == Mode::Simulate).then(|| self.net.loads());
            let stp = self.net.stp();
            self.stp_cache = Some(stp.clone());
            self.e2e = self.net.devices.iter().filter(|d| d.kind.is_host() && !d.off).filter_map(|d| Some((d.id, self.net.internet_rate(d.id, &stp)?))).collect();
        }
        if due {
            self.issues_at = now;
        }
        let issues = self.issues.clone().unwrap_or_default();
        self.net.live = live;
        self.tick(&ctx);
        if live {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        egui::Panel::top("top").frame(egui::Frame::new().fill(pal().panel).inner_margin(Margin::symmetric(14, 9)).stroke(Stroke::new(1.0, pal().line))).show(ui, |ui| self.top_bar(ui));

        if let Some((msg, until)) = self.toast.clone() {
            if Self::now(&ctx) < until {
                // Over the map only, so it never hides the settings panel.
                let c = if self.canvas_rect.is_positive() { self.canvas_rect } else { ctx.content_rect() };
                egui::Area::new(Id::new("toast")).fixed_pos(pos2(c.center().x, c.top() + 50.0)).pivot(Align2::CENTER_TOP).order(egui::Order::Foreground).show(&ctx, |ui| {
                    egui::Frame::new().fill(pal().card).stroke(Stroke::new(1.0, pal().accent)).corner_radius(10).inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
                        ui.set_max_width(520.0f32.min(c.width() - 60.0).max(200.0));
                        ui.label(msg);
                    });
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            } else {
                self.toast = None;
            }
        }

        if self.mode == Mode::Learn {
            self.learn(ui);
            return;
        }

        let side = egui::Frame::new().fill(pal().panel).inner_margin(Margin::same(12));
        let screen = ctx.content_rect();
        if self.mode == Mode::Design {
            egui::Panel::left("shelf").exact_size(if screen.width() < 1200.0 { 186.0 } else { 224.0 }).frame(side).show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.shelf(ui));
            });
        }
        // Side panels can't squeeze the map away: the inspector stays under ~40% of the window, the log under 40% of its height.
        egui::Panel::right("inspector").default_size((screen.width() * 0.32).clamp(320.0, 420.0)).size_range(320.0..=(screen.width() * 0.4).max(340.0)).frame(side).show(ui, |ui| {
            if self.mode == Mode::Simulate && !matches!(self.sel, Sel::Dev(_)) {
                Self::header(ui, ic::PLAY, pal().accent, tr("Simulation"), tr("Click any device to use its apps"));
                ui.add_space(6.0);
                ui.label(RichText::new(tr("Addresses were handed out by DHCP. Pick a device on the map: see its Wi-Fi, take it for a walk to watch it roam, open a web page in its browser, or run ping, speedtest, wifi and stp on its command line. Packets move along the cables and every step lands in the packet log below.")).color(pal().dim));
                ui.add_space(6.0);
                if ui.button(format!("{}  {}", ic::PENCIL_SIMPLE, tr("Back to design"))).clicked() {
                    self.mode = Mode::Design;
                }
                egui::ScrollArea::vertical().show(ui, |ui| self.resilience_and_power(ui));
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| self.inspector(ui, &issues));
        });
        egui::Panel::bottom("bottom").default_size(190.0).size_range(90.0..=(screen.height() * 0.4).max(120.0)).resizable(true).frame(side).show(ui, |ui| self.bottom_panel(ui, &issues));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(ui, |ui| self.canvas(ui, &issues));
        self.packet_windows(&ctx);
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args.iter().position(|a| a == "--screenshot").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    // A plan to open, e.g. from double-clicking a .netplan file.
    let open = args.iter().skip(1).find(|a| !a.starts_with("--") && Some(a.as_str()) != screenshot.as_deref().and_then(|p| p.to_str())).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1440.0, 900.0]).with_min_inner_size([960.0, 600.0]).with_title(tr("NetSketch — plan, simulate and learn networks"))
            .with_app_id("netsketch")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../packaging/netsketch.png")).unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native("NetSketch", options, Box::new(move |cc| Ok(Box::new(App::new(cc, screenshot, open)))))
}
