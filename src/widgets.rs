//! Hardware-style controls: panel keys with LEDs, motorised faders and a
//! 7-segment LED display.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Pos2, Rect, Response, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2, epaint::RectShape, pos2, vec2,
};

use crate::theme::{ACCENT, LED_RED, TEXT, TEXT_DIM, WELL};

/// A soft glowing dot: blurred halo plus a bright core.
pub fn glow_dot(p: &egui::Painter, c: Pos2, r: f32, color: Color32, on: f32) {
    let on = on.clamp(0.0, 1.0);
    let off = Color32::from_rgb(color.r() / 5, color.g() / 5, color.b() / 5);
    if on > 0.01 {
        let halo = Rect::from_center_size(c, Vec2::splat(r * 5.0));
        p.add(
            RectShape::filled(
                halo,
                CornerRadius::same(255),
                color.gamma_multiply(0.35 * on),
            )
            .with_blur_width(r * 3.0),
        );
    }
    p.circle_filled(c, r, off.lerp_to_gamma(color, on));
    if on > 0.01 {
        p.circle_filled(
            c - vec2(r * 0.3, r * 0.3),
            r * 0.35,
            Color32::from_white_alpha((90.0 * on) as u8),
        );
    }
}

pub struct Key<'a> {
    pub label: &'a str,
    pub sub: Option<&'a str>,
    /// LED brightness 0..1.
    pub led: f32,
    pub led_color: Color32,
    pub size: Vec2,
    /// Lights the key face itself (used for selected scanners).
    pub lit: bool,
    /// A small dot under the label (a stored scene).
    pub marker: bool,
    /// Keeps reporting the press however long it is held (egui drops a
    /// click-only press after ~0.8 s).
    pub hold: bool,
}

impl<'a> Key<'a> {
    pub fn new(label: &'a str, size: Vec2) -> Self {
        Self {
            label,
            sub: None,
            led: 0.0,
            led_color: LED_RED,
            size,
            lit: false,
            marker: false,
            hold: false,
        }
    }
    pub fn sub(mut self, sub: &'a str) -> Self {
        self.sub = Some(sub);
        self
    }
    pub fn led(mut self, on: f32) -> Self {
        self.led = on;
        self
    }
    pub fn led_color(mut self, c: Color32) -> Self {
        self.led_color = c;
        self
    }
    pub fn lit(mut self, lit: bool) -> Self {
        self.lit = lit;
        self
    }
    pub fn hold(mut self) -> Self {
        self.hold = true;
        self
    }
    pub fn marker(mut self, marker: bool) -> Self {
        self.marker = marker;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let sense = if self.hold {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let (rect, r) = ui.allocate_exact_size(self.size, sense);
        let r = r.on_hover_cursor(CursorIcon::PointingHand);
        let hover = ui
            .ctx()
            .animate_bool_with_time(r.id.with("h"), r.hovered(), 0.08);
        let down = r.is_pointer_button_down_on();
        let p = ui.painter();
        let radius = CornerRadius::same(7);

        // Drop shadow, then the key cap (pressed keys sink by 1.5 px).
        p.rect_filled(
            rect.translate(vec2(0.0, 2.0)),
            radius,
            Color32::from_black_alpha(120),
        );
        let cap = if down {
            rect.translate(vec2(0.0, 1.5))
        } else {
            rect
        };
        let base = if self.lit {
            Color32::from_rgb(86, 70, 38)
        } else {
            Color32::from_rgb(46, 49, 59)
        };
        let fill = base.lerp_to_gamma(Color32::from_rgb(70, 75, 90), hover * 0.5);
        let fill = if down {
            fill.gamma_multiply(0.85)
        } else {
            fill
        };
        p.rect_filled(cap, radius, fill);
        // Bevel: lighter top half and a top highlight line.
        let top = Rect::from_min_max(cap.min, pos2(cap.max.x, cap.center().y));
        p.rect_filled(
            top,
            CornerRadius {
                nw: 7,
                ne: 7,
                sw: 0,
                se: 0,
            },
            Color32::from_white_alpha(7),
        );
        p.line_segment(
            [
                cap.left_top() + vec2(6.0, 1.0),
                cap.right_top() + vec2(-6.0, 1.0),
            ],
            Stroke::new(
                1.0,
                Color32::from_white_alpha(if self.lit { 50 } else { 26 }),
            ),
        );
        let edge = if self.lit {
            ACCENT.gamma_multiply(0.8)
        } else {
            Color32::from_black_alpha(160)
        };
        p.rect_stroke(cap, radius, Stroke::new(1.0, edge), StrokeKind::Inside);
        if self.lit {
            p.add(
                RectShape::filled(cap, radius, ACCENT.gamma_multiply(0.10)).with_blur_width(10.0),
            );
        }

        // LED in the top-left corner.
        glow_dot(
            p,
            cap.left_top() + vec2(9.0, 9.0),
            3.0,
            self.led_color,
            self.led,
        );

        let text_color = if self.lit {
            Color32::from_rgb(255, 228, 180)
        } else {
            TEXT
        };
        let lines = self.label.lines().count().max(1);
        let font = FontId::proportional(if lines > 1 || self.label.len() > 8 {
            11.0
        } else {
            12.5
        });
        let centre = cap.center()
            + vec2(
                0.0,
                if self.sub.is_some() || self.marker {
                    -4.0
                } else {
                    2.0
                },
            );
        if self.marker {
            p.circle_filled(
                pos2(cap.center().x, cap.max.y - 10.0),
                2.5,
                if self.lit { ACCENT } else { TEXT_DIM },
            );
        }
        p.text(centre, Align2::CENTER_CENTER, self.label, font, text_color);
        if let Some(sub) = self.sub {
            p.text(
                pos2(cap.center().x, cap.max.y - 9.0),
                Align2::CENTER_CENTER,
                sub,
                FontId::proportional(9.0),
                if self.lit { ACCENT } else { TEXT_DIM },
            );
        }
        r
    }
}

/// A console fader. `value` is 0..1; returns the new value when the user moves it.
/// Drag the cap (or click the track to jump), scroll for fine steps, double-click
/// to snap between full and zero. When moved by the show, the cap glides like a
/// motorised fader.
pub fn fader(ui: &mut Ui, id: Id, value: f32, size: Vec2, color: Color32) -> Option<f32> {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let cap_h = 22.0;
    let travel = Rect::from_min_max(
        rect.min + vec2(0.0, cap_h / 2.0 + 2.0),
        rect.max - vec2(0.0, cap_h / 2.0 + 2.0),
    );
    let to_y = |v: f32| travel.max.y - v * travel.height();
    let from_y = |y: f32| ((travel.max.y - y) / travel.height()).clamp(0.0, 1.0);

    let grab_id = id.with("grab");
    let mut new = None;
    if r.drag_started() {
        let y = r.interact_pointer_pos().map_or(to_y(value), |p| p.y);
        // Grabbing the cap keeps its offset; clicking elsewhere jumps.
        let offset = if (y - to_y(value)).abs() <= cap_h / 2.0 + 2.0 {
            to_y(value) - y
        } else {
            0.0
        };
        ui.data_mut(|d| d.insert_temp(grab_id, offset));
        new = Some(from_y(y + offset));
    } else if r.dragged() {
        let offset: f32 = ui.data(|d| d.get_temp(grab_id)).unwrap_or(0.0);
        if let Some(p) = r.interact_pointer_pos() {
            new = Some(from_y(p.y + offset));
        }
    }
    if r.clicked()
        && let Some(p) = r.interact_pointer_pos()
        && (p.y - to_y(value)).abs() > cap_h / 2.0 + 2.0
    {
        new = Some(from_y(p.y));
    }
    if r.double_clicked() {
        new = Some(if value < 0.5 { 1.0 } else { 0.0 });
    }
    if r.hovered() {
        // About one DMX step per wheel notch; ten with Shift.
        let (scroll, shift) = ui.input(|i| {
            (
                i.smooth_scroll_delta.y + i.smooth_scroll_delta.x,
                i.modifiers.shift,
            )
        });
        if scroll != 0.0 {
            // Accumulate smooth-scroll pixels into whole DMX steps.
            let acc_id = id.with("scroll");
            let acc: f32 = ui.data(|d| d.get_temp(acc_id)).unwrap_or(0.0) + scroll / 50.0;
            let steps = acc.trunc();
            ui.data_mut(|d| d.insert_temp(acc_id, acc - steps));
            if steps != 0.0 {
                let per = if shift { 10.0 } else { 1.0 } / 255.0;
                new = Some((value + steps * per).clamp(0.0, 1.0));
            }
        }
    }
    let target = new.unwrap_or(value);
    let shown = if r.dragged() || new.is_some() {
        ui.ctx()
            .animate_value_with_time(id.with("pos"), target, 0.0)
    } else {
        ui.ctx()
            .animate_value_with_time(id.with("pos"), target, 0.18)
    };

    let p = ui.painter();
    let cx = rect.center().x;
    // Scale ticks.
    for i in 0..=10 {
        let y = travel.max.y - i as f32 / 10.0 * travel.height();
        let len = if i % 5 == 0 { 7.0 } else { 4.0 };
        let c = Color32::from_white_alpha(if i % 5 == 0 { 40 } else { 22 });
        p.line_segment(
            [pos2(rect.min.x + 2.0, y), pos2(rect.min.x + 2.0 + len, y)],
            Stroke::new(1.0, c),
        );
        p.line_segment(
            [pos2(rect.max.x - 2.0 - len, y), pos2(rect.max.x - 2.0, y)],
            Stroke::new(1.0, c),
        );
    }
    // Track slot with a lit fill below the cap.
    let slot = Rect::from_center_size(
        pos2(cx, travel.center().y),
        vec2(6.0, travel.height() + cap_h * 0.6),
    );
    p.rect_filled(slot, 3.0, WELL);
    p.rect_stroke(
        slot,
        3.0,
        Stroke::new(1.0, Color32::from_black_alpha(200)),
        StrokeKind::Outside,
    );
    let fill = Rect::from_min_max(
        pos2(slot.min.x + 1.0, to_y(shown)),
        pos2(slot.max.x - 1.0, slot.max.y - 1.0),
    );
    if shown > 0.001 {
        p.add(RectShape::filled(fill, 2.0, color.gamma_multiply(0.35)).with_blur_width(6.0));
        p.rect_filled(fill, 2.0, color.gamma_multiply(0.85));
    }

    // Cap.
    let hover = ui
        .ctx()
        .animate_bool_with_time(id.with("hov"), r.hovered() || r.dragged(), 0.08);
    let cap = Rect::from_center_size(pos2(cx, to_y(shown)), vec2(size.x - 10.0, cap_h));
    p.rect_filled(
        cap.translate(vec2(0.0, 3.0)),
        4.0,
        Color32::from_black_alpha(140),
    );
    let body = Color32::from_rgb(72, 76, 90).lerp_to_gamma(Color32::from_rgb(98, 104, 122), hover);
    p.rect_filled(cap, 4.0, body);
    p.rect_filled(
        Rect::from_min_max(cap.min, pos2(cap.max.x, cap.center().y)),
        CornerRadius {
            nw: 4,
            ne: 4,
            sw: 0,
            se: 0,
        },
        Color32::from_white_alpha(14),
    );
    for dy in [-6.0, -3.0, 3.0, 6.0] {
        let y = cap.center().y + dy;
        p.line_segment(
            [pos2(cap.min.x + 5.0, y), pos2(cap.max.x - 5.0, y)],
            Stroke::new(1.0, Color32::from_black_alpha(90)),
        );
    }
    p.line_segment(
        [
            pos2(cap.min.x + 3.0, cap.center().y),
            pos2(cap.max.x - 3.0, cap.center().y),
        ],
        Stroke::new(2.0, color),
    );
    p.rect_stroke(
        cap,
        4.0,
        Stroke::new(1.0, Color32::from_black_alpha(180)),
        StrokeKind::Inside,
    );

    if r.hovered() || r.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
    }
    new.filter(|n| (n - value).abs() > f32::EPSILON)
}

// ---- 7-segment display ------------------------------------------------------------------

const A: u8 = 1;
const B: u8 = 2;
const C: u8 = 4;
const D: u8 = 8;
const E: u8 = 16;
const F: u8 = 32;
const G: u8 = 64;

fn segments(ch: char) -> u8 {
    match ch {
        '0' | 'O' | 'D' => A | B | C | D | E | F,
        '1' | 'I' => B | C,
        '2' | 'Z' | 'z' => A | B | D | E | G,
        '3' => A | B | C | D | G,
        '4' => B | C | F | G,
        '5' | 'S' | 's' => A | C | D | F | G,
        '6' => A | C | D | E | F | G,
        '7' => A | B | C,
        '8' | 'B' => A | B | C | D | E | F | G,
        '9' | 'g' => A | B | C | D | F | G,
        'A' | 'a' => A | B | C | E | F | G,
        'b' => C | D | E | F | G,
        'C' => A | D | E | F,
        'c' => D | E | G,
        'd' => B | C | D | E | G,
        'E' | 'e' => A | D | E | F | G,
        'F' | 'f' => A | E | F | G,
        'G' => A | C | D | E | F,
        'H' => B | C | E | F | G,
        'h' => C | E | F | G,
        'i' => C,
        'J' | 'j' => B | C | D | E,
        'L' | 'l' => D | E | F,
        'M' | 'N' => A | B | C | E | F,
        'n' | 'm' => C | E | G,
        'o' => C | D | E | G,
        'P' | 'p' => A | B | E | F | G,
        'q' => A | B | C | F | G,
        'R' | 'r' => E | G,
        'T' | 't' => D | E | F | G,
        'U' => B | C | D | E | F,
        'u' | 'v' | 'V' => C | D | E,
        'Y' | 'y' => B | C | D | F | G,
        '-' => G,
        '_' => D,
        _ => 0,
    }
}

/// Splits text into up to `digits` cells of (char, decimal point).
fn cells(text: &str, digits: usize) -> Vec<(char, bool)> {
    let mut out: Vec<(char, bool)> = Vec::new();
    for ch in text.chars() {
        if ch == '.' {
            match out.last_mut() {
                Some(last) if !last.1 => last.1 = true,
                _ => out.push((' ', true)),
            }
        } else {
            out.push((ch, false));
        }
    }
    out.truncate(digits);
    while out.len() < digits {
        out.push((' ', false));
    }
    out
}

/// Draws a 4-digit LED readout in a recessed window.
pub fn seven_segment(ui: &mut Ui, text: &str, digit_h: f32, color: Color32) -> Response {
    let digits = 4;
    let dw = digit_h * 0.56;
    let gap = digit_h * 0.22;
    let pad = vec2(digit_h * 0.32, digit_h * 0.26);
    let size = vec2(
        digits as f32 * dw + (digits - 1) as f32 * gap + pad.x * 2.0 + gap * 0.5,
        digit_h + pad.y * 2.0,
    );
    let (rect, r) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();

    p.rect_filled(rect, 8.0, Color32::from_rgb(6, 4, 4));
    p.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, Color32::from_black_alpha(220)),
        StrokeKind::Inside,
    );
    // Faint red tint behind the glass.
    p.add(
        RectShape::filled(rect.shrink(6.0), 6.0, color.gamma_multiply(0.05)).with_blur_width(16.0),
    );
    // Glass reflection.
    p.rect_filled(
        Rect::from_min_max(
            rect.min + vec2(4.0, 3.0),
            pos2(rect.max.x - 4.0, rect.min.y + rect.height() * 0.35),
        ),
        CornerRadius::same(6),
        Color32::from_white_alpha(5),
    );

    let t = digit_h * 0.12; // stroke thickness
    let slant = 0.09;
    let ghost = color.gamma_multiply(0.07);
    for (i, (ch, dot)) in cells(text, digits).into_iter().enumerate() {
        let o = rect.min + pad + vec2(i as f32 * (dw + gap), 0.0);
        let bits = segments(ch);
        let pt = |x: f32, y: f32| pos2(o.x + x + (digit_h - y) * slant, o.y + y);
        let h = digit_h;
        let w = dw;
        let m = h / 2.0;
        let e = t * 0.5; // bevel
        let hseg = |y: f32| {
            vec![
                pt(e + 1.0, y),
                pt(e + 1.0 + e, y - e),
                pt(w - e - 1.0 - e, y - e),
                pt(w - e - 1.0, y),
                pt(w - e - 1.0 - e, y + e),
                pt(e + 1.0 + e, y + e),
            ]
        };
        let vseg = |x: f32, y0: f32, y1: f32| {
            vec![
                pt(x, y0 + 1.0),
                pt(x + e, y0 + 1.0 + e),
                pt(x + e, y1 - 1.0 - e),
                pt(x, y1 - 1.0),
                pt(x - e, y1 - 1.0 - e),
                pt(x - e, y0 + 1.0 + e),
            ]
        };
        let segs = [
            (A, hseg(e)),
            (B, vseg(w - e, e, m)),
            (C, vseg(w - e, m, h - e)),
            (D, hseg(h - e)),
            (E, vseg(e, m, h - e)),
            (F, vseg(e, e, m)),
            (G, hseg(m)),
        ];
        for (bit, pts) in segs {
            let on = bits & bit != 0;
            if on {
                // Glow: a wider, faint copy first.
                let c = pts.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / pts.len() as f32;
                let wide: Vec<Pos2> = pts
                    .iter()
                    .map(|p| (c + (p.to_vec2() - c) * 1.5).to_pos2())
                    .collect();
                p.add(Shape::convex_polygon(
                    wide,
                    color.gamma_multiply(0.18),
                    Stroke::NONE,
                ));
            }
            p.add(Shape::convex_polygon(
                pts,
                if on { color } else { ghost },
                Stroke::NONE,
            ));
        }
        let dot_c = pt(w + gap * 0.45, h - e);
        p.circle_filled(dot_c, t * 0.55, if dot { color } else { ghost });
    }
    r
}

/// A status chip: coloured dot plus text.
pub fn chip(ui: &mut Ui, text: &str, color: Color32, on: bool) -> Response {
    let font = FontId::proportional(11.0);
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), font, if on { TEXT } else { TEXT_DIM });
    let size = vec2(galley.size().x + 28.0, 22.0);
    let (rect, r) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        rect,
        11.0,
        if on {
            color.gamma_multiply(0.14)
        } else {
            Color32::from_white_alpha(6)
        },
    );
    p.rect_stroke(
        rect,
        11.0,
        Stroke::new(
            1.0,
            if on {
                color.gamma_multiply(0.5)
            } else {
                Color32::from_white_alpha(18)
            },
        ),
        StrokeKind::Inside,
    );
    glow_dot(
        p,
        pos2(rect.min.x + 11.0, rect.center().y),
        3.0,
        color,
        if on { 1.0 } else { 0.0 },
    );
    p.galley(
        pos2(rect.min.x + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        TEXT,
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_points_attach_to_the_previous_digit() {
        assert_eq!(
            cells("0.50", 4),
            vec![('0', true), ('5', false), ('0', false), (' ', false)]
        );
        assert_eq!(cells("b 01", 4)[1], (' ', false));
        assert_eq!(cells("12345", 4).len(), 4);
    }

    #[test]
    fn every_panel_message_is_displayable() {
        for msg in [
            "Prog", "run ", "PG A", "PG B", "SEL ", "bLAc", "Sto ", "dEL ", "AUtO", "Snd ", "MAn ",
            "FULL", "CP01", "C102",
        ] {
            for ch in msg.chars().filter(|c| *c != ' ') {
                assert_ne!(segments(ch), 0, "{ch:?} in {msg:?}");
            }
        }
    }
}
