//! The front panel: Obey 40 layout plus visualizer, patch and output settings.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Rect, RichText, Sense, Slider, Stroke, Ui,
    Vec2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::engine::{
    CHANNELS_PER_FIXTURE, CHASES, Controller, FADERS, FIXTURES, Mode, Page, SCENES_PER_BANK, Show,
    fixture_address,
};
use crate::fixtures::{DEFAULT_PATCH, Profile};
use crate::output::Output;
use crate::visualizer::Visualizer;

const PROGRAM_HOLD_SECONDS: f64 = 3.0;

const PANEL: Color32 = Color32::from_rgb(30, 32, 38);
const KEY: Color32 = Color32::from_rgb(52, 55, 64);
const KEY_HOVER: Color32 = Color32::from_rgb(66, 70, 82);
const LED_ON: Color32 = Color32::from_rgb(255, 48, 40);
const LED_OFF: Color32 = Color32::from_rgb(70, 22, 20);
const AMBER: Color32 = Color32::from_rgb(255, 196, 64);

/// What a show file holds: the programming plus the patch.
#[derive(Serialize, Deserialize)]
struct ShowFile {
    show: Show,
    patch: [Profile; FIXTURES],
}

pub struct App {
    controller: Arc<Mutex<Controller>>,
    output: Output,
    epoch: Instant,
    patch: [Profile; FIXTURES],
    visualizer: Visualizer,
    show_path: String,
    status: String,
    confirm_new: bool,
    dirty: bool,
    last_save: f64,
    program_hold: Option<f64>,
    program_hold_fired: bool,
    artnet_ip: String,
    #[cfg(feature = "enttec")]
    serial_ports: Vec<String>,
    #[cfg(feature = "enttec")]
    serial_choice: String,
    #[cfg(feature = "audio")]
    mic: Option<crate::audio::Mic>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, show_path: PathBuf) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let epoch = Instant::now();
        let controller = Arc::new(Mutex::new(Controller::default()));
        let output = Output::start(controller.clone(), epoch);
        let mut app = Self {
            controller,
            output,
            epoch,
            patch: DEFAULT_PATCH,
            visualizer: Visualizer { haze: true },
            show_path: show_path.display().to_string(),
            status: String::new(),
            confirm_new: false,
            dirty: false,
            last_save: 0.0,
            program_hold: None,
            program_hold_fired: false,
            artnet_ip: Ipv4Addr::BROADCAST.to_string(),
            #[cfg(feature = "enttec")]
            serial_ports: crate::enttec::list_ports(),
            #[cfg(feature = "enttec")]
            serial_choice: String::new(),
            #[cfg(feature = "audio")]
            mic: None,
        };
        if show_path.exists() {
            app.load();
        } else {
            let mut c = app.controller.lock().unwrap();
            *c = Controller::new(crate::demo::demo_show(&app.patch));
            c.press_scene(0);
            drop(c);
            app.status = format!("Demo show loaded — saves to {}", app.show_path);
        }
        app
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    fn save(&mut self) {
        let file = ShowFile {
            show: self.controller.lock().unwrap().show.clone(),
            patch: self.patch,
        };
        let result = serde_json::to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|json| std::fs::write(&self.show_path, json).map_err(|e| e.to_string()));
        self.status = match result {
            Ok(()) => format!("Saved {}", self.show_path),
            Err(e) => format!("Save failed: {e}"),
        };
        self.dirty = false;
        self.last_save = self.now();
    }

    fn load(&mut self) {
        let result = std::fs::read_to_string(&self.show_path)
            .map_err(|e| e.to_string())
            .and_then(|json| serde_json::from_str::<ShowFile>(&json).map_err(|e| e.to_string()));
        match result {
            Ok(file) => {
                *self.controller.lock().unwrap() = Controller::new(file.show);
                self.patch = file.patch;
                self.dirty = false;
                self.status = format!("Loaded {}", self.show_path);
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    // ---- top bar ---------------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("OBEY-LITE").strong().color(AMBER));
            ui.label(RichText::new("192 DMX CHANNEL CONTROLLER").small().weak());
            ui.separator();
            ui.label("Show file");
            if ui
                .add(egui::TextEdit::singleline(&mut self.show_path).desired_width(220.0))
                .lost_focus()
            {
                self.status.clear();
            }
            if ui.button("Save").clicked() {
                self.save();
            }
            if ui.button("Load").clicked() {
                self.load();
            }
            if self.confirm_new {
                ui.label(RichText::new("Erase all scenes and chases?").color(LED_ON));
                if ui.button("Erase").clicked() {
                    *self.controller.lock().unwrap() = Controller::default();
                    self.confirm_new = false;
                    self.dirty = true;
                    self.status = "New show".into();
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_new = false;
                }
            } else if ui.button("New").clicked() {
                self.confirm_new = true;
            }
            ui.separator();
            ui.checkbox(&mut self.visualizer.haze, "Haze");
            #[cfg(feature = "audio")]
            {
                let mut on = self.mic.is_some();
                if ui.checkbox(&mut on, "Mic").changed() {
                    if on {
                        match crate::audio::Mic::start(self.controller.clone()) {
                            Ok(m) => self.mic = Some(m),
                            Err(e) => self.status = format!("Mic: {e}"),
                        }
                    } else {
                        self.mic = None;
                    }
                }
            }
            ui.separator();
            ui.label(RichText::new(&self.status).weak());
        });
    }

    // ---- right panel: patch, outputs, DMX monitor --------------------------------------------

    fn side_panel(&mut self, ui: &mut Ui, c: &mut Controller) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Patch");
            ui.label(
                RichText::new("Each scanner owns 16 channels from its start address.")
                    .small()
                    .weak(),
            );
            egui::Grid::new("patch")
                .num_columns(3)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    for f in 0..FIXTURES {
                        let label = RichText::new(format!("Scanner {}", f + 1));
                        ui.label(if c.selected[f] {
                            label.color(AMBER)
                        } else {
                            label
                        });
                        ui.label(
                            RichText::new(format!("@{:03}", fixture_address(f)))
                                .monospace()
                                .weak(),
                        );
                        egui::ComboBox::from_id_salt(("profile", f))
                            .width(150.0)
                            .selected_text(self.patch[f].name())
                            .show_ui(ui, |ui| {
                                for p in Profile::ALL {
                                    if ui
                                        .selectable_value(&mut self.patch[f], p, p.name())
                                        .changed()
                                    {
                                        self.dirty = true;
                                    }
                                }
                            });
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.heading("Output");
            let status = self.output.status.lock().unwrap().clone();
            {
                let mut cfg = self.output.config.lock().unwrap();
                ui.checkbox(&mut cfg.artnet_enabled, "Art-Net");
                ui.horizontal(|ui| {
                    ui.label("Target IP");
                    let valid = self.artnet_ip.parse::<Ipv4Addr>();
                    let edit = egui::TextEdit::singleline(&mut self.artnet_ip)
                        .desired_width(120.0)
                        .text_color(if valid.is_ok() {
                            Color32::WHITE
                        } else {
                            LED_ON
                        });
                    if ui.add(edit).changed()
                        && let Ok(ip) = self.artnet_ip.parse()
                    {
                        cfg.artnet_target = ip;
                    }
                });
                ui.horizontal(|ui| {
                    for (label, ip) in [
                        ("Broadcast", "255.255.255.255"),
                        ("2.x", "2.255.255.255"),
                        ("10.x", "10.255.255.255"),
                    ] {
                        if ui.small_button(label).clicked() {
                            self.artnet_ip = ip.into();
                            cfg.artnet_target = ip.parse().unwrap();
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Universe");
                    ui.add(egui::DragValue::new(&mut cfg.artnet_universe).range(0..=32767));
                    let u = cfg.artnet_universe;
                    ui.label(
                        RichText::new(format!(
                            "net {} · sub {} · uni {}",
                            u >> 8,
                            (u >> 4) & 0xf,
                            u & 0xf
                        ))
                        .small()
                        .weak(),
                    );
                });
                ui.label(RichText::new(&status.artnet).small().weak());

                #[cfg(feature = "enttec")]
                {
                    ui.add_space(6.0);
                    ui.label("ENTTEC DMX USB Pro");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("serial")
                            .width(150.0)
                            .selected_text(if self.serial_choice.is_empty() {
                                "Port…"
                            } else {
                                &self.serial_choice
                            })
                            .show_ui(ui, |ui| {
                                for p in &self.serial_ports {
                                    ui.selectable_value(&mut self.serial_choice, p.clone(), p);
                                }
                            });
                        if ui
                            .small_button("Rescan")
                            .on_hover_text("Rescan ports")
                            .clicked()
                        {
                            self.serial_ports = crate::enttec::list_ports();
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.serial_choice)
                                .hint_text("/dev/ttyUSB0 or COM3")
                                .desired_width(150.0),
                        );
                        if cfg.enttec_port.is_some() {
                            if ui.button("Disconnect").clicked() {
                                cfg.enttec_port = None;
                            }
                        } else if ui.button("Connect").clicked() && !self.serial_choice.is_empty() {
                            cfg.enttec_port = Some(self.serial_choice.clone());
                        }
                    });
                    ui.label(RichText::new(&status.enttec).small().weak());
                }
            }

            ui.add_space(10.0);
            ui.heading("DMX monitor");
            dmx_monitor(ui, &c.output);

            ui.add_space(10.0);
            egui::CollapsingHeader::new("How to program (Obey 40 style)").show(ui, |ui| {
                ui.label(HELP);
            });
        });
    }

    // ---- the front panel ------------------------------------------------------------------

    fn front_panel(&mut self, ui: &mut Ui, c: &mut Controller, t: f64) {
        let blink = (t * 2.5).fract() < 0.5;
        let profile = c.first_selected().map(|f| self.patch[f]);
        let mut touched = false;

        egui::Frame::new()
            .fill(PANEL)
            .inner_margin(12.0)
            .corner_radius(6.0)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Left block: scanners above faders.
                    ui.vertical(|ui| {
                        ui.label(RichText::new("SCANNERS").small().weak());
                        ui.horizontal(|ui| {
                            for f in 0..FIXTURES {
                                if key(ui, &format!("{}", f + 1), c.selected[f], vec2(44.0, 34.0))
                                    .clicked()
                                {
                                    c.press_fixture(f);
                                }
                            }
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().slider_width = 170.0;
                            for f in 0..FADERS {
                                let ch = c.fader_channel(f);
                                ui.vertical(|ui| {
                                    ui.set_width(56.0);
                                    ui.label(
                                        RichText::new(format!("CH {}", ch + 1))
                                            .monospace()
                                            .strong(),
                                    );
                                    let role =
                                        profile.map_or("—".into(), |p| p.channel_label(ch));
                                    ui.label(RichText::new(role).small().color(AMBER));
                                    let mut v = c.faders[f];
                                    if ui
                                        .add(
                                            Slider::new(&mut v, 0..=255)
                                                .vertical()
                                                .show_value(false),
                                        )
                                        .changed()
                                    {
                                        c.move_fader(f, v);
                                        touched = true;
                                    }
                                    ui.label(RichText::new(format!("{v:3}")).monospace().weak());
                                });
                            }
                            ui.vertical(|ui| {
                                ui.add_space(30.0);
                                if key(ui, "PAGE\nSELECT", false, vec2(64.0, 44.0)).clicked() {
                                    c.toggle_page();
                                }
                                led_label(ui, "A  1-8", c.page == Page::A);
                                led_label(ui, "B  9-16", c.page == Page::B);
                            });
                        });
                    });

                    ui.separator();

                    // Right block: display, function keys, scenes, chases, speed/fade.
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            egui::Frame::new()
                                .fill(Color32::BLACK)
                                .inner_margin(egui::Margin::symmetric(14, 6))
                                .corner_radius(4.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(format!("{:<4}", c.display()))
                                            .font(FontId::monospace(34.0))
                                            .color(LED_ON),
                                    );
                                });
                            ui.vertical(|ui| {
                                led_label(ui, "PROGRAM", c.program_mode && blink);
                                led_label(ui, "AUTO", c.mode == Mode::Auto);
                                led_label(ui, "MUSIC", c.mode == Mode::Music);
                                led_label(ui, "BLACKOUT", c.blackout && blink);
                            });
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(format!("Step {:.2}s", c.step_seconds())).small(),
                                );
                                ui.label(
                                    RichText::new(format!("Fade {:.1}s", c.fade_seconds())).small(),
                                );
                                if c.tap_seconds.is_some() {
                                    ui.label(RichText::new("TAP SYNC").small().color(AMBER));
                                }
                            });
                        });
                        ui.add_space(6.0);

                        let fsize = vec2(78.0, 40.0);
                        ui.horizontal(|ui| {
                            let r = key(ui, "PROGRAM", c.program_mode && blink, fsize)
                                .on_hover_text("Hold 3 seconds to enter or leave program mode");
                            self.program_hold_ui(ui, &r, c, t);
                            if key(
                                ui,
                                "MIDI/ADD",
                                c.pending_add || (c.editing_chase.is_some() && blink),
                                fsize,
                            )
                            .clicked()
                            {
                                c.press_midi_add();
                                touched = true;
                            }
                            if key(
                                ui,
                                "AUTO/DEL",
                                c.mode == Mode::Auto || c.pending_delete,
                                fsize,
                            )
                            .clicked()
                            {
                                c.press_auto_del();
                                touched = true;
                            }
                            if key(
                                ui,
                                "MUSIC/\nBANK COPY",
                                c.mode == Mode::Music || c.bank_copy_source.is_some(),
                                fsize,
                            )
                            .clicked()
                            {
                                c.press_music_bank_copy();
                                touched = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            if key(ui, "BANK UP", false, fsize).clicked() {
                                c.bank_up();
                            }
                            if key(ui, "BANK DOWN", false, fsize).clicked() {
                                c.bank_down();
                            }
                            let tap_led =
                                c.mode == Mode::Auto && (t / c.step_seconds()).fract() < 0.15;
                            if key(ui, "TAP/\nDISPLAY", tap_led, fsize).clicked() {
                                c.press_tap();
                            }
                            if key(ui, "BLACKOUT", c.blackout && blink, fsize).clicked() {
                                c.press_blackout();
                            }
                        });
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(format!("SCENES · BANK {:02}", c.bank + 1))
                                        .small()
                                        .weak(),
                                );
                                let arm_blink = (c.pending_add || c.pending_delete) && blink;
                                for row in 0..2 {
                                    ui.horizontal(|ui| {
                                        for s in row * SCENES_PER_BANK / 2
                                            ..(row + 1) * SCENES_PER_BANK / 2
                                        {
                                            let stored = c.show.scenes[c.bank][s].is_some();
                                            let active = c
                                                .active_scene
                                                .is_some_and(|a| a.bank == c.bank && a.scene == s);
                                            let label = if stored {
                                                format!("{}•", s + 1)
                                            } else {
                                                format!("{}", s + 1)
                                            };
                                            if key(
                                                ui,
                                                &label,
                                                active || arm_blink,
                                                vec2(40.0, 34.0),
                                            )
                                            .on_hover_text(if stored {
                                                "Stored scene"
                                            } else {
                                                "Empty"
                                            })
                                            .clicked()
                                            {
                                                c.press_scene(s);
                                                touched = true;
                                            }
                                        }
                                    });
                                }
                                ui.label(RichText::new("CHASES").small().weak());
                                ui.horizontal(|ui| {
                                    for ch in 0..CHASES {
                                        let lit = c.active_chases[ch]
                                            || (c.editing_chase == Some(ch) && blink);
                                        let n = c.show.chases[ch].len();
                                        if key(
                                            ui,
                                            &format!("{}\n{n}", ch + 1),
                                            lit,
                                            vec2(40.0, 40.0),
                                        )
                                        .on_hover_text(format!("Chase {} · {n} steps", ch + 1))
                                        .clicked()
                                        {
                                            c.press_chase(ch);
                                            touched = true;
                                        }
                                    }
                                });
                            });
                            ui.spacing_mut().slider_width = 120.0;
                            ui.vertical(|ui| {
                                ui.label(RichText::new("SPEED").small());
                                let mut v = c.show.speed;
                                if ui
                                    .add(
                                        Slider::new(&mut v, 0.0..=1.0).vertical().show_value(false),
                                    )
                                    .changed()
                                {
                                    c.set_speed(v);
                                    touched = true;
                                }
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new("FADE").small());
                                let mut v = c.show.fade;
                                if ui
                                    .add(
                                        Slider::new(&mut v, 0.0..=1.0).vertical().show_value(false),
                                    )
                                    .changed()
                                {
                                    c.set_fade(v);
                                    touched = true;
                                }
                            });
                        });
                    });
                });
            });
        if touched {
            self.dirty = true;
        }
    }

    /// PROGRAM toggles after being held for 3 seconds, like the hardware.
    fn program_hold_ui(&mut self, ui: &mut Ui, r: &egui::Response, c: &mut Controller, t: f64) {
        if r.is_pointer_button_down_on() {
            let start = *self.program_hold.get_or_insert(t);
            let k = ((t - start) / PROGRAM_HOLD_SECONDS).min(1.0) as f32;
            let bar = Rect::from_min_size(
                r.rect.left_bottom() - vec2(0.0, 4.0),
                vec2(r.rect.width() * k, 4.0),
            );
            ui.painter().rect_filled(bar, 0.0, AMBER);
            if k >= 1.0 && !self.program_hold_fired {
                c.toggle_program();
                self.program_hold_fired = true;
            }
        } else {
            self.program_hold = None;
            self.program_hold_fired = false;
        }
    }

    fn keyboard(&mut self, ui: &Ui, c: &mut Controller) {
        if ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        ui.input(|i| {
            let scene_keys = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
            ];
            for (s, k) in scene_keys.into_iter().enumerate() {
                if i.key_pressed(k) {
                    c.press_scene(s);
                    self.dirty = true;
                }
            }
            if i.key_pressed(Key::Space) {
                c.beat();
            }
            if i.key_pressed(Key::B) {
                c.press_blackout();
            }
            if i.key_pressed(Key::T) {
                c.press_tap();
            }
            if i.key_pressed(Key::ArrowUp) {
                c.bank_up();
            }
            if i.key_pressed(Key::ArrowDown) {
                c.bank_down();
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let t = self.now();
        let controller = self.controller.clone();
        let mut c = controller.lock().unwrap();

        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::right("side")
            .default_size(340.0)
            .resizable(true)
            .show(ui, |ui| self.side_panel(ui, &mut c));
        egui::Panel::bottom("front")
            .resizable(false)
            .show(ui, |ui| self.front_panel(ui, &mut c, t));
        egui::CentralPanel::default_margins().show(ui, |ui| {
            let output = c.output;
            if let Some(f) = self
                .visualizer
                .show(ui, &output, &self.patch, &c.selected, t)
            {
                c.press_fixture(f);
            }
        });
        self.keyboard(ui, &mut c);
        drop(c);

        // Autosave a second after the last change.
        if self.dirty && t - self.last_save > 1.0 {
            self.save();
        }
        ui.ctx().request_repaint();
    }
}

/// A front-panel key with a status LED.
fn key(ui: &mut Ui, label: &str, led: bool, size: Vec2) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let fill = if r.is_pointer_button_down_on() {
        KEY_HOVER.gamma_multiply(0.8)
    } else if r.hovered() {
        KEY_HOVER
    } else {
        KEY
    };
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_black_alpha(120)),
        egui::StrokeKind::Inside,
    );
    p.circle_filled(
        rect.right_top() + vec2(-7.0, 7.0),
        3.0,
        if led { LED_ON } else { LED_OFF },
    );
    p.text(
        rect.center() + vec2(0.0, 2.0),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0),
        Color32::from_gray(225),
    );
    r
}

fn led_label(ui: &mut Ui, text: &str, on: bool) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter()
            .circle_filled(rect.center(), 4.0, if on { LED_ON } else { LED_OFF });
        ui.label(RichText::new(text).small());
    });
}

/// 12 x 16 grid of the 192 output channels.
fn dmx_monitor(ui: &mut Ui, output: &[u8]) {
    let cell = vec2(18.0, 14.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(cell.x * 16.0 + 26.0, cell.y * 12.0), Sense::hover());
    let p = ui.painter();
    for f in 0..FIXTURES {
        p.text(
            rect.min + vec2(0.0, cell.y * (f as f32 + 0.5)),
            Align2::LEFT_CENTER,
            format!("{}", f + 1),
            FontId::monospace(9.0),
            Color32::from_gray(140),
        );
        for ch in 0..CHANNELS_PER_FIXTURE {
            let v = output[f * CHANNELS_PER_FIXTURE + ch];
            let min = rect.min + vec2(26.0 + cell.x * ch as f32, cell.y * f as f32);
            let r = Rect::from_min_size(min, cell - vec2(2.0, 2.0));
            p.rect_filled(r, 1.0, Color32::from_rgb(20, 22, 28));
            let h = r.height() * v as f32 / 255.0;
            p.rect_filled(
                Rect::from_min_max(egui::pos2(r.min.x, r.max.y - h), r.max),
                1.0,
                AMBER.gamma_multiply(0.85),
            );
        }
    }
    if let Some(pos) = response.hover_pos() {
        let rel = pos - rect.min - vec2(26.0, 0.0);
        if rel.x >= 0.0 {
            let (ch, f) = ((rel.x / cell.x) as usize, (rel.y / cell.y) as usize);
            if ch < 16 && f < FIXTURES {
                let addr = f * CHANNELS_PER_FIXTURE + ch;
                response.on_hover_text(format!("DMX {} = {}", addr + 1, output[addr]));
            }
        }
    }
}

const HELP: &str = "\
Program mode: hold PROGRAM for 3 s (LED blinks). Hold again to leave.

Build a look: tap scanner buttons to select fixtures, move the faders. \
PAGE SELECT switches the faders between channels 1-8 and 9-16.

Store a scene: press MIDI/ADD, then a scene button (1-8). BANK UP/DOWN picks one of 30 banks.

Edit a scene: in program mode press its scene button, adjust, then MIDI/ADD + scene.

Delete a scene: in program mode press AUTO/DEL, then the scene button.

Copy a bank: go to the source bank, press MUSIC/BANK COPY, go to the target bank, press MIDI/ADD.

Program a chase: in program mode press a chase button (1-6). Choose a bank and press a \
scene button to load it, then MIDI/ADD to append it as a step. AUTO/DEL removes the last step. \
Press the chase button again when done. To erase a chase: AUTO/DEL, then the chase button.

Play: press scene buttons for manual playback. AUTO/DEL runs the bank (or the selected chases) at the \
SPEED fader rate; TAP/DISPLAY sets the rate by tapping. MUSIC runs on the beat (mic, or Space). \
FADE sets the crossfade time. BLACKOUT kills the output.

Keys: 1-8 scenes · Up/Down bank · T tap · B blackout · Space beat.";
