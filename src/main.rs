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
mod soft;
mod stp;
mod ui_companion;
mod ui_desk;
mod ui_draw;
mod ui_extras;
mod ui_kit;
mod ui_more;
mod ui_select;
mod ui_shell;
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
use ui_shell::Space;
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
/// Neutral greys with one teal accent: red, amber and green only ever mean a problem, a warning or all good.
const PALETTES: [Palette; 3] = [
    Palette { bg: rgb(17, 18, 21), panel: rgb(24, 25, 29), card: rgb(33, 35, 40), line: rgb(47, 50, 56), text: rgb(228, 230, 234), dim: rgb(146, 150, 159), accent: rgb(52, 196, 176), red: rgb(240, 104, 104), amber: rgb(232, 172, 58), green: rgb(86, 204, 128), hover: rgb(40, 42, 48), press: rgb(50, 53, 60) },
    Palette { bg: rgb(244, 245, 247), panel: rgb(255, 255, 255), card: rgb(240, 241, 244), line: rgb(222, 224, 229), text: rgb(27, 29, 33), dim: rgb(98, 103, 112), accent: rgb(13, 128, 116), red: rgb(196, 40, 40), amber: rgb(163, 100, 8), green: rgb(22, 128, 62), hover: rgb(232, 234, 238), press: rgb(220, 223, 228) },
    Palette { bg: rgb(0, 0, 0), panel: rgb(10, 10, 10), card: rgb(26, 26, 26), line: rgb(160, 160, 160), text: rgb(255, 255, 255), dim: rgb(214, 214, 214), accent: rgb(0, 255, 230), red: rgb(255, 92, 92), amber: rgb(255, 214, 10), green: rgb(80, 255, 120), hover: rgb(50, 50, 50), press: rgb(70, 70, 70) },
];

/// Which of PALETTES is in use (saved in the settings file).
static THEME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn pal() -> &'static Palette {
    &PALETTES[THEME.load(std::sync::atomic::Ordering::Relaxed).min(PALETTES.len() - 1)]
}

fn kind_color(k: Kind) -> Color32 {
    // One calm colour: the icon tells the kind, the status dot tells the health. Others' gear is dimmed.
    if k.foreign() { pal().dim } else { pal().text }
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
    // Cable kinds stay told apart, in muted tones; Wi-Fi in the accent.
    let c = match m {
        Medium::Cat5e => rgb(128, 134, 146),
        Medium::Cat6 => rgb(110, 150, 200),
        Medium::Cat6a => rgb(96, 176, 150),
        Medium::Fiber => rgb(236, 142, 58),
        Medium::WiFi => return pal().accent,
        Medium::Cat5 => rgb(214, 104, 146),
        Medium::Powerline => rgb(196, 170, 88),
        Medium::Mesh => rgb(176, 136, 200),
        Medium::Cellular => rgb(200, 180, 90),
    };
    // Darker on the light theme so thin lines stay visible.
    if THEME.load(std::sync::atomic::Ordering::Relaxed) == 1 { rgb((c.r() as u16 * 4 / 5) as u8, (c.g() as u16 * 4 / 5) as u8, (c.b() as u16 * 4 / 5) as u8) } else { c }
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
    ("Arrow keys", "Nudge what's selected by 25 cm (Shift: 1 m)"),
    ("Shift+click / Shift+drag", "Select several things (a box on empty map)"),
    ("Delete / Backspace", "Remove what's selected"),
    ("Esc", "Deselect, end a wall"),
    ("Ctrl+Z / Ctrl+Y", "Undo, redo"),
    ("Ctrl+Shift+Z", "Redo"),
    ("Ctrl+D", "Duplicate"),
    ("Ctrl+C / Ctrl+X / Ctrl+V", "Copy, cut, paste where the pointer is (also on another storey)"),
    ("Ctrl+S / Ctrl+O", "Save, open"),
    ("Ctrl+Shift+S", "Save as a new file"),
    ("Ctrl+N", "New blank plan"),
    ("Mouse wheel / pinch", "Zoom the map"),
    ("Drag empty map / middle button / Space+drag", "Move the map"),
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
            w.corner_radius = CornerRadius::same(6);
        }
        v.widgets.inactive.weak_bg_fill = pal().card;
        // Checkboxes, radios and slider rails use bg_fill: the page colour keeps them visible on cards and menus.
        v.widgets.inactive.bg_fill = pal().bg;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, pal().dim.gamma_multiply(0.45));
        v.widgets.hovered.weak_bg_fill = pal().hover;
        v.widgets.active.weak_bg_fill = pal().press;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal().line);
        // Buttons are quiet until hovered; hairline borders; one size of rounding.
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, pal().dim.gamma_multiply(0.6));
        v.widgets.active.bg_stroke = Stroke::new(1.0, pal().accent);
        v.menu_corner_radius = CornerRadius::same(10);
        v.window_corner_radius = CornerRadius::same(12);
        v.window_shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(60) };
        v.popup_shadow = egui::Shadow { offset: [0, 3], blur: 12, spread: 0, color: Color32::from_black_alpha(50) };
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 4.0);
        s.spacing.interact_size.y = 24.0;
        use egui::TextStyle as T;
        s.text_styles.insert(T::Body, FontId::proportional(13.5));
        s.text_styles.insert(T::Button, FontId::proportional(13.5));
        s.text_styles.insert(T::Heading, FontId::proportional(17.0));
        s.text_styles.insert(T::Small, FontId::proportional(11.5));
        s.text_styles.insert(T::Monospace, FontId::monospace(12.5));
        });
    }
}

/// What an in-place edit replaced, for its undo step (App::snapshot_before).
/// What replaces the open plan: the file picker, a file, or a ready plan (a template, the recovered autosave: true).
enum Swap {
    Open,
    Path(PathBuf),
    Plan(Box<Network>, bool),
}

enum Before {
    Device(net::Device),
    Link(net::Link),
    Wall(wifi::Wall),
    Point(companion::MeasurePoint),
    Label(net::RoomLabel),
}

/// The tabs of a device's panel in Design.
#[derive(PartialEq, Clone, Copy)]
enum DevTab {
    General,
    WiFi,
    Network,
    Security,
    Use,
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
/// The dots flowing one way along a link: the rate they show (gliding towards the real one) and where the lattice
/// is (a share of one base spacing).
struct Stream {
    shown: f32,
    phase: f32,
}

/// Widest spacing of the moving dots (map px at zoom 1): a trickle shows one dot per this; more traffic adds dots
/// halfway between, then quarter-way, down to 5 px.
const DOT_BASE: f32 = 128.0;

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

/// A file's name without its folder, for messages.
fn file_name(p: &std::path::Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())
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
    /// The open workspace of the left rail (None: folded away).
    space: Option<Space>,
    /// The open tab of a device's panel, and the add-device palette with its search text.
    dev_tab: DevTab,
    add_open: bool,
    add_query: String,
    /// The packet log's big window.
    log_window: bool,
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
    /// The last panel edit: what, when, and the undo depth after it (later edits to the same thing join that step).
    panel_edit: Option<(Sel, f64, u64)>,
    /// Undo steps taken so far (counts up forever; tells whether anything else happened since the last panel edit).
    steps: u64,
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
    heat_cache: Option<radio_ui::HeatMap>,
    heat_job: Option<radio_ui::HeatJob>,
    /// The moving dots of each link direction, and when they last moved.
    flow: HashMap<(u32, bool), Stream>,
    flow_t: f64,
    /// Wi-Fi map: 0 = best band, 1..=3 = 2.4 / 5 / 6 GHz only.
    heat_band: usize,
    /// The Wi-Fi map shows internet speed instead of signal.
    heat_speed: bool,
    /// The selected device whose own Wi-Fi map was closed (×): the map shows everything again.
    heat_unfocus: Option<u32>,
    /// More selected things besides `sel` (Shift+click, a box): moved, nudged, deleted and copied together.
    extra: Vec<Sel>,
    /// `sel` as it was at the end of the last frame, and whether this frame changed the selection on purpose.
    sel_seen: Sel,
    multi_touched: bool,
    /// What Ctrl+C took.
    clip: Option<ui_select::Clip>,
    /// Where a Shift+drag selection box started (map point).
    marquee: Option<Vec2>,
    /// The dragged device lined up with another this frame (then it doesn't also snap to the grid).
    guide_hit: bool,
    /// Devices whose desktop window is open, in opening order, and what each shows.
    desks: Vec<u32>,
    desk: HashMap<u32, ui_desk::Desk>,
    /// Outdoor areas' coverage on the shown storey, and when it was worked out.
    outdoor_cache: Option<(f64, Vec<(u32, plan::Coverage)>)>,
    /// The day's load chart (time computed, every half hour: internet line, busiest link), refreshed once a second.
    day_cache: Option<(f64, Vec<(f32, f32)>)>,
    /// Coverage score of the shown storey (storey, when computed, score).
    cov_cache: Option<(i8, f64, Option<plan::Coverage>)>,
    /// Search text for the measured networks, and whether their big window is open.
    measured_filter: String,
    measured_window: bool,
    /// What "Learn walls" found, waiting for Apply.
    wall_learn: Option<crate::companion::WallFit>,
    /// The budget planner: how much, mesh or cables, and what it proposed.
    budget: f32,
    budget_mesh: bool,
    proposal: Option<crate::plan::Proposal>,
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
    /// A plan waiting to replace the open one while "Save your changes?" is asked.
    pending: Option<Swap>,
    quit_ok: bool,
    /// When an arrow key last nudged a device: a run of nudges is one undo step.
    nudged: f64,
    toast: Option<(String, f64)>,
    screenshot: Option<PathBuf>,
    frame: u32,
}

impl App {
    /// The app's state before any plan is loaded.
    fn bare(settings: Settings, screenshot: Option<PathBuf>) -> Self {
        App {
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
            space: Some(Space::Plan),
            dev_tab: DevTab::General,
            add_open: false,
            add_query: String::new(),
            log_window: false,
            only_selected: false,
            terms: HashMap::new(),
            browsers: HashMap::new(),
            sim_tab: SimTab::WiFi,
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
            panel_edit: None,
            steps: 0,
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
            flow: HashMap::new(),
            flow_t: 0.0,
            heat_band: 0,
            heat_speed: false,
            heat_unfocus: None,
            extra: vec![],
            sel_seen: Sel::None,
            multi_touched: false,
            clip: None,
            marquee: None,
            guide_hit: false,
            desks: vec![],
            desk: HashMap::new(),
            cov_cache: None,
            measured_filter: String::new(),
            measured_window: false,
            wall_learn: None,
            budget: 500.0,
            budget_mesh: true,
            proposal: None,
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
            pending: None,
            quit_ok: false,
            nudged: 0.0,
            toast: None,
            screenshot,
            frame: 0,
        }
    }

    /// An app with the home example loaded, for tests that drive the editor without a window.
    #[cfg(test)]
    fn for_tests() -> Self {
        let mut app = Self::bare(Settings::default(), None);
        app.load(net::template("home"));
        app
    }

    fn new(cc: &eframe::CreationContext, screenshot: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        // The text font has no arrows (→ ↔); the built-in monospace font fills them in.
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.push("Hack".to_owned());
        }
        cc.egui_ctx.set_fonts(fonts);
        // Screenshots always look the same, whatever this computer's settings are.
        let settings = if screenshot.is_some() { Settings::default() } else { Settings::load() };
        settings.apply(&cc.egui_ctx);
        let mut app = Self::bare(settings, screenshot);
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
        // Learned and proposed results were worked out for the plan before this edit.
        self.wall_learn = None;
        self.proposal = None;
        self.redo.clear();
        self.steps += 1;
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
            Before::Wall(o) => {
                if let Some(w) = self.net.walls.iter_mut().find(|w| w.id == o.id) {
                    *w = o;
                }
            }
            Before::Point(o) => {
                if let Some(p) = self.net.points.iter_mut().find(|p| p.id == o.id) {
                    *p = o;
                }
            }
            Before::Label(o) => {
                if let Some(l) = self.net.labels.iter_mut().find(|l| l.id == o.id) {
                    *l = o;
                }
            }
        }
        self.snapshot();
        self.net = now;
    }

    /// The selected thing as it is now (for undoing edits made in its panel), and its JSON to spot a change.
    fn sel_copy(&self) -> Option<(Before, String)> {
        let b = match self.sel {
            Sel::Dev(id) => Before::Device(self.net.dev(id)?.clone()),
            Sel::Link(id) => Before::Link(self.net.links.iter().find(|l| l.id == id)?.clone()),
            Sel::Wall(id) => Before::Wall(self.net.walls.iter().find(|w| w.id == id)?.clone()),
            Sel::Point(id) => Before::Point(self.net.points.iter().find(|p| p.id == id)?.clone()),
            Sel::Label(id) => Before::Label(self.net.labels.iter().find(|l| l.id == id)?.clone()),
            _ => return None,
        };
        let json = match &b {
            Before::Device(x) => serde_json::to_string(x),
            Before::Link(x) => serde_json::to_string(x),
            Before::Wall(x) => serde_json::to_string(x),
            Before::Point(x) => serde_json::to_string(x),
            Before::Label(x) => serde_json::to_string(x),
        }
        .ok()?;
        Some((b, json))
    }

    fn load(&mut self, mut n: Network) {
        n.sanitize();
        let p = &self.net;
        if !(p.devices.is_empty() && p.walls.is_empty() && p.labels.is_empty() && p.points.is_empty() && p.plans.is_empty()) {
            self.snapshot();
        }
        self.net = n;
        self.saved_hash = plan_hash(&self.net);
        // An untouched plan isn't worth an automatic save: the recovery copy keeps the last plan that was worked on.
        self.autosave_last = serde_json::to_string(&self.net).unwrap_or_default();
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

    /// The latest message, at the bottom middle of the map so it never hides what's being worked on.
    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, until)) = self.toast.clone() else { return };
        if Self::now(ctx) >= until {
            self.toast = None;
            return;
        }
        let c = if self.canvas_rect.is_positive() && self.mode != Mode::Learn { self.canvas_rect } else { ctx.content_rect() };
        // Above the Wi-Fi map's card when it's up.
        let lift = if self.heat && self.mode != Mode::Learn { if c.width() < 810.0 { 190.0 } else { 130.0 } } else { 64.0 };
        egui::Area::new(Id::new("toast")).fixed_pos(pos2(c.center().x, c.bottom() - lift)).pivot(Align2::CENTER_BOTTOM).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::new().fill(pal().card).stroke(Stroke::new(1.0, pal().line)).corner_radius(10).inner_margin(Margin::symmetric(14, 9)).shadow(egui::Shadow { offset: [0, 3], blur: 14, spread: 0, color: Color32::from_black_alpha(70) }).show(ui, |ui| {
                ui.set_max_width(560.0f32.min(c.width() - 60.0).max(200.0));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ic::INFO).color(pal().accent));
                    ui.label(msg);
                });
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(300));
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
                self.say(ctx, trf("Saved {}", &[&file_name(&path)]));
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
                self.say(ctx, if newer { trf("Opened {}. It was saved by a newer NetSketch: settings this version doesn't know are left out, and saving here drops them.", &[&file_name(&path)]) } else { trf("Opened {}", &[&file_name(&path)]) });
                self.file = Some(path);
            }
            Err(e) => self.say(ctx, trf("Couldn't open that file: {}", &[&e])),
        }
    }

    /// Replaces the open plan, first asking "Save your changes?" when it has unsaved ones.
    fn swap(&mut self, ctx: &egui::Context, s: Swap) {
        let n = &self.net;
        let empty = n.devices.is_empty() && n.walls.is_empty() && n.labels.is_empty() && n.points.is_empty() && n.plans.is_empty();
        if !empty && plan_hash(&self.net) != self.saved_hash {
            self.pending = Some(s);
        } else {
            self.do_swap(ctx, s);
        }
    }

    fn do_swap(&mut self, ctx: &egui::Context, s: Swap) {
        match s {
            Swap::Open => self.open(ctx),
            Swap::Path(p) => self.open_path(ctx, p),
            Swap::Plan(n, recovered) => {
                self.load(*n);
                self.file = None;
                if recovered {
                    // A recovered copy must not overwrite whatever file was open before, and it isn't saved yet.
                    self.saved_hash = 0;
                }
                if recovered || self.mode == Mode::Learn {
                    self.mode = Mode::Design;
                }
            }
        }
    }

    /// "Save your changes?" when the window is closed, or another plan opened, with unsaved changes.
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
                    self.pending = None;
                }
            });
        });
        if quit {
            self.ask_quit = false;
            if let Some(s) = self.pending.take() {
                self.do_swap(ctx, s);
            } else {
                self.quit_ok = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn export(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter(tr("Web page (opens in any browser, prints to PDF)"), &["html"]).set_file_name("network-plan.html").save_file() else { return };
        match std::fs::write(&path, report::html(&self.net)) {
            Ok(()) => self.say(ctx, trf("Exported {}. Open it in a browser; Print → Save as PDF makes a PDF.", &[&file_name(&path)])),
            Err(e) => self.say(ctx, trf("Couldn't export: {}", &[&e])),
        }
    }

    /// The cable schedule and the shopping list as CSV files, side by side: "<name>-cables.csv", "<name>-shopping.csv".
    fn export_csv(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).set_file_name("network.csv").save_file() else { return };
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "network".into());
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let (a, b) = (dir.join(format!("{stem}-cables.csv")), dir.join(format!("{stem}-shopping.csv")));
        match std::fs::write(&a, report::cables_csv(&self.net)).and_then(|_| std::fs::write(&b, report::shopping_csv(&self.net))) {
            Ok(()) => self.say(ctx, trf("Exported {} and {}.", &[&file_name(&a), &file_name(&b)])),
            Err(e) => self.say(ctx, trf("Couldn't export: {}", &[&e])),
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
                    painter.circle_filled(pos2(x, y), 1.0, pal().line);
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

        // Time since the dots last moved (capped, so a pause doesn't make them leap).
        let now_t = ui.input(|i| i.time);
        let flow_dt = (now_t - self.flow_t).clamp(0.0, 0.1) as f32;
        self.flow_t = now_t;
        // Links. With a device selected or under the pointer its own links stand out and the rest step back;
        // otherwise Wi-Fi links stay thin and quiet so the cables read.
        let focus_dev = match self.sel {
            Sel::Dev(id) => Some(id),
            _ => pointer.and_then(|p| self.net.devices.iter().find(|d| d.floor == self.floor && Rect::from_center_size(self.dev_pos(d.id), Vec2::splat(58.0 * z)).contains(p)).map(|d| d.id)),
        };
        let mut hovered_link = None;
        for l in &self.net.links {
            let (a, b) = (self.dev_pos(l.a), self.dev_pos(l.b));
            // A drawn cable route: through its points.
            let pts: Vec<Pos2> = std::iter::once(a).chain(l.path.iter().map(|p| self.to_screen(vec2(p[0], p[1])))).chain(std::iter::once(b)).collect();
            let selected = self.sel == Sel::Link(l.id);
            // Cables entirely on other storeys are faded like their devices and can't be picked.
            let elsewhere = [l.a, l.b].iter().all(|x| self.net.dev(*x).is_some_and(|d| d.floor != self.floor));
            let mine = focus_dev.is_some_and(|f| l.a == f || l.b == f);
            let painter = {
                let mut p = painter.clone();
                if elsewhere {
                    p.multiply_opacity(0.3);
                }
                if focus_dev.is_some() && !mine && self.sel != Sel::Link(l.id) {
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
                let (w, a) = if mine || selected || near { (w, 0.9) } else { (w * 0.6, 0.55) };
                painter.extend(Shape::dashed_line(&pts, Stroke::new(w, col.gamma_multiply(a)), 6.0 * z, 5.0 * z));
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
                let zz = z.max(0.6);
                for (now, forward) in [(*to_b, true), (*to_a, false)] {
                    if total < 1.0 {
                        continue;
                    }
                    // All dots of a stream move together, and the rate behind speed, spacing and size glides to a new
                    // value over about half a second, so a change shows along the whole line at once. Spacing comes
                    // from a lattice: every DOT_BASE one dot, then dots halfway between, then quarter-way…; more
                    // traffic switches on finer levels, the newest level fading in, so nothing jumps or slides back.
                    let st = self.flow.entry((l.id, forward)).or_insert(Stream { shown: now, phase: 0.0 });
                    st.shown += (now - st.shown) * (1.0 - (-flow_dt / 0.5).exp());
                    let lg = (1.0 + st.shown).log10();
                    st.phase = (st.phase + (25.0 + 25.0 * lg) * flow_dt / DOT_BASE).fract();
                    if st.shown < 0.05 {
                        continue;
                    }
                    let gap = (90.0 / (1.0 + st.shown).powf(0.45)).clamp(5.0, 120.0);
                    let levels = (DOT_BASE / gap).log2().max(0.0);
                    let r = (1.3 + 0.7 * lg).min(3.6) * zz;
                    let base = DOT_BASE * zz;
                    for level in 0..=levels.ceil() as u32 {
                        let alpha = if level == 0 || (level as f32) <= levels { 1.0 } else { levels.fract() };
                        let col = c.gamma_multiply(alpha);
                        // Where this level's dots sit within one base spacing.
                        let offs: Vec<f32> = if level == 0 { vec![0.0] } else { (0..1u32 << (level - 1)).map(|m| (2 * m + 1) as f32 / (1u32 << level) as f32).collect() };
                        for off in offs {
                            let mut d = (st.phase + off).fract() * base;
                            while d < total {
                                let (mut rest, mut i) = (if forward { d } else { total - d }, 0);
                                while i + 1 < lens.len() && rest > lens[i] {
                                    rest -= lens[i];
                                    i += 1;
                                }
                                let dir = (pts[i + 1] - pts[i]) / lens[i].max(0.01);
                                let side = if forward { 1.0 } else { -1.0 };
                                painter.circle_filled(pts[i] + dir * rest + dir.rot90() * side * (1.0 * zz + r), r, col);
                                d += base;
                            }
                        }
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
                    (tr("no power").to_string(), pal().dim)
                } else {
                    (tr("standby").to_string(), pal().dim)
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
        // Network gear (what everything hangs off) is drawn a little larger.
        let gear = |k: Kind| k.radio() || matches!(k, Kind::Router | Kind::Switch);
        let size_of = |k: Kind| if gear(k) && !k.foreign() { size * 1.15 } else { size };
        for id in &ids {
            let c = self.dev_pos(*id);
            let r = Rect::from_center_size(c, Vec2::splat(self.net.dev(*id).map_or(size, |d| size_of(d.kind))));
            let r_resp = ui.interact(r, Id::new(("dev", *id)), Sense::click_and_drag());
            if r_resp.clicked() {
                clicked_dev = Some(*id);
            }
            if r_resp.drag_started() && (self.tool == Tool::Connect && self.mode == Mode::Design) {
                self.connect_from = Some(*id);
            }
            if r_resp.drag_stopped() && self.snap && !self.guide_hit && self.extra.is_empty() && self.connect_from.is_none() && self.mode == Mode::Design {
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
                // Part of a bigger selection: everything selected moves along.
                let group = self.mode == Mode::Design && !self.extra.is_empty() && self.selection().contains(&Sel::Dev(*id));
                if group {
                    self.move_selection([delta.x, delta.y]);
                } else {
                    if let Some(d) = self.net.dev_mut(*id) {
                        d.pos[0] += delta.x;
                        d.pos[1] += delta.y;
                    }
                    self.sel = Sel::Dev(*id);
                    self.guide_hit = self.mode == Mode::Design && self.snap && self.align_guides(*id, &painter);
                }
                self.walks.remove(id);
                if self.mode == Mode::Simulate {
                    let ctx = ui.ctx().clone();
                    self.after_move(&ctx, *id);
                }
            }
            // Double-click: sit down at the device (its desktop).
            if r_resp.double_clicked() {
                self.open_desk(*id, ui_desk::Page::Home);
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
        let mut taken: Vec<Rect> = self.net.devices.iter().filter(|d| d.floor == self.floor).map(|d| Rect::from_center_size(self.dev_pos(d.id), Vec2::splat(size_of(d.kind)))).collect();
        // Each label sits on a soft backing so walls and cables never run through the text.
        let mut place = |painter: &egui::Painter, at: Pos2, text: String, font: FontId, col: Color32, force: bool| {
            let g = painter.layout_no_wrap(text, font, col);
            let r = Rect::from_center_size(at, g.size());
            if force || !taken.iter().any(|t| t.intersects(r.shrink(1.0))) {
                taken.push(r);
                painter.rect_filled(r.expand2(vec2(3.0, 0.5)), 4, pal().bg.gamma_multiply(0.72));
                painter.galley(r.min, g, col);
            }
        };
        // Network gear first: when labels are short of room, theirs win.
        let mut order: Vec<&net::Device> = self.net.devices.iter().collect();
        order.sort_by_key(|d| !gear(d.kind));
        for d in order {
            // Devices on other storeys stay visible but faded, like a see-through floor plan.
            let painter = {
                let mut p = painter.clone();
                if d.floor != self.floor {
                    p.multiply_opacity(0.3);
                }
                p
            };
            let c = self.dev_pos(d.id);
            let size = size_of(d.kind);
            let r = Rect::from_center_size(c, Vec2::splat(size));
            let col = kind_color(d.kind);
            let is_gear = gear(d.kind) && !d.kind.foreign();
            if d.floor != self.floor {
                painter.text(r.center_top() + vec2(0.0, -2.0 * z), Align2::CENTER_BOTTOM, net::floor_name(d.floor), FontId::proportional(10.0 * z), pal().text);
            }
            let selected = self.sel == Sel::Dev(d.id) || self.extra.contains(&Sel::Dev(d.id));
            let hovered = pointer.is_some_and(|p| r.contains(p));
            if selected {
                painter.rect_filled(r.expand(6.0 * z), 18.0 * z, pal().accent.gamma_multiply(0.18));
            }
            let fill = if d.kind.foreign() { pal().card.gamma_multiply(0.6) } else { pal().card };
            painter.rect_filled(r, 14.0 * z, fill);
            if is_gear {
                painter.rect_filled(r, 14.0 * z, pal().accent.gamma_multiply(0.07));
            }
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
            let edge = if selected || hovered { col } else if is_gear { pal().accent.gamma_multiply(0.55) } else { col.gamma_multiply(0.35) };
            painter.rect_stroke(r, 14.0 * z, Stroke::new(if selected { 2.0 } else { 1.2 }, edge), StrokeKind::Inside);
            if self.walks.contains_key(&d.id) {
                painter.text(r.left_top() + vec2(2.0, 2.0) * z, Align2::LEFT_TOP, ic::PERSON_SIMPLE_WALK, FontId::proportional(14.0 * z), pal().accent);
            }
            painter.text(c, Align2::CENTER_CENTER, kind_icon(d.kind), FontId::proportional(28.0 * z), col);
            // A dot only when something's wrong (red: a problem, amber: a warning).
            let status = match worst.get(&d.id) {
                Some(Level::Error) => Some(pal().red),
                Some(Level::Warning) => Some(pal().amber),
                _ => None,
            };
            if let Some(st) = status {
                let at = r.right_top() + vec2(-5.0, 5.0) * z;
                painter.circle_filled(at, 5.5 * z.max(0.7), pal().bg);
                painter.circle_filled(at, 4.0 * z.max(0.7), st);
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
            // Network gear keeps its name further out; the second line (address, Wi-Fi name) waits until zoomed in,
            // except in Simulate where it's the live signal and speed.
            let focus = selected || hovered;
            let names = self.zoom >= 0.45 || focus || (is_gear && self.zoom >= 0.18);
            let subs = focus || if self.mode == Mode::Simulate { self.zoom >= 0.65 } else { self.zoom >= 0.9 };
            if names {
                place(&painter, c + vec2(0.0, size * 0.5 + 9.0 * z.max(0.7)), d.name.clone(), FontId::proportional((12.5 * z).max(10.0)), pal().text, focus);
            }
            let ip = d.ifaces.get(if d.kind == Kind::HomeRouter { 0 } else { d.dhcp_iface() }).map(|f| f.ip.clone()).unwrap_or_default();
            let chans = |d: &net::Device| wifi::Band::ALL.iter().filter(|b| d.bands[b.i()]).map(|b| self.net.channel(d, *b).to_string()).collect::<Vec<_>>().join("/");
            let wifi_sig = self.net.wifi_link_of(d.id).and_then(|l| self.net.link_signal(l));
            let (sub, font, sub_col) = if d.kind.transmits() && (self.mode == Mode::Design || d.kind == Kind::Neighbor) {
                let what = if d.kind == Kind::Interference { format!("{} ch {}", ic::LIGHTNING, d.ch[0]) } else { format!("{} {} · ch {}", ic::WIFI_HIGH, d.ssid, chans(d)) };
                (what, FontId::proportional(11.0 * z), pal().dim)
            } else if let Some(o) = d.own_net.as_ref().filter(|_| self.mode == Mode::Design) {
                // A printer's or TV's own network (Wi-Fi Direct).
                (format!("{} {} · ch {}", ic::WIFI_HIGH, o.ssid, o.ch), FontId::proportional(11.0 * z), pal().dim)
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
        // Shift+click adds to the selection (or takes out); a plain click selects just that.
        let shift = ui.input(|i| i.modifiers.shift) && self.mode == Mode::Design;
        if let Some(id) = clicked_dev.filter(|_| !drawing) {
            if self.tool == Tool::Connect && self.mode == Mode::Design {
                match self.sel {
                    Sel::Dev(prev) if prev != id => {
                        self.try_connect(ui.ctx(), prev, id);
                    }
                    _ => self.sel = Sel::Dev(id),
                }
            } else if shift {
                self.toggle_sel(Sel::Dev(id));
            } else {
                self.sel = Sel::Dev(id);
                self.extra.clear();
            }
        } else if resp.clicked() && !drawing {
            let hit = hovered_link.map(Sel::Link).or(hovered_wall.filter(|_| self.mode == Mode::Design).map(Sel::Wall));
            match (shift, hit) {
                (true, Some(h)) => self.toggle_sel(h),
                (true, None) => {}
                (false, h) => {
                    self.sel = h.unwrap_or(Sel::None);
                    self.extra.clear();
                }
            }
        }
        // Right-click: a menu for the wall or cable under the pointer, or for the empty spot (the wall tool uses right-click to stop).
        if !drawing {
            if resp.secondary_clicked() {
                let at = pointer.map(|p| self.to_world(p)).unwrap_or_default();
                self.ctx_at = Some((hovered_wall.filter(|_| hovered_link.is_none()), hovered_link, [at.x, at.y]));
            }
            resp.context_menu(|ui| self.map_menu(ui));
        }
        // Shift+drag on empty map draws a selection box; any other drag pans (middle button or Space+drag too,
        // even while drawing walls).
        if resp.drag_started_by(egui::PointerButton::Primary) && shift && !drawing && clicked_dev.is_none() {
            self.marquee = pointer.map(|p| self.to_world(p));
        }
        if let Some(a) = self.marquee {
            if let Some(p) = pointer {
                let r = Rect::from_two_pos(self.to_screen(a), p);
                painter.rect(r, 2, pal().accent.gamma_multiply(0.08), Stroke::new(1.0, pal().accent), StrokeKind::Inside);
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                if let Some(p) = pointer.or(ui.input(|i| i.pointer.latest_pos())) {
                    let b = self.to_world(p);
                    self.select_box(a, b, true);
                }
                self.marquee = None;
            }
        } else if resp.dragged() && (!drawing || !resp.dragged_by(egui::PointerButton::Primary) || ui.input(|i| i.key_down(egui::Key::Space))) {
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
            self.heat_card(ui.ctx(), rect);
        }
        // Zoom controls, bottom right.
        ui_kit::float(ui.ctx(), "zoom", rect.right_bottom() + vec2(-12.0, -12.0), Align2::RIGHT_BOTTOM, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if ui.add(egui::Button::new(ic::MINUS).frame(false)).on_hover_text(tr("Zoom out")).clicked() {
                    self.zoom = (self.zoom / 1.2).max(0.1);
                }
                ui.label(RichText::new(format!("{:.0}%", self.zoom * 100.0)).small().color(pal().dim)).on_hover_text(tr("Zoom level"));
                if ui.add(egui::Button::new(ic::PLUS).frame(false)).on_hover_text(tr("Zoom in")).clicked() {
                    self.zoom = (self.zoom * 1.2).min(MAX_ZOOM);
                }
                ui.label(RichText::new(trf("· dots {} m", &[&(self.grid_m())])).small().color(pal().dim)).on_hover_text(tr("Distance between two dots of the grid"));
                if ui.add(egui::Button::new(ic::CORNERS_OUT).frame(false)).on_hover_text(tr("Fit everything")).clicked() {
                    self.fit();
                }
                if ui.add(egui::Button::new(ic::CAMERA).frame(false)).on_hover_text(tr("Save the map as a picture (PNG)")).clicked() {
                    let ctx = ui.ctx().clone();
                    self.export_picture(&ctx);
                }
            });
        });
        if self.mode == Mode::Design {
            self.dock(ui.ctx(), rect);
            // An empty storey: three ways to start, in the middle of the map.
            let f = self.floor;
            let blank = !self.net.devices.iter().any(|d| d.floor == f) && !self.net.walls.iter().any(|w| w.floor == f) && !self.net.plans.iter().any(|p| p.floor == f);
            if blank && self.tool == Tool::Select && self.drag_kind.is_none() {
                self.start_card(ui.ctx(), rect);
            }
        } else if self.sel == Sel::None {
            ui_kit::float(ui.ctx(), "sim_hint", rect.left_top() + vec2(12.0, 12.0), Align2::LEFT_TOP, |ui| {
                egui::Frame::new().inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                    ui.label(RichText::new(format!("{}  {}", ic::PLAY, tr("Click any device to use its apps"))).color(pal().accent));
                });
            });
        }
        self.map_top_right(ui.ctx(), rect);
        self.load_cache = load;
    }

    /// The card on an empty storey: start from a floor-plan picture, by drawing walls, or with the router.
    fn start_card(&mut self, ctx: &egui::Context, rect: Rect) {
        ui_kit::float(ctx, "start", rect.center(), Align2::CENTER_CENTER, |ui| {
            egui::Frame::new().inner_margin(Margin::same(14)).show(ui, |ui| {
                ui.set_max_width(320.0);
                ui.label(RichText::new(trf("{} is empty", &[&self.net.storey(self.floor)])).size(16.0).strong());
                ui.label(RichText::new(tr("Start with the walls (from a picture of the floor plan, or drawn by hand), then add the router and your devices.")).color(pal().dim));
                ui.add_space(8.0);
                let full = |ui: &mut egui::Ui, t: String| ui.add_sized([ui.available_width(), 30.0], egui::Button::new(t));
                if full(ui, format!("{}  {}", ic::IMAGE, tr("Choose a floor-plan picture…"))).on_hover_text(tr("PNG or JPEG of this storey's plan, e.g. a scan or a screenshot of the architect's PDF")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter(tr("Picture"), &["png", "jpg", "jpeg"]).pick_file() {
                        self.add_picture(ctx, &path);
                    }
                }
                if full(ui, format!("{}  {}", ic::WALL, tr("Draw the walls"))).on_hover_text(tr("Click corner after corner; right-click or Esc ends the wall")).clicked() {
                    self.tool = Tool::Wall;
                }
                if full(ui, format!("{}  {}", ic::WIFI_HIGH, tr("Add the router"))).clicked() {
                    self.snapshot();
                    let c = self.to_world(rect.center());
                    let id = self.net.add_device(Kind::HomeRouter, [c.x, c.y]);
                    self.net.dev_mut(id).unwrap().floor = self.floor;
                    self.sel = Sel::Dev(id);
                }
            });
        });
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
        self.steps += 1;
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
        self.steps += 1;
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
        if !self.extra.is_empty() {
            self.multi_panel(ui);
            return;
        }
        match self.sel {
            Sel::Dev(id) if self.net.dev(id).is_some() => {
                if self.mode == Mode::Simulate {
                    self.device_sim(ui, id, issues);
                } else {
                    self.device_editor(ui, id, issues);
                }
            }
            Sel::Link(id) if self.net.links.iter().any(|l| l.id == id) => self.link_editor(ui, id),
            Sel::Wall(id) if self.net.walls.iter().any(|w| w.id == id) => self.wall_editor(ui, id),
            Sel::Point(id) if self.net.points.iter().any(|p| p.id == id) => self.point_editor(ui, id),
            Sel::Label(id) if self.net.labels.iter().any(|l| l.id == id) => self.label_editor(ui, id),
            _ => {}
        }
    }

    fn header(ui: &mut egui::Ui, icon: &str, col: Color32, title: &str, sub: &str) {
        ui.horizontal(|ui| {
            ui_kit::icon_tile(ui, icon, col, 38.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).size(15.0).strong());
                ui.label(RichText::new(tr(sub)).small().color(pal().dim));
            });
        });
    }

    /// The × at the top right of the side panel: closes it by deselecting.
    fn close_button(&mut self, ui: &mut egui::Ui) {
        let r = Rect::from_min_size(ui.max_rect().right_top() + vec2(-26.0, 0.0), vec2(26.0, 26.0));
        if ui.put(r, egui::Button::new(RichText::new(ic::X).color(pal().dim)).frame(false)).on_hover_text(tr("Close (Esc)")).clicked() {
            self.sel = Sel::None;
        }
    }

    /// A fold-out section: a title row that opens and closes. Returns whether it's open.
    /// The open/closed choice is remembered per title, so it stays the same from device to device.
    fn section(ui: &mut egui::Ui, title: &str) -> bool {
        Self::section_hint(ui, title, "")
    }

    /// A section whose explanation sits behind an info icon instead of filling the panel.
    /// In a workspace (ui_shell::FLAT) sections are plain headed groups that are always open.
    fn section_hint(ui: &mut egui::Ui, title: &str, hint: &str) -> bool {
        if let Some(space) = ui_shell::FLAT.with(|f| f.get()) {
            if space != title {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr(title)).strong());
                    if !hint.is_empty() {
                        ui.label(RichText::new(ic::INFO).color(pal().dim)).on_hover_text(tr(hint));
                    }
                });
                ui.add_space(2.0);
            }
            return true;
        }
        // Inside a tab most sections are short enough to start open; rarely changed details start folded away.
        const CLOSED: &[&str] = &["Antenna", "Behaviour", "Spanning tree", "Model", "More network names", "Routes"];
        let id = Id::new(("fold", title));
        let mut open = ui.ctx().data_mut(|d| *d.get_persisted_mut_or(id, !CLOSED.contains(&title)));
        ui.add_space(4.0);
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

    /// Which tabs a device's panel has: only those with something in them.
    fn dev_tabs(&self, id: u32) -> Vec<(DevTab, String)> {
        let d = self.net.dev(id).unwrap();
        let k = d.kind;
        let wifi = k.transmits() || k.wifi_client() || matches!(k, Kind::SmartHome | Kind::HomeRouter | Kind::Speaker);
        let addresses = !d.ifaces.is_empty() && !k.bridges() && !d.on_802154();
        let network = addresses || k == Kind::Router || k.serves() || k.routes() || (!k.foreign() && k != Kind::Internet) || k.bridges();
        let mut v = vec![(DevTab::General, tr("General").to_string())];
        if wifi {
            v.push((DevTab::WiFi, tr("Wi-Fi").to_string()));
        }
        if network {
            v.push((DevTab::Network, tr("Network").to_string()));
        }
        if matches!(k, Kind::HomeRouter | Kind::Router) {
            v.push((DevTab::Security, tr("Security").to_string()));
        }
        if !k.foreign() && k != Kind::Internet {
            v.push((DevTab::Use, tr("Power and use").to_string()));
        }
        v
    }

    fn device_editor(&mut self, ui: &mut egui::Ui, id: u32, issues: &[Issue]) {
        let mut desk_req: Option<ui_desk::Page> = None;
        let currency = self.net.currency.clone();
        let mut delete = false;
        let mut renew = false;
        let self_wired = self.net.links.iter().any(|l| (l.a == id || l.b == id) && !l.medium.wireless());
        let tabs = self.dev_tabs(id);
        if !tabs.iter().any(|t| t.0 == self.dev_tab) {
            self.dev_tab = DevTab::General;
        }
        {
            // Header: icon, editable name, kind and address.
            let ip = self.net.dev(id).and_then(|d| d.first_ip().map(|i| i.to_string()));
            let d = self.net.dev_mut(id).unwrap();
            let host = (d.kind.is_host() || d.kind == Kind::HomeRouter).then(|| trf("On the network as {}.home.arpa", &[&(d.hostname())]));
            ui.horizontal(|ui| {
                ui_kit::icon_tile(ui, kind_icon(d.kind), kind_color(d.kind), 38.0).on_hover_text(d.kind.blurb());
                ui.vertical(|ui| {
                    let name = ui.add(TextEdit::singleline(&mut d.name).font(FontId::proportional(15.0)).desired_width(ui.available_width() - 30.0));
                    if let Some(h) = &host {
                        name.on_hover_text(h);
                    }
                    let sub = match (&ip, d.model.is_empty()) {
                        (Some(ip), true) => format!("{} · {ip}", d.kind.label()),
                        (Some(ip), false) => format!("{} · {ip}", d.model),
                        (None, true) => d.kind.label().to_string(),
                        (None, false) => format!("{} · {}", d.kind.label(), d.model),
                    };
                    ui.label(RichText::new(sub).small().color(pal().dim));
                });
            });
        }
        // Actions, then this device's problems: the things you came for, before the details.
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if ui.button(format!("{}  {}", ic::PLAY, tr("Try it"))).on_hover_text(tr("Switches to Simulate with this device picked: use its browser, command line and Wi-Fi")).clicked() {
                self.mode = Mode::Simulate;
                let ctx = ui.ctx().clone();
                self.start_sim(&ctx);
            }
            self.power_toggle(ui, id);
            ui.menu_button(ic::DOTS_THREE, |ui| {
                if ui.button(format!("{}  {}", ic::COPY, tr("Duplicate"))).on_hover_text(tr("A copy next to it, same hardware, fresh addresses (Ctrl+D)")).clicked() {
                    self.snapshot();
                    if let Some(new) = self.net.duplicate(id) {
                        self.sel = Sel::Dev(new);
                    }
                    ui.close();
                }
                if ui.button(RichText::new(format!("{}  {}", ic::TRASH, tr("Delete"))).color(pal().red)).on_hover_text(tr("Remove this device (Delete key)")).clicked() {
                    delete = true;
                    ui.close();
                }
            })
            .response
            .on_hover_text(tr("More: duplicate, delete"));
        });
        // Problems as banners; tips folded under one line so they don't push the settings away.
        let mine: Vec<&Issue> = issues.iter().filter(|i| i.dev == Some(id)).collect();
        let tips = mine.iter().filter(|i| i.level == Level::Tip).count();
        let mut fix = None;
        for i in mine.iter().filter(|i| i.level != Level::Tip) {
            ui.add_space(2.0);
            if let (_, Some(f)) = ui_kit::banner(ui, level_color(i.level), |ui| issue_row(ui, i)) {
                fix = Some(f);
            }
        }
        if tips > 0 {
            egui::CollapsingHeader::new(RichText::new(trf("{}  Tips for this device ({})", &[&(ic::LIGHTBULB), &tips])).color(pal().accent)).id_salt("devtips").show(ui, |ui| {
                for i in mine.iter().filter(|i| i.level == Level::Tip) {
                    if let (_, Some(f)) = issue_row(ui, i) {
                        fix = Some(f);
                    }
                }
            });
        }
        if let Some(f) = fix {
            self.snapshot();
            let msg = self.net.apply_fix(&f);
            let ctx = ui.ctx().clone();
            self.say(&ctx, msg);
        }
        ui.add_space(4.0);
        ui_kit::tabs(ui, &mut self.dev_tab, &tabs);
        let tab = self.dev_tab;
        let plugged = self.net.port_users(id);
        {
            // On the mobile line the router's own address comes from the provider's shared range.
            let mobile = self.net.dev(id).and_then(|d| self.net.cgnat(d));
            let d = self.net.dev_mut(id).unwrap();
            let kind = d.kind;
            // Thread and Zigbee gadgets get no address from DHCP (their hub speaks for them; see Smart-home radio).
            if tab == DevTab::Network && !d.ifaces.is_empty() && !kind.bridges() && !d.on_802154() && Self::section(ui, "Addresses") {
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
            if tab == DevTab::Network && kind == Kind::Router && d.ifaces.is_empty() && Self::section(ui, "Addresses") {
                ui.label(RichText::new(tr("Connect cables first: each cable uses one of the router's ports, which then gets its own address.")).small().color(pal().dim));
            }
            if tab == DevTab::General && kind == Kind::Internet && Self::section(ui, "Internet plan") {
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
            if tab == DevTab::Network && crate::soft::has_desktop(kind) && Self::section(ui, "Software") {
                // What's installed; servers can be started and stopped right here, everything else is on its desktop.
                use crate::soft::Soft;
                ui.horizontal_wrapped(|ui| {
                    for s in Soft::ALL.into_iter().filter(|s| d.has(*s) && !s.is_server()) {
                        ui.label(RichText::new(format!("{}  {}", s.icon(), tr(s.label()))).small().color(pal().dim)).on_hover_text(tr(s.blurb()));
                    }
                });
                let servers: Vec<Soft> = Soft::ALL.into_iter().filter(|s| d.has(*s) && s.is_server()).collect();
                for s in servers {
                    let mut on = d.running(s);
                    if ui.checkbox(&mut on, format!("{}  {}", s.icon(), tr(s.label()))).on_hover_text(tr(s.blurb())).changed() {
                        d.set_running(s, on);
                    }
                    if s == Soft::DhcpServer && on {
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
                    if s == Soft::DnsServer && on && kind != Kind::Internet {
                        ui.add(TextEdit::multiline(&mut d.dns_records).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(tr("www.home.test 192.168.1.10"))).on_hover_text(tr("Name records, one per line: name address. A mail server for a domain: school.test MX mail.school.test"));
                    }
                    if s == Soft::WebServer && on && ui.small_button(format!("{}  {}", ic::FILE_HTML, tr("Edit pages…"))).on_hover_text(tr("The pages are files in its webserver folder: opens them on its desktop")).clicked() {
                        desk_req = Some(ui_desk::Page::App(Soft::WebServer));
                    }
                }
                if ui.button(format!("{}  {}", ic::PACKAGE, tr("Install or remove software…"))).on_hover_text(tr("Opens this device's desktop at its software list")).clicked() {
                    desk_req = Some(ui_desk::Page::Installer);
                }
            }
            if tab == DevTab::Network && kind.routes() && kind != Kind::Internet && Self::section(ui, "Routes") {
                let was = d.services.rip;
                ui.checkbox(&mut d.services.rip, tr("RIP: share routes with neighbour routers")).on_hover_text(tr("Routers with RIP on tell each other which networks they reach and fill their routing tables themselves, up to 15 routers away"));
                if d.services.rip && !was {
                    // Its routes spread one update (30 s) at a time from now on.
                    d.services.rip_since = Some(Self::now(ui.ctx()));
                }
                ui.add(TextEdit::multiline(&mut d.routes).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace).hint_text(tr("10.0.2.0/24 via 10.0.1.2"))).on_hover_text(tr("Extra routes, one per line: network/prefix via gateway"));
            }
            // The internet cloud is the outside world: no ports or storey to plan.
            if tab == DevTab::General && kind != Kind::Internet && Self::section(ui, "Hardware") {
                // Plain wired routers come from the catalog here (Wi-Fi gear does in "Wi-Fi hardware").
                if kind == Kind::Router {
                    if let Some(old) = radio_ui::catalog_picker(ui, d, &mut self.catalog_query) {
                        self.before = Some(Before::Device(old));
                        self.replug = Some(id);
                    }
                }
                egui::Grid::new(("hw", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    ui.label(tr("Storey"));
                    ui.add(egui::DragValue::new(&mut d.floor).range(-3..=20).custom_formatter(|v, _| net::floor_name(v as i8)));
                    ui.end_row();
                    if kind.transmits() || kind.wifi_client() {
                        ui.label(tr("Mounted at"));
                        ui.add(egui::DragValue::new(&mut d.mount).range(0.0..=10.0).speed(0.05).max_decimals(2).suffix(tr(" m above the floor"))).on_hover_text(tr("Antenna height: desks and hands about 1 m, a ceiling access point 2.5 m"));
                        ui.end_row();
                    }
                });
            }
            if tab == DevTab::Use && !kind.foreign() && kind != Kind::Internet && Self::section(ui, "Power") {
                egui::Grid::new(("pwr", id)).num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                    ui.label(tr("Power draw"));
                    ui.add(egui::DragValue::new(&mut d.watts).range(0.0..=2000.0).speed(0.5).suffix(tr(" W average"))).on_hover_text(tr("Average over a day, standby included. Devices powered over the cable count at their switch."));
                    ui.end_row();
                    ui.label(tr("UPS battery"));
                    ui.add(egui::DragValue::new(&mut d.ups_wh).range(0.0..=5000.0).speed(1.0).suffix(" Wh")).on_hover_text(tr("Plugged into an uninterruptible power supply with this much battery (0 = none). It also carries what this device powers over its cables."));
                    ui.end_row();
                });
            }
            // Phones and gadgets have no socket; hosts have one, so just its speed.
            let socket = kind.is_host() && !matches!(kind, Kind::Phone | Kind::SmartHome);
            let ports = !kind.foreign() && !kind.is_host() && kind != Kind::Internet;
            if tab == DevTab::Network && (socket || ports) && Self::section(ui, "Ports") {
                if socket {
                    ui.horizontal(|ui| {
                        ui.label(tr("Ethernet"));
                        if d.ports.is_empty() {
                            d.ports.push(ports::PortGroup::rj45(1, 1000));
                        }
                        d.ports.truncate(1);
                        ui_extras::speed_combo(ui, ("pspeed", id), &mut d.ports[0].mbps);
                    });
                }
                if ports {
                    let old = d.clone();
                    if ui_extras::port_editor(ui, id, &mut d.ports, &plugged) {
                        self.before = Some(Before::Device(old));
                    }
                }
            }
            if tab == DevTab::General && Self::section(ui, "Price and notes") {
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
        if tab == DevTab::WiFi {
            if kind.transmits() {
                self.radio_editor(ui, id);
                if kind.radio() {
                    self.names_editor(ui, id);
                }
            } else if kind.wifi_client() {
                self.adapter_editor(ui, id);
            }
        }
        if tab == DevTab::Network {
            if kind == Kind::Router {
                self.vlan_editor(ui, id);
            }
            if kind.bridges() || kind == Kind::HomeRouter {
                let stp = self.stp();
                self.stp_editor(ui, id, stp.root, stp.cost.get(&id).copied());
            }
        }
        self.device_extras(ui, id);
        if let Some(p) = desk_req {
            self.open_desk(id, p);
        }
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

    /// A device in Simulate mode: its little desktop of apps.
    fn device_sim(&mut self, ui: &mut egui::Ui, id: u32, issues: &[Issue]) {
        let d = self.net.dev(id).unwrap();
        let sub = match d.ifaces.get(d.dhcp_iface()).map(|f| f.ip.as_str()).filter(|s| !s.is_empty()) {
            Some(ip) => ip.to_string(),
            // Switches, access points, speakers and disturbers have no address of their own here.
            None if !d.kind.is_host() && !d.kind.routes() => if d.model.is_empty() { d.kind.label().to_string() } else { d.model.clone() },
            None => tr("no address yet").into(),
        };
        Self::header(ui, kind_icon(d.kind), kind_color(d.kind), &d.name, &sub);
        self.power_toggle(ui, id);
        let d = self.net.dev(id).unwrap();
        // Can it reach the internet, how fast, and what holds it back; then its own problems.
        if (d.kind.is_host() || d.kind == Kind::SmartHome) && !d.off && self.net.devices.iter().any(|x| x.kind == Kind::Internet) {
            ui.add_space(4.0);
            match self.e2e.get(&id).cloned().or_else(|| self.net.internet_rate(id, &self.stp())) {
                Some((r, why)) => {
                    ui.label(RichText::new(format!("{}  {}", ic::GLOBE, trf("Reaches the internet at about {} Mbit/s", &[&format!("{r:.0}")]))).color(pal().green));
                    ui.label(RichText::new(trf("Slowest part: {}", &[&why])).small().color(pal().dim));
                }
                None => {
                    ui.label(RichText::new(format!("{}  {}", ic::GLOBE_X, tr("No way to the internet from here: check its connection, the cables on the way and whether everything is switched on"))).color(pal().red));
                }
            }
        }
        for i in issues.iter().filter(|i| i.dev == Some(id) && i.level != Level::Tip) {
            let col = if i.level == Level::Error { pal().red } else { pal().amber };
            ui_kit::banner(ui, col, |ui| ui.label(RichText::new(&i.text).small()));
        }
        let d = self.net.dev(id).unwrap();
        ui.add_space(4.0);
        // Only the apps this kind of device really has.
        let k = d.kind;
        let wifi = k.transmits() || self.net.wifi_link_of(id).is_some();
        let tabs: Vec<(SimTab, &str, &str)> = [
            (SimTab::WiFi, ic::WIFI_HIGH, "Wi-Fi", wifi),
            (SimTab::Ports, ic::PLUGS_CONNECTED, tr("Ports"), k.has_ports_view()),
            (SimTab::Traffic, ic::LIST_BULLETS, tr("Traffic"), !k.foreign()),
        ]
        .into_iter()
        .filter(|t| t.3)
        .map(|t| (t.0, t.1, t.2))
        .collect();
        if let Some(first) = tabs.first().filter(|_| !tabs.iter().any(|t| t.0 == self.sim_tab)) {
            self.sim_tab = first.0;
        }
        // The computer itself: its desktop with all its programs, or straight into the browser or command line.
        if crate::soft::has_desktop(k) {
            use crate::soft::Soft;
            let mut open = None;
            ui.horizontal_wrapped(|ui| {
                if ui.button(RichText::new(format!("{}  {}", ic::MONITOR, tr("Open desktop"))).strong()).on_hover_text(tr("Its screen, with every program it has (double-click a device does the same)")).clicked() {
                    open = Some(ui_desk::Page::Home);
                }
                for s in [Soft::Browser, Soft::Terminal, Soft::MailClient] {
                    if d.has(s) && ui.button(format!("{}  {}", s.icon(), tr(s.label()))).clicked() {
                        open = Some(ui_desk::Page::App(s));
                    }
                }
            });
            if let Some(p) = open {
                self.open_desk(id, p);
            }
            ui.add_space(4.0);
        }
        if tabs.is_empty() {
            return;
        }
        let items: Vec<(SimTab, String)> = tabs.iter().map(|(t, icon, label)| (*t, format!("{icon}  {}", tr(label)))).collect();
        ui_kit::tabs(ui, &mut self.sim_tab, &items);
        match self.sim_tab {
            SimTab::WiFi => self.wifi_tab(ui, id),
            SimTab::Ports => self.ports_tab(ui, id),
            SimTab::Traffic => {
                // What it runs (apps or a test load), then everything it sent and received.
                self.apps_picker(ui, id);
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
                            // Monospace has no icon font: a plain arrow (the monospace font has it).
                            ui.label(RichText::new(format!("{} → {}", r.log.src, r.log.dst)).monospace());
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
                    let b = egui::Button::new(RichText::new(format!("{:>2}  {title}  {mark}", i + 1)).color(if on { pal().accent } else { pal().text })).fill(if on { pal().card } else { Color32::TRANSPARENT }).stroke(Stroke::NONE).min_size(vec2(ui.available_width(), 30.0));
                    if ui.add(b).clicked() {
                        self.lesson = i;
                    }
                }
                ui.add_space(10.0);
                ui.label(RichText::new(tr("FIND THE FAULT")).small().color(pal().dim));
                for (key, goal) in scenarios::EXERCISES {
                    let label = scenarios::SCENARIOS.iter().find(|s| s.0 == *key).map_or(*key, |s| s.1);
                    if ui.add(egui::Button::new(format!("{}  {}", ic::TARGET, tr(label))).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).min_size(vec2(ui.available_width(), 28.0))).on_hover_text(tr(goal)).clicked() {
                        self.swap(&ui.ctx().clone(), Swap::Plan(Box::new(net::template(key)), false));
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
                            self.swap(&ui.ctx().clone(), Swap::Plan(Box::new(net::template(key)), false));
                        }
                    }
                });
            });
        });
    }
}

fn stat(ui: &mut egui::Ui, value: &str, label: &str, col: Color32, width: f32) -> egui::Response {
    egui::Frame::new().fill(pal().card).corner_radius(8).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.set_width(width);
        ui.vertical(|ui| {
            ui.label(RichText::new(value).size(18.0).strong().color(col));
            ui.label(RichText::new(tr(label)).small().color(pal().dim));
        });
    })
    .response
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
    // + 0.0: an empty sum is −0, which would print as "-0".
    let total = items.iter().map(|i| i.2).sum::<f32>() + 0.0;
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
        // Channels and power limits follow the plan's country.
        wifi::Region::set(self.net.region);
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
        // Arrow keys nudge what's selected by 25 cm (Shift: 1 m); a run of nudges is one undo step.
        if !typing && self.mode == Mode::Design && !matches!(self.sel, Sel::None | Sel::Link(_)) {
            let step = if ctx.input(|i| i.modifiers.shift) { PX_PER_M } else { PX_PER_M / 4.0 };
            let d = ctx.input(|i| [(egui::Key::ArrowLeft, [-step, 0.0]), (egui::Key::ArrowRight, [step, 0.0]), (egui::Key::ArrowUp, [0.0, -step]), (egui::Key::ArrowDown, [0.0, step])].into_iter().filter(|(k, _)| i.key_pressed(*k)).map(|x| x.1).next());
            if let Some(d) = d {
                let now = Self::now(&ctx);
                if now - self.nudged > 1.0 {
                    self.snapshot();
                }
                self.nudged = now;
                self.move_selection(d);
            }
        }
        if !typing && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.swap(&ctx, Swap::Open);
        }
        if !typing && self.mode == Mode::Design && self.sel != Sel::None && ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)) {
            self.snapshot();
            self.connect_from = None;
            self.say(&ctx, tr("Deleted — Ctrl+Z brings it back."));
            self.delete_selection();
        }
        // Copy, cut and paste (egui reports Ctrl+C/X/V on the map as these events).
        if !typing && self.mode == Mode::Design {
            let (copy, cut, paste) = ctx.input(|i| {
                let ev = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
                (ev(|e| matches!(e, egui::Event::Copy)), ev(|e| matches!(e, egui::Event::Cut)), ev(|e| matches!(e, egui::Event::Paste(_))) || (i.modifiers.command && i.key_pressed(egui::Key::V)))
            });
            if (copy || cut) && self.sel != Sel::None {
                self.clip = self.copy_selection();
                if cut {
                    self.snapshot();
                    self.delete_selection();
                }
                self.say(&ctx, if cut { tr("Cut — Ctrl+V puts it where the pointer is.") } else { tr("Copied — Ctrl+V puts it where the pointer is.") });
            }
            if paste {
                self.paste_clip(&ctx);
            }
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
        if !typing && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::N)) {
            self.swap(&ctx, Swap::Plan(Box::new(net::template("blank")), false));
        }
        if !typing && self.mode == Mode::Design && ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D)) {
            self.duplicate_selection();
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
                "netplan" | "json" | "fls" => self.swap(&ctx, Swap::Path(path)),
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
        if self.ask_quit || self.pending.is_some() {
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

        egui::Panel::top("top").frame(egui::Frame::new().fill(pal().panel).inner_margin(Margin::symmetric(12, 7)).stroke(Stroke::new(1.0, pal().line))).show(ui, |ui| self.top_bar(ui));
        egui::Panel::left("rail").exact_size(64.0).frame(egui::Frame::new().fill(pal().panel).inner_margin(Margin::symmetric(4, 6)).stroke(Stroke::new(1.0, pal().line))).show(ui, |ui| self.rail(ui, &issues));

        if self.mode == Mode::Learn {
            self.toast_ui(&ctx);
            self.learn(ui);
            return;
        }

        let side = egui::Frame::new().fill(pal().panel).inner_margin(Margin::same(14)).stroke(Stroke::new(1.0, pal().line));
        let screen = ctx.content_rect();
        // Narrow windows: while something is selected its panel wins and the workspace folds away (it comes back on deselect).
        let room = screen.width() >= 1250.0 || self.sel == Sel::None;
        if let Some(space) = self.space.filter(|_| room) {
            egui::Panel::left("space").default_size(320.0).size_range(260.0..=(screen.width() * 0.32).max(280.0)).frame(side).show(ui, |ui| self.workspace(ui, space, &issues));
        }
        // The side panel opens for what's selected; the map keeps at least ~60% of the window.
        if self.sel != Sel::None {
            egui::Panel::right("inspector").default_size((screen.width() * 0.28).clamp(320.0, 420.0)).size_range(300.0..=(screen.width() * 0.4).max(320.0)).frame(side).show(ui, |ui| {
                self.close_button(ui);
                // Edits made in the panel become undo steps; a burst on the same thing (typing, a slider) is one step.
                let before = (self.mode == Mode::Design).then(|| self.sel_copy()).flatten();
                let steps = self.steps;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.inspector(ui, &issues));
                if let Some((old, json)) = before {
                    // Buttons that took their own undo step (a fix, delete) are already covered.
                    if self.before.is_none() && self.steps == steps && self.sel_copy().is_some_and(|(_, now)| now != json) {
                        let t = Self::now(ui.ctx());
                        let merge = self.panel_edit.is_some_and(|(s, at, n)| s == self.sel && t - at < 1.5 && n == self.steps);
                        if !merge {
                            self.before = Some(old);
                        }
                        self.panel_edit = Some((self.sel, t, self.steps + u64::from(!merge)));
                    }
                }
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg)).show(ui, |ui| self.canvas(ui, &issues));
        self.toast_ui(&ctx);
        self.packet_windows(&ctx);
        self.measured_window_ui(&ctx);
        self.log_window_ui(&ctx);
        self.desk_ui(&ctx);
        self.tidy_selection();
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
