//! The printable report (Export): one self-contained HTML page with each storey's Wi-Fi map as a
//! drawing, the coverage score, the access points and their channels, the shopping list, open
//! problems and the address plan. Opens in any browser; "Print → Save as PDF" makes a PDF of it.
use crate::i18n::{tr, trf};
use crate::net::{Level, Network, PX_PER_M};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// One storey seen from above: Wi-Fi map cells, floor-plan picture, walls, rooms and devices.
fn storey_svg(n: &Network, floor: i8) -> String {
    let walls: Vec<_> = n.walls.iter().filter(|w| w.floor == floor).collect();
    let devs: Vec<_> = n.devices.iter().filter(|d| d.floor == floor && !d.kind.foreign()).collect();
    let pts: Vec<[f32; 2]> = walls.iter().flat_map(|w| [w.a, w.b]).chain(devs.iter().map(|d| d.pos)).chain(n.outdoor.iter().filter(|o| o.floor == floor).flat_map(|o| [o.a, o.b])).collect();
    if pts.is_empty() {
        return String::new();
    }
    let m = 2.0 * PX_PER_M;
    let (x0, y0) = (pts.iter().map(|p| p[0]).fold(f32::MAX, f32::min) - m, pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min) - m);
    let (x1, y1) = (pts.iter().map(|p| p[0]).fold(f32::MIN, f32::max) + m, pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max) + m);
    let (w, h) = (x1 - x0, y1 - y0);
    // Text and dots in proportion to the drawing, so they read the same on a flat and a campus.
    let fs = (w / 70.0).clamp(6.0, 40.0);
    let mut s = format!(r#"<svg viewBox="{x0:.0} {y0:.0} {w:.0} {h:.0}" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="{}">"#, esc(&n.storey(floor)));
    if let Some(p) = n.plans.iter().find(|p| p.floor == floor && !p.data.is_empty()) {
        let pw = p.width_m * PX_PER_M;
        s += &format!(r#"<image href="data:image/jpeg;base64,{}" x="{}" y="{}" width="{pw}" opacity="{}" preserveAspectRatio="xMinYMin meet"/>"#, p.data, p.pos[0], p.pos[1], p.opacity);
    }
    // The map in cells of about half a metre (coarser on big plans: at most ~6000 cells).
    let radios = n.radios();
    let cell = (PX_PER_M / 2.0).max((w * h / 6000.0).sqrt());
    // One see-through group, so cells that overlap by half a pixel don't show seams.
    s += r#"<g opacity="0.45" shape-rendering="crispEdges">"#;
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let best = n.strongest_at(&radios, [x + cell / 2.0, y + cell / 2.0], floor, None, crate::radio_ui::heat_bands(n.target)[4].0);
            if let Some(c) = crate::radio_ui::heat_color(best, n.target) {
                let [r, g, b, _] = c.to_srgba_unmultiplied();
                s += &format!(r#"<rect x="{x:.1}" y="{y:.1}" width="{:.1}" height="{:.1}" fill="rgb({r},{g},{b})"/>"#, cell + 0.5, cell + 0.5);
            }
            x += cell;
        }
        y += cell;
    }
    s += "</g>";
    for o in n.outdoor.iter().filter(|o| o.floor == floor) {
        let (ax, ay, bx, by) = (o.a[0].min(o.b[0]), o.a[1].min(o.b[1]), o.a[0].max(o.b[0]), o.a[1].max(o.b[1]));
        s += &format!(r##"<rect x="{ax}" y="{ay}" width="{}" height="{}" fill="none" stroke="#16a34a" stroke-dasharray="6 4"/>"##, bx - ax, by - ay);
    }
    for wl in walls {
        s += &format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#334155" stroke-width="{:.1}" stroke-linecap="round"/>"##, wl.a[0], wl.a[1], wl.b[0], wl.b[1], fs * 0.3);
    }
    for l in n.labels.iter().filter(|l| l.floor == floor) {
        s += &format!(r##"<text x="{}" y="{}" font-size="{fs:.1}" fill="#475569" text-anchor="middle">{}</text>"##, l.pos[0], l.pos[1], esc(&l.text));
    }
    for d in devs {
        let col = if d.kind.radio() { "#0d9488" } else { "#64748b" };
        s += &format!(r##"<circle cx="{}" cy="{}" r="{:.1}" fill="{col}" stroke="white" stroke-width="{:.1}"/><text x="{}" y="{}" font-size="{fs:.1}" fill="#0f172a">{}</text>"##, d.pos[0], d.pos[1], fs * 0.6, fs * 0.15, d.pos[0] + fs, d.pos[1] + fs * 0.35, esc(&d.name));
    }
    s + "</svg>"
}

/// The room a spot is in, going by the nearest room label on its storey (within 5 m).
fn room_of(n: &Network, p: [f32; 2], floor: i8) -> Option<String> {
    n.labels.iter().filter(|l| l.floor == floor && !l.text.is_empty()).map(|l| ((l.pos[0] - p[0]).hypot(l.pos[1] - p[1]), l)).filter(|x| x.0 <= 5.0 * PX_PER_M).min_by(|a, b| a.0.total_cmp(&b.0)).map(|x| x.1.text.clone())
}

/// A field for a CSV file: quoted when it holds a separator, a quote or a line break.
fn csv(s: &str) -> String {
    if s.contains([',', '"', '\n', ';']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// The cable schedule for whoever lays the cables: from, to, kind, storeys and rooms, length (estimated with the
/// plan's slack, or as measured).
pub fn cables_csv(n: &Network) -> String {
    let mut out = [tr("From"), tr("To"), tr("Type"), tr("Storey"), tr("From room"), tr("To room"), tr("Length (m)")].map(csv).join(",") + "\n";
    for l in n.links.iter().filter(|l| !l.medium.wireless()) {
        let (a, b) = (n.dev(l.a), n.dev(l.b));
        let storey = |d: Option<&crate::net::Device>| d.map(|d| n.storey(d.floor)).unwrap_or_default();
        let room = |d: Option<&crate::net::Device>| d.and_then(|d| room_of(n, d.pos, d.floor)).unwrap_or_default();
        let storeys = if storey(a) == storey(b) { storey(a) } else { format!("{} → {}", storey(a), storey(b)) };
        out += &[n.name(l.a), n.name(l.b), l.medium.label().to_string(), storeys, room(a), room(b), format!("{:.0}", n.link_len(l))].map(|x| csv(&x)).join(",");
        out += "\n";
    }
    out
}

/// The shopping list: item, quantity, price, with the total last.
pub fn shopping_csv(n: &Network) -> String {
    let (items, total) = crate::shopping(n);
    let mut out = [tr("Item"), tr("Quantity"), &trf("Price ({})", &[&n.currency])].map(csv).join(",") + "\n";
    for (what, qty, cost) in items {
        out += &format!("{},{qty},{cost:.0}\n", csv(&what));
    }
    out + &format!("{},,{total:.0}\n", csv(tr("Total (estimate)")))
}

/// The whole report, in the language the app is set to.
pub fn html(n: &Network) -> String {
    crate::wifi::Region::set(n.region);
    let mut s = format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{t}</title><style>
body{{font:15px/1.5 system-ui,sans-serif;color:#0f172a;background:#fff;max-width:1000px;margin:0 auto;padding:24px}}
h1{{margin:0 0 4px}}h2{{margin-top:32px;border-bottom:2px solid #0d9488;padding-bottom:4px}}
table{{border-collapse:collapse;width:100%;margin:8px 0}}th,td{{text-align:left;padding:4px 8px;border-bottom:1px solid #e2e8f0;vertical-align:top}}
th{{background:#f1f5f9}}td.n{{text-align:right;white-space:nowrap}}.dim{{color:#64748b}}
svg{{width:100%;height:auto;border:1px solid #e2e8f0;border-radius:8px;background:#fafafa}}
.key span{{display:inline-block;margin-right:14px}}.key i{{display:inline-block;width:12px;height:12px;border-radius:2px;margin-right:4px;vertical-align:-1px}}
.Error{{color:#b91c1c}}.Warning{{color:#b45309}}.Tip{{color:#0369a1}}
@media print{{body{{padding:0}}h2{{break-after:avoid}}svg,table{{break-inside:avoid}}}}
</style></head><body><h1>{t}</h1><p class="dim">{sub}</p>"#,
        t = esc(&n.title),
        sub = esc(tr("Network plan made with NetSketch. Prices and the Wi-Fi map are estimates."))
    );
    s += "<table>";
    for (k, v) in crate::ui_more::summary(n) {
        s += &format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(&v));
    }
    s += "</table>";

    // Each storey's map, with the colour key and its coverage.
    let key: String = crate::radio_ui::heat_bands(n.target)
        .iter()
        .map(|(v, t)| {
            let [r, g, b, _] = crate::radio_ui::heat_color(*v, n.target).unwrap().to_srgba_unmultiplied();
            format!(r#"<span><i style="background:rgb({r},{g},{b})"></i>{} {v:.0} dBm</span>"#, esc(tr(t)))
        })
        .collect();
    for f in n.used_floors().into_iter().rev() {
        let svg = storey_svg(n, f);
        if svg.is_empty() {
            continue;
        }
        s += &format!("<h2>{}</h2>", esc(&n.storey(f)));
        if let Some(c) = n.coverage(f) {
            s += &format!("<p>{}</p>", esc(&trf("{}% of this storey (≈ {} m²) has good Wi-Fi, {}% at least usable.", &[&format!("{:.0}", c.good * 100.0), &format!("{:.0}", c.area_m2), &format!("{:.0}", c.usable * 100.0)])));
        }
        s += &svg;
        s += &format!(r#"<p class="key dim">{}: {key}</p>"#, esc(tr("Wi-Fi map, strongest band at each spot")));
    }

    s += &format!(
        "<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>",
        esc(tr("Access points and channels")),
        esc(tr("Device")),
        esc(tr("Where")),
        esc(tr("Model")),
        esc(tr("Network name")),
        esc(tr("Bands"))
    );
    for d in n.devices.iter().filter(|d| d.kind.radio() && !d.kind.foreign()) {
        let bands: Vec<String> = crate::wifi::Band::ALL.iter().filter(|b| d.bands[b.i()]).map(|b| trf("{} ch {} / {} MHz / {} dBm", &[&(b.label()), &(n.channel(d, *b)), &(d.width[b.i()]), &format!("{:.0}", d.tx[b.i()])])).collect();
        let model = if d.model.is_empty() { tr("Custom").to_string() } else { d.model.clone() };
        let place = [Some(n.storey(d.floor)), room_of(n, d.pos, d.floor), Some(trf("{} m up", &[&format!("{:.1}", d.mount)]))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        s += &format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", esc(&d.name), esc(&place), esc(&model), esc(&d.ssid), bands.iter().map(|b| esc(b)).collect::<Vec<_>>().join("<br>"));
    }
    s += "</table>";

    s += &format!("<h2>{}</h2><table>", esc(tr("Shopping list (estimates)")));
    let (items, total) = crate::shopping(n);
    for (what, qty, cost) in items {
        s += &format!(r#"<tr><td>{}</td><td class="n">× {qty}</td><td class="n">{}{cost:.0}</td></tr>"#, esc(&what), esc(&n.currency));
    }
    s += &format!(r#"<tr><th>{}</th><th></th><th class="n">{}{total:.0}</th></tr></table>"#, esc(tr("Total (estimate)")), esc(&n.currency));
    s += &format!("<p class=\"dim\">{}</p>", esc(&trf("Power: ≈ {} W, {} kWh a year (≈ {}{} a year).", &[&format!("{:.0}", n.power_w()), &format!("{:.0}", n.yearly_kwh()), &n.currency, &format!("{:.0}", n.yearly_kwh() * n.kwh_price)])));

    let issues = n.check();
    s += &format!("<h2>{}</h2>", esc(tr("Open problems")));
    if issues.is_empty() {
        s += &format!("<p>{}</p>", esc(tr("None found.")));
    } else {
        s += "<ul>";
        for i in &issues {
            let (class, word) = match i.level {
                Level::Error => ("Error", tr("Problem")),
                Level::Warning => ("Warning", tr("Warning")),
                Level::Tip => ("Tip", tr("Tip")),
            };
            s += &format!(r#"<li><b class="{class}">{}</b>: {}</li>"#, esc(word), esc(&i.text));
        }
        s += "</ul>";
    }

    s += &format!("<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>", esc(tr("Devices and addresses")), esc(tr("Device")), esc(tr("Type")), esc(tr("Address")), esc(tr("Storey")));
    for d in n.devices.iter().filter(|d| !d.kind.foreign()) {
        let ips: Vec<String> = d.ifaces.iter().filter(|f| !f.ip.is_empty()).map(|f| format!("{}/{}", f.ip, f.prefix)).collect();
        s += &format!("<tr><td>{}</td><td>{}</td><td>{}{}</td><td>{}</td></tr>", esc(&d.name), esc(&d.kind.label()), esc(&ips.join(", ")), if d.dhcp_client { " (DHCP)" } else { "" }, esc(&n.storey(d.floor)));
    }
    s += "</table>";
    s += &format!("<h2>{}</h2><table>", esc(tr("Connections")));
    for l in &n.links {
        s += &format!(r#"<tr><td>{} ↔ {}</td><td>{}</td><td class="n">{:.0} m</td></tr>"#, esc(&n.name(l.a)), esc(&n.name(l.b)), esc(&l.medium.label()), n.link_len(l));
    }
    s + "</table></body></html>\n"
}

#[cfg(test)]
mod tests {
    use crate::net::template;

    #[test]
    fn the_report_has_a_map_per_storey_and_the_shopping_list() {
        let mut n = template("home");
        n.title = "A <b> & co".into();
        let h = super::html(&n);
        assert!(h.starts_with("<!doctype html>") && h.trim_end().ends_with("</html>"));
        assert!(h.contains("A &lt;b&gt; &amp; co"), "titles are escaped");
        assert_eq!(h.matches("<svg").count(), n.used_floors().len());
        assert!(h.contains("fill=\"rgb(") && h.contains("Shopping list"));
    }

    #[test]
    fn cable_lists_quote_what_needs_it_and_slack_makes_runs_longer() {
        let mut n = template("home");
        let c = super::cables_csv(&n);
        let cables = n.links.iter().filter(|l| !l.medium.wireless()).count();
        assert_eq!(c.lines().count(), cables + 1);
        assert_eq!(super::csv("a, \"b\""), "\"a, \"\"b\"\"\"");
        assert!(super::shopping_csv(&n).lines().last().unwrap().starts_with("Total"));
        let l = n.links.iter().find(|l| l.medium.cable() && l.length_m.is_none()).unwrap().clone();
        let before = n.link_len(&l);
        n.cable_slack = 0.5;
        assert!(n.link_len(&l) > before, "{before} → {}", n.link_len(&l));
    }
}
