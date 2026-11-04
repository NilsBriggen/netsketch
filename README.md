# NetSketch

**Plan, simulate and learn home networks.** Draw your home, place real routers, mesh nodes and access points, see where the Wi-Fi reaches and why a cable is slow, then send packets through it all. Every problem it finds comes with a plain-English explanation and, where it can, a one-click fix.

NetSketch started as a modern successor to [Filius](https://www.lernsoftware-filius.de/), the classic tool for teaching how networks work. It keeps the teaching side and adds what you need to plan a real home or small office.

![NetSketch showing a two-storey home: router, switch, access point and devices on the floor plan, with the plan overview on the right](docs/screenshot.png)

## What it does

### Design your network
- Drag routers, switches, access points, extenders, mesh speakers, PCs, phones, TVs, NAS boxes, cameras and more onto the map, and connect them with Cat 5/5e/6/6A, fibre, powerline, Wi-Fi or a 5G backup line.
- Draw walls by material (drywall, brick, concrete, glass, metal…). Work across several storeys, and put a picture of your floor plan underneath to trace over.
- Model **ports properly**: a router can have "1 × 10G SFP+ WAN, 1 × 2.5G, 4 × 1G". Every cable plugs into a specific port, and its speed comes from the ports at both ends.
- Plan VLANs, guest Wi-Fi, firewall rules (port ranges, TCP/UDP, time schedules for parental control), UPnP, ad blocking, IPv6 with prefix delegation, Power over Ethernet budgets, spanning tree and LACP bundles.
- Storey heights and mounting heights, cable routes drawn along the walls, Thread and Zigbee gadgets with their hub, VPN remote access (it tells you when carrier-grade NAT gets in the way), per-device priority (QoS), Wi-Fi 7 multi-link and UPS runtime.

### Pick real hardware instead of typing specs
A built-in catalog fills in the radio, ports, power draw and price in one click:

| | |
|---|---|
| **287 devices** from 25 brands | Home routers, gateways and firewalls, mesh systems, range extenders, ceiling access points, outdoor point-to-point bridges and plain wired routers |
| **Swiss providers** | All 21 Swisscom routers and WLAN-Boxes from Centro Grande to Internet-Box 5 Pro, plus 25 boxes from Sunrise, Salt and Quickline |
| **Popular brands** | AVM FRITZ!, TP-Link (Archer, Deco, Omada), Netgear (Nighthawk, Orbi), ASUS, Ubiquiti UniFi, eero, Google, Linksys, devolo, MikroTik and more |
| **56 internet plans** | Current plans from Swisscom, Init7, Sunrise, Salt, Quickline, iWay, Wingo, M-Budget, yallo and Teleboy |
| **19 powerline kits** | devolo, TP-Link, AVM, Netgear and Swisscom's own, each with its real-world speed |

Each entry links to the page its specs come from. Values that had to be estimated (often power draw, price or launch year) say so in the entry's note.

### See the Wi-Fi
- A coverage heat map per storey and band (2.4, 5 and 6 GHz), from a multi-wall path-loss model: free-space distance plus each wall's material (hit at an angle costs more) and the floors in between.
- European rules built in: 5 GHz channels 36–64 and 100–140 (30 dBm on the upper ones), 6 GHz up to 6425 MHz, radar (DFS) channel checks, and the 1/5/9/13 plan on 2.4 GHz.
- Channel planning, neighbours' networks and interference, roaming and band steering. You can take a phone for a walk through the house and watch it hand over.
- "Where would another access point help most?", a coverage score for every storey, and outdoor areas (garden, terrace) with their own score. You choose what good Wi-Fi means for you (browsing, video calls, or 4K and gaming), and the score, the map colours and its dBm legend follow.
- Calibrate the model with real measurements: import survey files, or use the companion phone app (below).

### Simulate
- DHCP, DNS, NAT, routing, VLANs and spanning tree behave like the real thing.
- Every device has a terminal: `ipconfig`, `ping`, `traceroute`, `nslookup`, `speedtest` (real speed and the bottleneck on the way), `wifi`, `stp`, `vlan`, `arp`, `route` and more.
- A built-in browser opens pages hosted on your simulated servers, or real websites.
- Watch packets travel along the cables in the packet log, try "what if this cable breaks?", and see traffic at the evening busy hour.
- Click any packet to see its layers (Ethernet, IP with TTL, TCP/UDP with sequence numbers, the application), or step through a packet's journey hop by hop.
- Mail that follows MX records and is relayed between servers (`mail`, `inbox`), ARP and switch MAC tables, and RIP dynamic routing that spreads one 30-second update at a time.

### Plan the practical side
- A shopping list with prices, one line per model, leaving out what you already own (phones, computers and TVs by default). It also gives the cable you'll need to lay, and yearly power use and running cost (TVs and PCs count their typical hours, not 24/7).
- Compare two saved options side by side: cost, cable, power, Wi-Fi coverage and open problems.
- Export a printable report, a web page that any browser saves as PDF. It has each storey's Wi-Fi map, the access points and channels, the shopping list, open problems and the addresses. You can also save the map as a picture.
- Floor-plan pictures are saved inside the plan file, so a plan you send keeps them.
- Undo and redo, autosave, "Recover last session", Save as, and recent plans. NetSketch asks before closing with unsaved changes. Plans, phone surveys and pictures can be dropped on the window.

### Learn
**Learn** mode has 19 short, up-to-date lessons, from "How a network fits together" through IPv6, Wi-Fi security, mesh and roaming to VLANs, each with a hands-on "try it". There are also ready-made scenarios that each show a real problem:

*Small flat · Typical home · Walk & roam · Range-extender trouble · Crowded apartment block · The speaker that ate the network · The nightmare home · University campus · Office with 100 VLANs · Two networks and a router (the classic first lesson)*

Every lesson ends with a check-yourself question (progress is remembered), and five scenarios are "find the fault" exercises with a **Check my work** button.

Old Filius projects (`.fls`) can be opened too.

### Comfort
- English or German (settings menu, top right), dark, light or high-contrast colours, and a text-size slider.
- Keyboard: Tab walks through the devices on the map, the arrow keys nudge them, and the keyboard button lists every shortcut.

## Getting started

Download the latest version from the [releases page](https://github.com/NilsBriggen/netsketch/releases):

| | |
|---|---|
| **Windows** | `NetSketch-…-x64.msi` installs it with a Start-menu entry and opens `.netplan` files; `…-portable.exe` runs without installing. Windows may warn that the publisher is unknown (the builds aren't code-signed): choose *More info → Run anyway*. |
| **Linux** | `NetSketch-x86_64.AppImage`: make it executable (`chmod +x`) and run it. It needs GTK 3, which desktop Linux systems have. |
| **macOS** (Apple silicon) | `netsketch-macos-arm64.tar.gz`: unpack it and run `netsketch`. It isn't notarised: if macOS refuses, run `xattr -d com.apple.quarantine netsketch` once. |
| **Android** | `NetSketch-Survey-….apk`, the companion phone app (see below). |

To build it yourself you need a recent stable [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/NilsBriggen/netsketch.git
cd netsketch
cargo run --release
```

Plans are saved as `.netplan` files (JSON). Your own saved device models and the autosave live in `~/.config/netsketch/` (or `$XDG_CONFIG_HOME/netsketch/`).

NetSketch is built with [egui/eframe](https://github.com/emilk/egui). It's developed and tested on Linux; the Windows and macOS builds pass the same tests in CI, but haven't been tried by hand yet.

### Development

```bash
cargo test                                  # all tests, no GUI needed
cargo test ports                            # tests whose name contains "ports"
cargo run -- --screenshot shots/now.png     # render the app to a PNG and exit
```

GitHub Actions builds and tests every push on Linux, Windows and macOS. Tagging a version (`v0.2.0`) builds the AppImage, the Windows installer and portable exe, the macOS archive and the phone app, and publishes them as a GitHub release with the notes in `.github/release-notes/<tag>.md` (tags with a dash, like `v0.1-beta`, become pre-releases). The installer and desktop files are in `packaging/`. German text lives in `data/de.json` (interface) and `data/lessons_de.json` (lessons): English text is the key, and anything missing stays English.

`CLAUDE.md` describes the architecture in more detail.

## NetSketch Survey: the companion phone app

`companion-android/` holds a small Android app (Android 10 or newer) for measuring Wi-Fi where you actually stand.

- **Linked (recommended):** in NetSketch, choose *Real measurements → Link a phone (live)* and enter the 4-digit code on the phone. Your floor plan appears on the phone. Tap where you stand, press *Measure here*, and the reading lands in NetSketch, pinned in place.
- **Offline:** measure anyway, then share the file and import it under *Real measurements → Import measurements…*.

On the phone, each spot gets a plain-language verdict (weak signal, a sticky connection, the slow band, a crowded channel and which one to try), a channel chart and an optional speed test (download, upload, delay), always over Wi-Fi, never mobile data. *Quick point* records just your own network's signal without a scan. That's fast for many spots, because Android allows only 4 scans every 2 minutes. A ping test checks the delay to your router, *Measure again here* replaces a point, and a measurement can be cancelled. The plan can be zoomed, and your points show in colour by signal strength. The app follows the phone's language (English or German) and its dark or light setting.

Measured networks named like yours are matched to your access points. The others can be added as neighbours, positioned where their signal fits your readings. If the phone can't connect, allow TCP 47878 and UDP 47879 through the computer's firewall.

Download `NetSketch-Survey-….apk` from the [releases](https://github.com/NilsBriggen/netsketch/releases), or build it with `./build.sh` (JDK 17+ and the Android SDK, no Gradle). CI builds it on every push. [`INSTALL.txt`](companion-android/INSTALL.txt) walks through installing and linking it.

## Adding hardware to the catalog

The catalog is plain JSON in [`data/`](data/):

- `devices.json`: home routers and gateways, plain routers, mesh nodes, extenders, access points and bridges.
- `plans.json`: internet plans.
- `powerline.json`: powerline kits.

Each field is explained on the `Product`, `Plan` and `PowerlineKit` structs in [`src/catalog.rs`](src/catalog.rs). Add an entry, run `cargo test`, and the catalog tests will reject anything implausible: unknown fields, impossible channel widths, odd port speeds, duplicates or a missing source.

Known gaps: some older hardware Swisscom supplied (the 2013–2014 boosters and repeaters, AirTies units, Huawei mobile routers, and Bluewin-era ADSL routers) isn't in the catalog yet.

## About this project

NetSketch is an independent, clean-room rewrite. It isn't affiliated with the Filius project and contains none of its code. The `.fls` import works from Filius's saved file format only.

NetSketch is free software under the [GNU General Public License v3.0](LICENSE): you may use, study, share and change it, and versions you pass on must stay under the same license with their source.
