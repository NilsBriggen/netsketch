//! Small building blocks shared by the panels: tabs, banners, captions, icon tiles, rail items, pills and floating map panels.
use crate::pal;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Id, Margin, Pos2, Rect, RichText, Sense, Stroke, vec2};

/// A row of text tabs with an underline under the open one. Returns true when the user picked another tab.
pub fn tabs<T: PartialEq + Copy>(ui: &mut egui::Ui, current: &mut T, items: &[(T, String)]) -> bool {
    let mut changed = false;
    let r = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.spacing_mut().button_padding = vec2(4.0, 4.0);
            for (t, label) in items {
                let on = *current == *t;
                let text = RichText::new(label).color(if on { pal().text } else { pal().dim });
                let resp = ui.add(egui::Button::new(text).frame(false).min_size(vec2(0.0, 26.0)));
                let line = Rect::from_min_max(resp.rect.left_bottom() + vec2(4.0, -1.0), resp.rect.right_bottom() + vec2(-4.0, 1.0));
                if on {
                    ui.painter().rect_filled(line, 1, pal().accent);
                } else if resp.hovered() {
                    ui.painter().rect_filled(line, 1, pal().line);
                }
                if resp.clicked() && !on {
                    *current = *t;
                    changed = true;
                }
            }
        })
        .response;
    let y = r.rect.bottom() + 1.0;
    ui.painter().hline(r.rect.left()..=ui.max_rect().right(), y, Stroke::new(1.0, pal().line));
    ui.add_space(6.0);
    changed
}

/// A small dim caption above a group of rows.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).small().color(pal().dim));
}

/// A coloured message strip (problem, warning, tip).
pub fn banner<R>(ui: &mut egui::Ui, col: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(col.gamma_multiply(0.12)).stroke(Stroke::new(1.0, col.gamma_multiply(0.35))).corner_radius(8).inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    })
    .inner
}

/// A small icon tile, e.g. at the top of a panel.
pub fn icon_tile(ui: &mut egui::Ui, icon: &str, col: Color32, size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::same((size * 0.25) as u8), pal().card);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, icon, FontId::proportional(size * 0.52), col);
    resp
}

/// The frame floating panels on the map use.
pub fn float_frame() -> egui::Frame {
    egui::Frame::new().fill(pal().panel).stroke(Stroke::new(1.0, pal().line)).corner_radius(10).inner_margin(Margin::same(4))
}

/// A panel floating over the map, pinned at `pos` by its `pivot` corner.
pub fn float<R>(ctx: &egui::Context, id: &str, pos: Pos2, pivot: Align2, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let r = egui::Area::new(Id::new(id)).fixed_pos(pos).pivot(pivot).order(egui::Order::Middle).show(ctx, |ui| float_frame().show(ui, add).inner);
    r
}

/// A square icon button for docks and toolbars; `on` shows it picked.
pub fn icon_button(ui: &mut egui::Ui, icon: &str, on: bool, hint: &str) -> egui::Response {
    let b = egui::Button::new(RichText::new(icon).size(17.0).color(if on { pal().accent } else { pal().text }))
        .fill(if on { pal().accent.gamma_multiply(0.16) } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .min_size(vec2(32.0, 32.0));
    ui.add(b).on_hover_text(hint)
}

/// One entry of the left rail: icon over a short label, with an optional count badge.
pub fn rail_item(ui: &mut egui::Ui, icon: &str, label: &str, on: bool, badge: Option<(usize, Color32)>) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(56.0, 50.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(r.shrink2(vec2(4.0, 2.0)), 8, pal().accent.gamma_multiply(0.16));
    } else if resp.hovered() {
        p.rect_filled(r.shrink2(vec2(4.0, 2.0)), 8, pal().hover);
    }
    let col = if on { pal().accent } else { pal().dim };
    p.text(r.center() - vec2(0.0, 8.0), Align2::CENTER_CENTER, icon, FontId::proportional(19.0), col);
    p.text(r.center() + vec2(0.0, 13.0), Align2::CENTER_CENTER, label, FontId::proportional(10.5), if on { pal().text } else { pal().dim });
    if let Some((n, c)) = badge.filter(|b| b.0 > 0) {
        let at = r.center() + vec2(11.0, -15.0);
        let t = if n > 99 { "99+".to_string() } else { n.to_string() };
        let w = 8.0 + 6.0 * t.len() as f32;
        p.rect_filled(Rect::from_center_size(at, vec2(w, 15.0)), 8, c);
        p.text(at, Align2::CENTER_CENTER, t, FontId::proportional(10.0), pal().bg);
    }
    resp.on_hover_text(label)
}

/// Picked or not, as a pill (segmented controls, filters).
pub fn pill(ui: &mut egui::Ui, on: bool, text: impl Into<String>) -> egui::Response {
    let b = egui::Button::new(RichText::new(text.into()).color(if on { pal().text } else { pal().dim }))
        .fill(if on { pal().card } else { Color32::TRANSPARENT })
        .stroke(if on { Stroke::new(1.0, pal().line) } else { Stroke::NONE })
        .corner_radius(7);
    ui.add(b)
}
