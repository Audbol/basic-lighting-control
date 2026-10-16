//! Stage visualizer: a box truss holding the 12 scanner slots, soft beams
//! through haze, pools on a perspective floor and light spill on the back
//! wall. Pars focus down-stage; moving heads follow pan/tilt.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, epaint::RectShape, pos2, vec2,
};

use crate::engine::{CHANNELS_PER_FIXTURE, FIXTURES, Look, fixture_address};
use crate::fixtures::{FixtureState, Kind, Profile, fixture_state};
use crate::theme::{ACCENT, TEXT_DIM};

pub struct Visualizer {
    pub haze: bool,
}

/// Additive light colour: premultiplied RGB with zero alpha.
fn light(rgb: (f32, f32, f32), a: f32) -> Color32 {
    let k = 255.0 * a.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((rgb.0 * k) as u8, (rgb.1 * k) as u8, (rgb.2 * k) as u8, 0)
}

fn vgradient(mesh: &mut Mesh, rect: Rect, top: Color32, bottom: Color32) {
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

/// Soft ellipse: bright centre fading to nothing, as a triangle fan with rings.
fn soft_ellipse(p: &egui::Painter, c: Pos2, r: Vec2, rings: &[(f32, Color32)], centre: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, centre);
    let seg = 36u32;
    for &(k, color) in rings {
        for i in 0..seg {
            let a = i as f32 / seg as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(c + vec2(a.cos() * r.x * k, a.sin() * r.y * k), color);
        }
    }
    for i in 0..seg {
        let j = (i + 1) % seg;
        mesh.add_triangle(0, 1 + i, 1 + j);
        for ring in 1..rings.len() as u32 {
            let (a0, a1) = ((ring - 1) * seg + 1 + i, (ring - 1) * seg + 1 + j);
            let (b0, b1) = (ring * seg + 1 + i, ring * seg + 1 + j);
            mesh.add_triangle(a0, b0, b1);
            mesh.add_triangle(a0, b1, a1);
        }
    }
    p.add(Shape::mesh(mesh));
}

struct Fx {
    pos: Pos2,
    state: FixtureState,
    level: f32,
    target: Pos2,
    radius: f32,
}

impl Fx {
    fn lit(&self) -> bool {
        matches!(self.state.kind, Kind::Par | Kind::Mover | Kind::Strobe)
            && self.level * self.state.r.max(self.state.g).max(self.state.b) > 0.004
    }
    fn rgb(&self) -> (f32, f32, f32) {
        (self.state.r, self.state.g, self.state.b)
    }
}

impl Visualizer {
    /// Draws the stage; returns a scanner index if one was clicked.
    pub fn show(
        &self,
        ui: &mut Ui,
        output: &Look,
        patch: &[Profile; FIXTURES],
        selected: &[bool; FIXTURES],
        time: f64,
    ) -> Option<usize> {
        let size = ui.available_size().max(vec2(320.0, 220.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter_at(rect);
        let (w, h) = (rect.width(), rect.height());
        let at = |x: f32, y: f32| rect.min + vec2(x, y);
        let horizon = h * 0.6;
        // Floor coordinates: u -1 (stage left) .. 1, v 0 (upstage) .. 1 (downstage).
        let floor = |u: f32, v: f32| {
            let y = horizon + (h * 0.985 - horizon) * v;
            let half = w * (0.30 + 0.24 * v);
            at(w / 2.0 + u * half, y)
        };

        // ---- room --------------------------------------------------------------------------
        let mut room = Mesh::default();
        vgradient(
            &mut room,
            Rect::from_min_max(rect.min, at(w, horizon)),
            Color32::from_rgb(5, 6, 10),
            Color32::from_rgb(15, 17, 25),
        );
        vgradient(
            &mut room,
            Rect::from_min_max(at(0.0, horizon), rect.max),
            Color32::from_rgb(20, 22, 30),
            Color32::from_rgb(11, 12, 16),
        );
        p.add(Shape::mesh(room));
        // Stage edge highlight.
        p.line_segment(
            [at(0.0, horizon), at(w, horizon)],
            Stroke::new(1.0, Color32::from_white_alpha(10)),
        );
        let grid = Stroke::new(1.0, Color32::from_white_alpha(9));
        for i in 0..=10 {
            let u = -1.0 + i as f32 / 5.0;
            p.line_segment([floor(u, 0.0), floor(u, 1.0)], grid);
        }
        for i in 1..=5 {
            let v = (i as f32 / 5.0).powf(1.3);
            p.line_segment([floor(-1.0, v), floor(1.0, v)], grid);
        }

        // ---- fixture states ------------------------------------------------------------------
        let truss_y = h * 0.11;
        let (x0, x1) = (w * 0.07, w * 0.93);
        let fixtures: Vec<Fx> = (0..FIXTURES)
            .map(|f| {
                let values = &output[f * CHANNELS_PER_FIXTURE..(f + 1) * CHANNELS_PER_FIXTURE];
                let state = fixture_state(patch[f], values);
                let mut level = state.dimmer;
                if state.strobe > 0.04 {
                    let hz = 1.0 + state.strobe as f64 * 19.0;
                    if (time * hz).fract() >= 0.22 {
                        level = 0.0;
                    }
                }
                let x = x0 + (x1 - x0) * f as f32 / (FIXTURES - 1) as f32;
                let across = (x - w / 2.0) / (w / 2.0);
                let (u, v, spread) = match state.kind {
                    Kind::Mover => ((state.pan - 0.5) * 2.4, state.tilt.clamp(0.0, 1.0), 0.05),
                    Kind::Strobe => (across * 0.85, 0.4, 0.2),
                    _ => (across * 0.85, 0.45, 0.12),
                };
                let target = floor(u, v);
                Fx {
                    pos: at(x, truss_y + 20.0),
                    state,
                    level,
                    target,
                    radius: w * spread * (0.6 + 0.4 * v),
                }
            })
            .collect();

        // ---- back-wall spill and beams (additive) ----------------------------------------------
        for fx in fixtures.iter().filter(|f| f.lit()) {
            let wall = pos2(fx.pos.x, horizon - h * 0.12);
            soft_ellipse(
                &p,
                wall,
                vec2(w * 0.09, h * 0.16),
                &[
                    (0.5, light(fx.rgb(), 0.05 * fx.level)),
                    (1.0, light(fx.rgb(), 0.0)),
                ],
                light(fx.rgb(), 0.09 * fx.level),
            );
        }
        for (f, fx) in fixtures.iter().enumerate().filter(|(_, f)| f.lit()) {
            if self.haze {
                // Haze breathes slowly so beams aren't flat.
                let breathe = 0.85 + 0.15 * ((time * 0.6 + f as f64 * 1.7).sin() as f32);
                self.beam(&p, fx, breathe);
            }
            // Floor pool, with a hot centre.
            let k = fx.level;
            soft_ellipse(
                &p,
                fx.target,
                vec2(fx.radius, fx.radius * 0.3),
                &[
                    (0.35, light(fx.rgb(), 0.75 * k)),
                    (0.7, light(fx.rgb(), 0.3 * k)),
                    (1.0, light(fx.rgb(), 0.0)),
                ],
                light(fx.rgb(), 1.0 * k),
            );
            // Spill around the pool.
            soft_ellipse(
                &p,
                fx.target,
                vec2(fx.radius * 2.2, fx.radius * 0.7),
                &[(1.0, light(fx.rgb(), 0.0))],
                light(fx.rgb(), 0.12 * k),
            );
        }

        // ---- truss --------------------------------------------------------------------------
        let steel = Color32::from_rgb(78, 82, 94);
        let steel_dark = Color32::from_rgb(44, 47, 56);
        let (tl, tr) = (x0 - 30.0, x1 + 30.0);
        let (top, bottom) = (at(0.0, truss_y - 12.0).y, at(0.0, truss_y + 6.0).y);
        let x_of = |x: f32| rect.min.x + x;
        // Chords and lattice.
        for y in [top, bottom] {
            p.line_segment(
                [pos2(x_of(tl), y), pos2(x_of(tr), y)],
                Stroke::new(3.0, steel),
            );
            p.line_segment(
                [pos2(x_of(tl), y - 1.0), pos2(x_of(tr), y - 1.0)],
                Stroke::new(1.0, Color32::from_white_alpha(30)),
            );
        }
        let bay = 18.0;
        let mut x = tl;
        let mut up = true;
        while x < tr - 0.1 {
            let nx = (x + bay).min(tr);
            let (ya, yb) = if up { (bottom, top) } else { (top, bottom) };
            p.line_segment(
                [pos2(x_of(x), ya), pos2(x_of(nx), yb)],
                Stroke::new(1.2, steel_dark),
            );
            x = nx;
            up = !up;
        }
        for x in [tl, tr] {
            p.rect_filled(
                Rect::from_min_max(
                    pos2(x_of(x) - 3.0, top - 4.0),
                    pos2(x_of(x) + 3.0, bottom + 4.0),
                ),
                1.0,
                steel,
            );
        }

        // ---- fixtures --------------------------------------------------------------------------
        let hover_pos = response.hover_pos();
        let mut hovered = None;
        for (f, fx) in fixtures.iter().enumerate() {
            let hit = Rect::from_center_size(fx.pos, vec2(34.0, 40.0));
            if hover_pos.is_some_and(|hp| hit.contains(hp)) {
                hovered = Some(f);
            }
            self.fixture_body(&p, fx, selected[f], hovered == Some(f));
            // Number tag above the truss.
            let tag = Rect::from_center_size(pos2(fx.pos.x, top - 16.0), vec2(22.0, 15.0));
            let sel = selected[f];
            p.rect_filled(
                tag,
                7.0,
                if sel {
                    ACCENT
                } else {
                    Color32::from_black_alpha(120)
                },
            );
            p.text(
                tag.center(),
                Align2::CENTER_CENTER,
                (f + 1).to_string(),
                FontId::proportional(10.0),
                if sel { Color32::BLACK } else { TEXT_DIM },
            );
        }

        // ---- lens glows and flares (on top of everything) ---------------------------------------
        for fx in fixtures.iter().filter(|f| f.lit()) {
            let c = self.lens_pos(fx);
            let k = fx.level * fx.state.r.max(fx.state.g).max(fx.state.b);
            p.add(
                RectShape::filled(
                    Rect::from_center_size(c, Vec2::splat(46.0)),
                    CornerRadius::same(255),
                    light(fx.rgb(), 0.5 * k),
                )
                .with_blur_width(30.0),
            );
            p.add(
                RectShape::filled(
                    Rect::from_center_size(c, vec2(54.0 * k, 2.0)),
                    CornerRadius::same(2),
                    light(fx.rgb(), 0.22 * k),
                )
                .with_blur_width(4.0),
            );
            p.circle_filled(c, 4.5, light((1.0, 1.0, 1.0), 0.6 * k));
        }

        // ---- vignette ------------------------------------------------------------------------
        let mut vig = Mesh::default();
        let edge = Color32::from_black_alpha(150);
        let clear = Color32::TRANSPARENT;
        let band = 60.0;
        vgradient(
            &mut vig,
            Rect::from_min_max(rect.min, pos2(rect.max.x, rect.min.y + band)),
            edge,
            clear,
        );
        vgradient(
            &mut vig,
            Rect::from_min_max(pos2(rect.min.x, rect.max.y - band), rect.max),
            clear,
            edge,
        );
        p.add(Shape::mesh(vig));
        p.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, Color32::from_black_alpha(200)),
            StrokeKind::Inside,
        );

        // ---- hover info ---------------------------------------------------------------------
        let response = response.on_hover_cursor(if hovered.is_some() {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        });
        if let Some(f) = hovered {
            let fx = &fixtures[f];
            let s = fx.state;
            let mut info = format!(
                "Scanner {} · {}\nDMX {}–{}",
                f + 1,
                patch[f].name(),
                fixture_address(f),
                fixture_address(f) + CHANNELS_PER_FIXTURE - 1
            );
            if s.kind != Kind::None && s.kind != Kind::Generic {
                info += &format!(
                    "\nDimmer {:.0}%   RGB {:.0} {:.0} {:.0}",
                    s.dimmer * 100.0,
                    s.r * 255.0,
                    s.g * 255.0,
                    s.b * 255.0
                );
            }
            if s.kind == Kind::Mover {
                info += &format!("\nPan {:.0}%   Tilt {:.0}%", s.pan * 100.0, s.tilt * 100.0);
            }
            info += "\nClick to select";
            response.clone().on_hover_text_at_pointer(info);
        }

        // Clicking a fixture toggles it, like its scanner button.
        if response.clicked() { hovered } else { None }
    }

    fn lens_pos(&self, fx: &Fx) -> Pos2 {
        match fx.state.kind {
            Kind::Mover => {
                let dir = (fx.target - fx.pos).normalized();
                fx.pos + vec2(0.0, 12.0) + dir * 9.0
            }
            _ => fx.pos + vec2(0.0, 12.0),
        }
    }

    fn beam(&self, p: &egui::Painter, fx: &Fx, breathe: f32) {
        let from = self.lens_pos(fx);
        let to = fx.target;
        let dir = (to - from).normalized();
        let n = vec2(-dir.y, dir.x);
        let k = fx.level * breathe;
        let (src_w, core) = if fx.state.kind == Kind::Mover {
            (4.0, 0.55)
        } else {
            (7.0, 0.38)
        };
        let end = vec2(n.x * fx.radius, n.y * fx.radius * 0.3);
        // Soft edges: transparent sides, bright centre line, fading with distance.
        let mut m = Mesh::default();
        m.colored_vertex(from - n * src_w, light(fx.rgb(), 0.0));
        m.colored_vertex(from, light(fx.rgb(), core * k));
        m.colored_vertex(from + n * src_w, light(fx.rgb(), 0.0));
        m.colored_vertex(to - end, light(fx.rgb(), 0.0));
        m.colored_vertex(to, light(fx.rgb(), 0.07 * k));
        m.colored_vertex(to + end, light(fx.rgb(), 0.0));
        for (a, b, c) in [(0, 1, 4), (0, 4, 3), (1, 2, 5), (1, 5, 4)] {
            m.add_triangle(a, b, c);
        }
        p.add(Shape::mesh(m));
    }

    fn fixture_body(&self, p: &egui::Painter, fx: &Fx, selected: bool, hovered: bool) {
        let body = if fx.state.kind == Kind::None {
            Color32::from_rgb(30, 32, 40)
        } else {
            Color32::from_rgb(40, 43, 52)
        };
        let edge = if selected {
            Stroke::new(1.5, ACCENT)
        } else if hovered {
            Stroke::new(1.0, Color32::from_white_alpha(90))
        } else {
            Stroke::new(1.0, Color32::from_black_alpha(160))
        };
        let c = fx.pos;
        // Hanging clamp.
        p.rect_filled(
            Rect::from_center_size(c - vec2(0.0, 16.0), vec2(8.0, 8.0)),
            1.0,
            Color32::from_rgb(60, 63, 74),
        );
        match fx.state.kind {
            Kind::None => {
                p.rect_stroke(
                    Rect::from_center_size(c, vec2(18.0, 18.0)),
                    4.0,
                    Stroke::new(1.0, Color32::from_white_alpha(25)),
                    StrokeKind::Inside,
                );
            }
            Kind::Mover => {
                // Base, yoke and a head that turns toward its target.
                let base = Rect::from_center_size(c - vec2(0.0, 9.0), vec2(30.0, 9.0));
                p.rect_filled(base, 3.0, body);
                p.rect_stroke(base, 3.0, edge, StrokeKind::Inside);
                for side in [-1.0, 1.0] {
                    p.line_segment(
                        [c + vec2(side * 11.0, -5.0), c + vec2(side * 11.0, 12.0)],
                        Stroke::new(3.0, body),
                    );
                }
                let lens = self.lens_pos(fx);
                p.circle_filled(c + vec2(0.0, 12.0), 11.0, body);
                p.circle_stroke(c + vec2(0.0, 12.0), 11.0, edge);
                p.circle_filled(lens, 5.5, Color32::from_rgb(12, 12, 15));
            }
            _ => {
                let can = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(22.0, 22.0));
                p.rect_filled(can, 5.0, body);
                p.rect_filled(
                    Rect::from_min_max(can.min, pos2(can.max.x, can.center().y)),
                    CornerRadius {
                        nw: 5,
                        ne: 5,
                        sw: 0,
                        se: 0,
                    },
                    Color32::from_white_alpha(8),
                );
                p.rect_stroke(can, 5.0, edge, StrokeKind::Inside);
                // Yoke.
                p.line_segment(
                    [c + vec2(-13.0, -12.0), c + vec2(-13.0, 4.0)],
                    Stroke::new(2.0, Color32::from_rgb(60, 63, 74)),
                );
                p.line_segment(
                    [c + vec2(13.0, -12.0), c + vec2(13.0, 4.0)],
                    Stroke::new(2.0, Color32::from_rgb(60, 63, 74)),
                );
                p.circle_filled(self.lens_pos(fx), 6.5, Color32::from_rgb(12, 12, 15));
            }
        }
        if selected {
            p.add(
                RectShape::filled(
                    Rect::from_center_size(c + vec2(0.0, 3.0), vec2(40.0, 44.0)),
                    10.0,
                    ACCENT.gamma_multiply(0.18),
                )
                .with_blur_width(14.0),
            );
        }
    }
}
