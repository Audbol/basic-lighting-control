//! Stage visualizer: a truss holding the 12 scanner slots, beams through haze and
//! pools on a perspective floor. Pars point down-stage; moving heads follow pan/tilt.

use eframe::egui::{self, Color32, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, vec2};

use crate::engine::{CHANNELS_PER_FIXTURE, FIXTURES, Look};
use crate::fixtures::{Kind, Profile, fixture_state};

pub struct Visualizer {
    pub haze: bool,
}

/// Light colour at `a` (0..1) for additive blending: premultiplied with zero alpha.
fn light(r: f32, g: f32, b: f32, a: f32) -> Color32 {
    let k = 255.0 * a.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied((r * k) as u8, (g * k) as u8, (b * k) as u8, 0)
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
        let size = ui.available_size().max(vec2(200.0, 160.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter_at(rect);
        let (w, h) = (rect.width(), rect.height());
        let at = |x: f32, y: f32| rect.min + vec2(x, y);
        // Floor: u -1 (stage left) .. 1, v 0 (upstage) .. 1 (downstage).
        let floor = |u: f32, v: f32| {
            let y = h * (0.58 + 0.40 * v);
            let half = w * (0.32 + 0.2 * v);
            at(w / 2.0 + u * half, y)
        };

        // Room: back wall and floor.
        p.rect_filled(
            Rect::from_min_size(rect.min, vec2(w, h * 0.58)),
            0.0,
            Color32::from_rgb(9, 10, 15),
        );
        p.rect_filled(
            Rect::from_min_max(at(0.0, h * 0.58), rect.max),
            0.0,
            Color32::from_rgb(24, 26, 34),
        );
        let grid = Stroke::new(1.0, Color32::from_white_alpha(14));
        for i in 0..=8 {
            let u = -1.0 + i as f32 / 4.0;
            p.line_segment([floor(u, 0.0), floor(u, 1.0)], grid);
        }
        for i in 0..=4 {
            let v = i as f32 / 4.0;
            p.line_segment([floor(-1.0, v), floor(1.0, v)], grid);
        }

        // Truss.
        let truss_y = h * 0.12;
        let (x0, x1) = (w * 0.08, w * 0.92);
        let steel = Color32::from_rgb(46, 49, 60);
        p.rect_filled(
            Rect::from_min_max(at(x0 - 12.0, truss_y - 16.0), at(x1 + 12.0, truss_y - 10.0)),
            1.0,
            steel,
        );
        p.rect_filled(
            Rect::from_min_max(at(x0 - 12.0, truss_y - 5.0), at(x1 + 12.0, truss_y - 2.0)),
            1.0,
            steel,
        );

        struct Fx {
            pos: Pos2,
            kind: Kind,
            rgb: (f32, f32, f32),
            level: f32,
        }
        let fixtures: Vec<Fx> = (0..FIXTURES)
            .map(|f| {
                let values = &output[f * CHANNELS_PER_FIXTURE..(f + 1) * CHANNELS_PER_FIXTURE];
                let s = fixture_state(patch[f], values);
                let mut level = s.dimmer;
                if s.strobe > 0.04 {
                    let hz = 1.0 + s.strobe as f64 * 19.0;
                    if (time * hz).fract() >= 0.25 {
                        level = 0.0;
                    }
                }
                let x = x0 + (x1 - x0) * f as f32 / (FIXTURES - 1) as f32;
                // Pan/tilt for movers, a fixed down-stage focus for everything else.
                let (u, v, spread) = match s.kind {
                    Kind::Mover => ((s.pan - 0.5) * 2.4, s.tilt.clamp(0.0, 1.0), 0.06),
                    Kind::Strobe => (((x - w / 2.0) / (w / 2.0)) * 0.85, 0.45, 0.22),
                    _ => (((x - w / 2.0) / (w / 2.0)) * 0.85, 0.45, 0.13),
                };
                let fx = Fx {
                    pos: at(x, truss_y),
                    kind: s.kind,
                    rgb: (s.r, s.g, s.b),
                    level,
                };
                if matches!(s.kind, Kind::Par | Kind::Mover | Kind::Strobe) && level > 0.002 {
                    self.beam(
                        &p,
                        fx.pos,
                        floor(u, v),
                        w * spread * (0.6 + 0.4 * v),
                        fx.rgb,
                        level,
                    );
                }
                fx
            })
            .collect();

        // Fixture bodies, lenses and numbers.
        for (f, fx) in fixtures.iter().enumerate() {
            let body = if fx.kind == Kind::None {
                Color32::from_rgb(28, 30, 38)
            } else {
                Color32::from_rgb(62, 66, 80)
            };
            let lens_y = if fx.kind == Kind::Mover {
                p.rect_filled(
                    Rect::from_center_size(fx.pos + vec2(0.0, -5.0), vec2(24.0, 6.0)),
                    1.0,
                    body,
                );
                p.circle_filled(fx.pos + vec2(0.0, 9.0), 11.0, body);
                9.0
            } else {
                p.rect_filled(
                    Rect::from_center_size(fx.pos + vec2(0.0, 3.0), vec2(20.0, 20.0)),
                    3.0,
                    body,
                );
                6.0
            };
            let lit = matches!(fx.kind, Kind::Par | Kind::Mover | Kind::Strobe) && fx.level > 0.002;
            let lens = if lit {
                let (r, g, b) = fx.rgb;
                let c = |x: f32| (40.0 + x * fx.level * 215.0) as u8;
                Color32::from_rgb(c(r), c(g), c(b))
            } else {
                Color32::from_rgb(14, 14, 16)
            };
            p.circle_filled(fx.pos + vec2(0.0, lens_y), 6.0, lens);
            p.text(
                fx.pos + vec2(0.0, -24.0),
                egui::Align2::CENTER_CENTER,
                (f + 1).to_string(),
                egui::FontId::proportional(12.0),
                if selected[f] {
                    Color32::from_rgb(255, 204, 51)
                } else {
                    Color32::from_white_alpha(120)
                },
            );
        }

        // Clicking a fixture toggles it, like its scanner button.
        response
            .interact_pointer_pos()
            .filter(|_| response.clicked())
            .and_then(|pos| {
                fixtures
                    .iter()
                    .position(|fx| (fx.pos + vec2(0.0, 2.0)).distance(pos) < 18.0)
            })
    }

    fn beam(
        &self,
        p: &egui::Painter,
        from: Pos2,
        to: Pos2,
        radius: f32,
        (r, g, b): (f32, f32, f32),
        level: f32,
    ) {
        if r.max(g).max(b) * level <= 0.002 {
            return;
        }
        let dir = (to - from).normalized();
        let n = Vec2::new(-dir.y, dir.x);

        if self.haze {
            let mut mesh = Mesh::default();
            let top = light(r, g, b, 0.45 * level);
            let bottom = light(r, g, b, 0.05 * level);
            mesh.colored_vertex(from + n * 5.0, top);
            mesh.colored_vertex(from - n * 5.0, top);
            mesh.colored_vertex(to - vec2(n.x * radius, n.y * radius * 0.35), bottom);
            mesh.colored_vertex(to + vec2(n.x * radius, n.y * radius * 0.35), bottom);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            p.add(Shape::mesh(mesh));
        }

        // Floor pool: a squashed radial gradient as a triangle fan.
        let mut mesh = Mesh::default();
        mesh.colored_vertex(to, light(r, g, b, 0.9 * level));
        let rings = [(0.6, 0.35), (1.0, 0.0)];
        let segments = 40;
        for (radius_k, alpha) in rings {
            for i in 0..segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let off = vec2(
                    a.cos() * radius * radius_k,
                    a.sin() * radius * radius_k * 0.32,
                );
                mesh.colored_vertex(to + off, light(r, g, b, alpha * level));
            }
        }
        for i in 0..segments as u32 {
            let j = (i + 1) % segments as u32;
            mesh.add_triangle(0, 1 + i, 1 + j);
            let (a, b2) = (1 + i, 1 + j);
            let (c, d) = (1 + segments as u32 + i, 1 + segments as u32 + j);
            mesh.add_triangle(a, c, d);
            mesh.add_triangle(a, d, b2);
        }
        p.add(Shape::mesh(mesh));
    }
}
