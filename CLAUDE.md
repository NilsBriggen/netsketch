# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

NetSketch: a from-scratch modernization of the Filius network-teaching app, rewritten in Rust + egui (eframe). The goal was to modernize the outdated tool and turn it into versatile software that also plans real home networks: Wi-Fi coverage, walls/floors, measurements, cost, power. There's also an Android companion app for on-site Wi-Fi surveys.

**Clean room:** nothing inside `/usr/share/filius/filius.jar` may be read: its files, folder and code. Work from the person's description and reference pictures only. `fls.rs` imports old `.fls` projects from the *saved file format* only (checked against a real Filius 2.4.1 save), not from the jar.

Locally this repository sits in a Craft Anything project folder (`../`). Its files there (`craft.json`, `feed.json`, `checks/`, `investigation.cany/`, `build-activity.log`, `usage.json`, reference pictures) are the build tool's state, not part of the repo. Don't hand-edit them.

## Commands

```bash
cargo run                                  # the desktop app
cargo test                                 # all unit tests (fast, no GUI)
cargo test fix_all_repairs                 # one test by name substring
cargo run -- --screenshot shots/now.png    # renders ~120 frames, saves a PNG, exits
```

The `--screenshot` mode is what the Craft Anything build checks use. It also turns autosave off.

Android companion (`companion-android/`): `./build.sh` needs JDK 17+ and the Android SDK (platform 35, build-tools). It uses no Gradle: javac + aapt2 + d8 + apksigner. Its first step compiles `Survey.java`/`Link.java` with `test/SurveyJsonTest.java` on the host and **regenerates `sample.netsurvey`**. The Rust test in `companion.rs` reads that file with `include_str!`, so a change to the phone's file format must be checked from both sides.

## Architecture

The project is one binary crate, `src/`. The design has two halves:

- **Model (`Network`, in `net.rs`):** a plain, serde-serializable struct: devices, links, walls, floor plans, survey points, labels and settings. Its behaviour is split across files by topic, each adding its own `impl Network` block: `net.rs` (addressing, DHCP, problem checks, packet simulation, the terminal `exec`), `wifi.rs` (signal/path-loss model, channels, roaming, hardware presets), `stp.rs` (spanning tree), `issues.rs` (more checks plus one-click `Fix`es via `apply_fix`), `plan.rs` (coverage score, AP suggestion, `fix_all`), `extras.rs` (PoE, firewall, IPv6, guest VLAN, app traffic, device library), `more.rs` (resilience, power, time of day), `companion.rs` (phone measurements → pins/neighbours), `phone_link.rs` (live phone link). These files have no egui dependency and carry all the tests.
- **UI (`App`, in `main.rs`):** one egui app with modes Design / Simulate / Learn. It is likewise split into `impl App` blocks in `radio_ui.rs`, `ui_draw.rs`, `ui_extras.rs`, `ui_more.rs` and `ui_companion.rs`, which pull in `main.rs`'s imports, colour constants and types with `use super::*`.

Key cross-file facts:

- **Problem list:** `Network::check()` (net.rs) calls `check_more()` (issues.rs) and produces `Issue`s, optionally carrying a `Fix`. The UI caches the result in `App.issues`. Any edit that changes the plan must reset it (`snapshot()` does this, and also pushes the undo state, capped at 40).
- **Save format:** `.netplan` is `serde_json` of `Network`. `#[serde(default)]` on the structs keeps older files loading, so new fields need a sensible `Default`. Runtime-only fields use `#[serde(skip)]`. Loaded files go through `Network::sanitize()`. An autosave goes every 30 s to `$XDG_CONFIG_HOME` (see `extras::library_path`), next to the device-model library.
- **Templates:** `net::template(key)` / `template_seeded(key, seed)` build the ready-made plans. The scenario keys and menu text are in `scenarios.rs` (`SCENARIOS`, `build`). Tests usually start from a template, e.g. `template("flat")`.
- **Reproducible randomness:** each plan has a `seed`. `sim.rs` derives MAC addresses, fading and traffic noise from it with a hash, so use `sim::hash` instead of a global RNG.
- **Units:** canvas pixels map to metres via `net::PX_PER_M` (20 px = 1 m). Wi-Fi ranges, cable lengths and coverage area all use it.
- **Phone link:** it starts only when the user presses "Link a phone". It answers a UDP broadcast `NETSKETCH?` on 47879 and serves `GET /plan` / `POST /points` on TCP 47878, guarded by a 4-digit code. The Java side is `companion-android/src/.../Link.java`, so keep the two in sync.
- **Ports (`ports.rs`):** `Device.ports` is a list of `PortGroup`s (count, speed, RJ45/SFP/DSL/coax, WAN flag), saved as `port_groups`. Each `Link` records the port at each end (`a_port`/`b_port`), and a cable's speed comes from those two ports (`end_mbps`, `negotiated`). `assign_ports()` keeps cables where they fit and puts the rest in the best free port, with the internet line on the WAN side and fast devices on fast ports. It runs in `connect`, in `sanitize`, at the end of every template, and every frame in the UI. After a device gets new hardware, call `replug(id)` so its cables re-plug from scratch. Old saves had `ports`/`port_mbps`; `sanitize` turns them into groups, adding the WAN port the old home-router count left out.
- **Product catalog (`catalog.rs`, files in `data/`, embedded with `include_str!`):**
  - `devices.json` holds real routers, mesh nodes, extenders, APs, outdoor bridges and wired gateways: Swisscom's whole lineup, other Swiss providers' boxes and popular brands. The `Product` struct documents each field. `Product::fits` decides which device kinds a product may become: role `home` (ISP boxes, Wi-Fi routers, gateways, firewalls) is for the Home router only, role `router` (general-purpose wired routers) for the plain Router and the Home router.
  - `plans.json` holds Swiss internet plans, which set the internet connection's speed.
  - `powerline.json` holds powerline kits. A powerline link stores its kit in `Link.kit`, which sets the link's speed and its price in the shopping list.
  - `deny_unknown_fields` plus the catalog tests reject bad entries.
  - Pickers: `radio_ui.rs::catalog_picker` (in "Wi-Fi hardware", and in "Hardware" for plain routers), `ui_extras.rs::plan_picker`, and the "Kit" row in the cable panel.
  - This is separate from the user's own saved models (`extras::Model`, `~/.config/netsketch/models.json`).
  - Known Swisscom gaps: Swisscom's download centre also lists older hardware not yet in the catalog, because its specs weren't verified. That covers the WLAN-Booster (2014), the WLAN Dual Band Access Point (an ASUS RT-AC66U), the WLAN-Netzerweiterer (2013), AirTies units (Air4400, Air4742v2, Air4641), wireless TV kits, and the Huawei mobile routers and hotspots of the Mobile Unlimited offer (B315s, B593s, E5776s, E5786s, E5330). Bluewin-era ADSL routers (Netopia 3347W/3357W, Zyxel Prestige) are missing too. Many power, price and year values are estimates, and each entry's note says so.
- **Built-in browser:** `html.rs` is a deliberately tiny HTML renderer. `main.rs::fetch_real` fetches real pages with `ureq` (3 MB cap, 15 s timeout). Speed-test sites get an internal test instead.

## Conventions seen in the code

- Every file starts with a `//!` header saying what it covers. Keep it accurate when moving things.
- UI and lesson text is plain English for non-experts, current (no obsolete hubs, IPv6 is normal, WPA3, etc.). Don't reintroduce outdated facts.
- Known simplifications are marked `// shortcut: <limit>` (e.g. the multi-wall path-loss model in `wifi.rs`, the HTML subset in `html.rs`).
- Icons come from `egui_phosphor::regular` (imported as `ic`).
