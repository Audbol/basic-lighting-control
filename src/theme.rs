//! Colours and egui style: a dark console look with an amber accent.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2,
};

pub const BG: Color32 = Color32::from_rgb(14, 15, 19);
pub const PANEL: Color32 = Color32::from_rgb(21, 23, 28);
pub const CARD: Color32 = Color32::from_rgb(28, 30, 37);
pub const CARD_EDGE: Color32 = Color32::from_rgb(40, 43, 52);
pub const WELL: Color32 = Color32::from_rgb(9, 10, 13);
pub const TEXT: Color32 = Color32::from_rgb(222, 224, 230);
pub const TEXT_DIM: Color32 = Color32::from_rgb(132, 137, 150);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(84, 88, 100);
pub const ACCENT: Color32 = Color32::from_rgb(255, 176, 46);
pub const LED_RED: Color32 = Color32::from_rgb(255, 52, 40);
pub const LED_GREEN: Color32 = Color32::from_rgb(70, 230, 120);
pub const LED_BLUE: Color32 = Color32::from_rgb(70, 160, 255);

pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.all_styles_mut(|style| {
        use FontFamily::{Monospace, Proportional};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(17.0, Proportional)),
            (TextStyle::Body, FontId::new(13.5, Proportional)),
            (TextStyle::Button, FontId::new(13.5, Proportional)),
            (TextStyle::Small, FontId::new(11.0, Proportional)),
            (TextStyle::Monospace, FontId::new(12.5, Monospace)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(8.0, 6.0);
        s.button_padding = Vec2::new(10.0, 4.0);
        s.interact_size.y = 24.0;
        s.window_margin = Margin::same(12);

        let v = &mut style.visuals;
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.extreme_bg_color = WELL;
        v.faint_bg_color = CARD;
        v.override_text_color = None;
        v.window_stroke = Stroke::new(1.0, CARD_EDGE);
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        v.popup_shadow = Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        v.slider_trailing_fill = true;

        let radius = CornerRadius::same(6);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = CARD;
        w.noninteractive.weak_bg_fill = CARD;
        w.noninteractive.bg_stroke = Stroke::new(1.0, CARD_EDGE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
        w.noninteractive.corner_radius = radius;
        for (wv, fill, stroke) in [
            (
                &mut w.inactive,
                Color32::from_rgb(38, 41, 50),
                Color32::from_rgb(52, 56, 68),
            ),
            (
                &mut w.hovered,
                Color32::from_rgb(48, 52, 63),
                Color32::from_rgb(80, 85, 100),
            ),
            (&mut w.active, Color32::from_rgb(58, 62, 75), ACCENT),
            (
                &mut w.open,
                Color32::from_rgb(44, 48, 58),
                Color32::from_rgb(80, 85, 100),
            ),
        ] {
            wv.bg_fill = fill;
            wv.weak_bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0, stroke);
            wv.fg_stroke = Stroke::new(1.0, TEXT);
            wv.corner_radius = radius;
            wv.expansion = 0.0;
        }
    });
}

/// Rounded card used to group panel sections.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, CARD_EDGE))
        .corner_radius(10.0)
        .inner_margin(Margin::symmetric(14, 12))
}

/// Small letter-spaced section caption.
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.5)
            .color(TEXT_FAINT)
            .strong(),
    );
}

/// The window icon: a light beam from a fixture, drawn procedurally.
pub fn icon() -> egui::IconData {
    let n = 64u32;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 / n as f32, y as f32 / n as f32);
            let i = ((y * n + x) * 4) as usize;
            // Rounded dark tile.
            let dx = (fx - 0.5).abs().max(0.0) - 0.36;
            let dy = (fy - 0.5).abs().max(0.0) - 0.36;
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() > 0.12;
            if outside {
                continue;
            }
            let mut c = [22.0f32, 24.0, 30.0];
            // Beam: a cone opening downward from the top centre.
            let half = 0.05 + fy * 0.32;
            let d = (fx - 0.5).abs();
            if fy > 0.18 && d < half {
                let k = (1.0 - d / half).powf(0.7) * (1.0 - fy * 0.45);
                c[0] += 233.0 * k;
                c[1] += 150.0 * k;
                c[2] += 20.0 * k;
            }
            // Fixture head.
            if (0.36..0.64).contains(&fx) && (0.08..0.2).contains(&fy) {
                c = [70.0, 74.0, 88.0];
            }
            rgba[i] = c[0].min(255.0) as u8;
            rgba[i + 1] = c[1].min(255.0) as u8;
            rgba[i + 2] = c[2].min(255.0) as u8;
            rgba[i + 3] = 255;
        }
    }
    egui::IconData {
        rgba,
        width: n,
        height: n,
    }
}

/// A card whose contents stack vertically, whatever the parent layout.
pub fn vcard<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card().show(ui, |ui| ui.vertical(add).inner).inner
}
