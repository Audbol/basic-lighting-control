//! The window: a status bar, the stage, a tabbed side panel (patch, output,
//! monitor, show file, help) and the Obey 40 front panel along the bottom.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Key as K, Layout, Margin, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui, epaint::RectShape, pos2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::engine::{
    CHANNELS_PER_FIXTURE, CHASES, Controller, FADERS, FIXTURES, Mode, Page, SCENES_PER_BANK, Show,
    fixture_address,
};
use crate::fixtures::{DEFAULT_PATCH, Profile};
use crate::output::Output;
use crate::theme::{
    self, ACCENT, BG, CARD_EDGE, LED_BLUE, LED_GREEN, LED_RED, TEXT, TEXT_DIM, TEXT_FAINT, WELL,
    caption, card, vcard,
};
use crate::visualizer::Visualizer;
use crate::widgets::{Key, chip, fader, glow_dot, seven_segment};

const PROGRAM_HOLD_SECONDS: f64 = 3.0;
const FADER_SIZE: egui::Vec2 = egui::Vec2::new(50.0, 176.0);

/// What a show file holds: the programming plus the patch.
#[derive(Serialize, Deserialize)]
struct ShowFile {
    show: Show,
    patch: [Profile; FIXTURES],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Patch,
    Output,
    Monitor,
    Show,
    Help,
}

pub struct App {
    controller: Arc<Mutex<Controller>>,
    output: Output,
    epoch: Instant,
    patch: [Profile; FIXTURES],
    visualizer: Visualizer,
    tab: Tab,
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
        theme::apply(&cc.egui_ctx);
        let epoch = Instant::now();
        let controller = Arc::new(Mutex::new(Controller::default()));
        let output = Output::start(controller.clone(), epoch);
        let mut app = Self {
            controller,
            output,
            epoch,
            patch: DEFAULT_PATCH,
            visualizer: Visualizer { haze: true },
            tab: Tab::Patch,
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
            app.status = "Demo show loaded".into();
        }
        app
    }

    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    fn show_name(&self) -> String {
        Path::new(&self.show_path)
            .file_name()
            .map(|n| {
                n.to_string_lossy()
                    .trim_end_matches(".json")
                    .trim_end_matches(".show")
                    .to_string()
            })
            .unwrap_or_else(|| "untitled".into())
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
            Ok(()) => "Saved".into(),
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
                self.status = "Loaded".into();
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    // ---- status bar ---------------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut Ui, c: &Controller) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("OBEY").size(18.0).strong().color(ACCENT));
            ui.add_space(-6.0);
            ui.label(RichText::new("LITE").size(18.0).color(TEXT));
            ui.label(RichText::new("DMX · 192").size(11.0).color(TEXT_FAINT));
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new(self.show_name()).color(TEXT).strong())
                .on_hover_text(&self.show_path);
            let saved = if self.dirty {
                "editing…"
            } else {
                self.status.as_str()
            };
            ui.label(RichText::new(saved).size(11.0).color(TEXT_FAINT));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let status = self.output.status.lock().unwrap().clone();
                let enttec_on = status.enttec.starts_with("sending");
                let artnet_on = status.artnet.starts_with("sending");
                if c.blackout {
                    chip(ui, "BLACKOUT", LED_RED, (self.now() * 2.5).fract() < 0.5);
                }
                #[cfg(feature = "audio")]
                chip(ui, "MIC", LED_BLUE, self.mic.is_some());
                #[cfg(feature = "enttec")]
                chip(ui, "USB DMX", LED_GREEN, enttec_on).on_hover_text(&status.enttec);
                chip(ui, "ART-NET", LED_GREEN, artnet_on).on_hover_text(&status.artnet);
                let (mode, color) = if c.program_mode {
                    ("PROGRAM", ACCENT)
                } else {
                    match c.mode {
                        Mode::Auto => ("AUTO", LED_GREEN),
                        Mode::Music => ("MUSIC", LED_BLUE),
                        Mode::Manual => ("MANUAL", TEXT_DIM),
                    }
                };
                chip(ui, mode, color, true);
                let _ = enttec_on;
            });
        });
    }

    // ---- side panel ----------------------------------------------------------------------------

    fn side_panel(&mut self, ui: &mut Ui, c: &mut Controller) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (tab, name) in [
                (Tab::Patch, "Patch"),
                (Tab::Output, "Output"),
                (Tab::Monitor, "Monitor"),
                (Tab::Show, "Show"),
                (Tab::Help, "Help"),
            ] {
                if tab_button(ui, name, self.tab == tab).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match self.tab {
                Tab::Patch => self.patch_tab(ui, c),
                Tab::Output => self.output_tab(ui),
                Tab::Monitor => {
                    card().show(ui, |ui| {
                        caption(ui, "DMX output");
                        ui.label(
                            RichText::new("Hover a cell for its address and value.")
                                .size(11.0)
                                .color(TEXT_DIM),
                        );
                        ui.add_space(6.0);
                        dmx_monitor(ui, &c.output, &self.patch);
                    });
                }
                Tab::Show => self.show_tab(ui),
                Tab::Help => {
                    card().show(ui, |ui| {
                        caption(ui, "Programming like an Obey 40");
                        ui.add_space(4.0);
                        for (title, body) in HELP {
                            ui.label(RichText::new(*title).strong().color(TEXT));
                            ui.label(RichText::new(*body).color(TEXT_DIM));
                            ui.add_space(6.0);
                        }
                    });
                }
            });
    }

    fn patch_tab(&mut self, ui: &mut Ui, c: &mut Controller) {
        card().show(ui, |ui| {
            caption(ui, "Fixture patch");
            ui.label(
                RichText::new("Each scanner owns 16 channels from its start address. Set your fixture to that address.")
                    .size(11.0)
                    .color(TEXT_DIM),
            );
            ui.add_space(6.0);
            egui::Grid::new("patch").num_columns(3).spacing([10.0, 6.0]).show(ui, |ui| {
                for f in 0..FIXTURES {
                    let sel = c.selected[f];
                    let label = RichText::new(format!("{:>2}", f + 1)).monospace().strong();
                    if ui
                        .add(egui::Button::new(if sel { label.color(Color32::BLACK) } else { label }).fill(if sel {
                            ACCENT
                        } else {
                            Color32::from_rgb(38, 41, 50)
                        }))
                        .on_hover_text("Select this scanner")
                        .clicked()
                    {
                        c.press_fixture(f);
                    }
                    ui.label(RichText::new(format!("DMX {:03}", fixture_address(f))).monospace().color(TEXT_DIM));
                    egui::ComboBox::from_id_salt(("profile", f))
                        .width(170.0)
                        .selected_text(self.patch[f].name())
                        .show_ui(ui, |ui| {
                            for p in Profile::ALL {
                                if ui.selectable_value(&mut self.patch[f], p, p.name()).changed() {
                                    self.dirty = true;
                                }
                            }
                        });
                    ui.end_row();
                }
            });
        });
    }

    fn output_tab(&mut self, ui: &mut Ui) {
        let status = self.output.status.lock().unwrap().clone();
        let mut cfg = self.output.config.lock().unwrap();
        card().show(ui, |ui| {
            ui.horizontal(|ui| {
                caption(ui, "Art-Net");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    toggle(ui, &mut cfg.artnet_enabled);
                });
            });
            ui.add_space(4.0);
            egui::Grid::new("artnet")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Target").color(TEXT_DIM));
                    let valid = self.artnet_ip.parse::<Ipv4Addr>().is_ok();
                    let edit = egui::TextEdit::singleline(&mut self.artnet_ip)
                        .desired_width(160.0)
                        .font(egui::TextStyle::Monospace)
                        .text_color(if valid { TEXT } else { LED_RED });
                    if ui.add(edit).changed()
                        && let Ok(ip) = self.artnet_ip.parse()
                    {
                        cfg.artnet_target = ip;
                    }
                    ui.end_row();
                    ui.label("");
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
                    ui.end_row();
                    ui.label(RichText::new("Universe").color(TEXT_DIM));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut cfg.artnet_universe).range(0..=32767));
                        let u = cfg.artnet_universe;
                        ui.label(
                            RichText::new(format!(
                                "net {} · sub {} · uni {}",
                                u >> 8,
                                (u >> 4) & 0xf,
                                u & 0xf
                            ))
                            .size(11.0)
                            .color(TEXT_FAINT),
                        );
                    });
                    ui.end_row();
                });
            ui.add_space(4.0);
            status_line(ui, &status.artnet, status.artnet.starts_with("sending"));
        });

        #[cfg(feature = "enttec")]
        {
            ui.add_space(8.0);
            card().show(ui, |ui| {
                caption(ui, "ENTTEC DMX USB Pro");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("serial")
                        .width(180.0)
                        .selected_text(if self.serial_choice.is_empty() {
                            "Choose port…"
                        } else {
                            &self.serial_choice
                        })
                        .show_ui(ui, |ui| {
                            if self.serial_ports.is_empty() {
                                ui.label(RichText::new("No ports found").color(TEXT_DIM));
                            }
                            for p in &self.serial_ports {
                                ui.selectable_value(&mut self.serial_choice, p.clone(), p);
                            }
                        });
                    if ui.small_button("Rescan").clicked() {
                        self.serial_ports = crate::enttec::list_ports();
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.serial_choice)
                            .hint_text("/dev/ttyUSB0 or COM3")
                            .font(egui::TextStyle::Monospace)
                            .desired_width(180.0),
                    );
                    if cfg.enttec_port.is_some() {
                        if ui.button("Disconnect").clicked() {
                            cfg.enttec_port = None;
                        }
                    } else if ui
                        .add_enabled(!self.serial_choice.is_empty(), egui::Button::new("Connect"))
                        .clicked()
                    {
                        cfg.enttec_port = Some(self.serial_choice.clone());
                    }
                });
                ui.add_space(4.0);
                status_line(
                    ui,
                    if status.enttec.is_empty() {
                        "off"
                    } else {
                        &status.enttec
                    },
                    status.enttec.starts_with("sending"),
                );
            });
        }

        #[cfg(feature = "audio")]
        {
            ui.add_space(8.0);
            card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    caption(ui, "Microphone (music mode)");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut on = self.mic.is_some();
                        if toggle(ui, &mut on).changed() {
                            if on {
                                match crate::audio::Mic::start(self.controller.clone()) {
                                    Ok(m) => self.mic = Some(m),
                                    Err(e) => self.status = format!("Mic: {e}"),
                                }
                            } else {
                                self.mic = None;
                            }
                        }
                    });
                });
                ui.label(
                    RichText::new(
                        "Steps scenes on the beat in MUSIC mode. Space also triggers a beat.",
                    )
                    .size(11.0)
                    .color(TEXT_DIM),
                );
            });
        }
    }

    fn show_tab(&mut self, ui: &mut Ui) {
        card().show(ui, |ui| {
            caption(ui, "Show file");
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.show_path)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY),
            );
            ui.label(
                RichText::new("Changes save automatically.")
                    .size(11.0)
                    .color(TEXT_FAINT),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Load").clicked() {
                    self.load();
                }
                if !self.confirm_new && ui.button("New show…").clicked() {
                    self.confirm_new = true;
                }
            });
            if self.confirm_new {
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(LED_RED.gamma_multiply(0.12))
                    .stroke(Stroke::new(1.0, LED_RED.gamma_multiply(0.5)))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Erase all scenes and chases?").color(TEXT));
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(RichText::new("Erase").color(Color32::WHITE))
                                        .fill(LED_RED.gamma_multiply(0.7)),
                                )
                                .clicked()
                            {
                                *self.controller.lock().unwrap() = Controller::default();
                                self.confirm_new = false;
                                self.dirty = true;
                                self.status = "New show".into();
                            }
                            if ui.button("Cancel").clicked() {
                                self.confirm_new = false;
                            }
                        });
                    });
            }
        });
    }

    // ---- the front panel ------------------------------------------------------------------------

    fn front_panel(&mut self, ui: &mut Ui, c: &mut Controller, t: f64) {
        let blink = if (t * 2.5).fract() < 0.5 { 1.0 } else { 0.0 };
        let profile = c.first_selected().map(|f| self.patch[f]);
        let mut touched = false;
        let on = |b: bool| if b { 1.0 } else { 0.0 };

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 8.0);

                // ---- scanners and channel faders ----
                vcard(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(5.0, 6.0);
                    caption(ui, "Scanners");
                    ui.horizontal(|ui| {
                        for f in 0..FIXTURES {
                            let sel = c.selected[f];
                            let r = Key::new(&(f + 1).to_string(), vec2(46.0, 42.0))
                                .sub(self.patch[f].short())
                                .lit(sel)
                                .led(on(sel))
                                .led_color(ACCENT)
                                .show(ui)
                                .on_hover_text(format!(
                                    "{} · DMX {}",
                                    self.patch[f].name(),
                                    fixture_address(f)
                                ));
                            if r.clicked() {
                                c.press_fixture(f);
                            }
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        caption(
                            ui,
                            &format!(
                                "Channels {}",
                                if c.page == Page::A { "1–8" } else { "9–16" }
                            ),
                        );
                        if c.first_selected().is_none() {
                            ui.label(
                                RichText::new("select a scanner to use the faders")
                                    .size(11.0)
                                    .color(TEXT_FAINT),
                            );
                        }
                    });
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        for f in 0..FADERS {
                            let ch = c.fader_channel(f);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.set_width(FADER_SIZE.x + 6.0);
                                let role = profile.map_or("—".into(), |p| p.channel_label(ch));
                                centered_text(
                                    ui,
                                    &format!("{}", ch + 1),
                                    FontId::monospace(13.0),
                                    TEXT,
                                    FADER_SIZE.x + 6.0,
                                );
                                centered_text(
                                    ui,
                                    &role,
                                    FontId::proportional(10.0),
                                    ACCENT,
                                    FADER_SIZE.x + 6.0,
                                );
                                let v = c.faders[f] as f32 / 255.0;
                                let color = if c.first_selected().is_some() {
                                    ACCENT
                                } else {
                                    TEXT_FAINT
                                };
                                if let Some(nv) =
                                    fader(ui, egui::Id::unique(("fader", f)), v, FADER_SIZE, color)
                                {
                                    c.move_fader(f, (nv * 255.0).round() as u8);
                                    touched = true;
                                }
                                readout(ui, &format!("{:3}", c.faders[f]), FADER_SIZE.x + 6.0);
                            });
                        }
                        ui.vertical(|ui| {
                            ui.add_space(40.0);
                            if Key::new("PAGE", vec2(64.0, 46.0))
                                .sub("SELECT")
                                .show(ui)
                                .clicked()
                            {
                                c.toggle_page();
                            }
                            ui.add_space(6.0);
                            led_row(ui, "A  1–8", c.page == Page::A, LED_GREEN);
                            led_row(ui, "B  9–16", c.page == Page::B, LED_GREEN);
                        });
                    });
                });

                // ---- display and function keys ----
                vcard(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    caption(ui, "Display");
                    ui.horizontal(|ui| {
                        seven_segment(ui, &c.display(), 46.0, LED_RED);
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            let tap_flash =
                                c.mode == Mode::Auto && (t / c.step_seconds()).fract() < 0.12;
                            led_row(ui, "PROGRAM", c.program_mode && blink > 0.5, LED_RED);
                            led_row(ui, "AUTO", c.mode == Mode::Auto, LED_GREEN);
                            led_row(ui, "MUSIC", c.mode == Mode::Music, LED_BLUE);
                            led_row(ui, "STEP", tap_flash, ACCENT);
                            led_row(ui, "BLACKOUT", c.blackout && blink > 0.5, LED_RED);
                        });
                    });
                    ui.horizontal(|ui| {
                        stat(ui, "STEP", &format_seconds(c.step_seconds()));
                        stat(ui, "FADE", &format_seconds(c.fade_seconds()));
                        if c.tap_seconds.is_some() {
                            stat(ui, "SYNC", "TAP");
                        }
                    });
                    ui.add_space(2.0);
                    caption(ui, "Functions");
                    let fsize = vec2(84.0, 46.0);
                    egui::Grid::new("fn").spacing([6.0, 6.0]).show(ui, |ui| {
                        let r = Key::new("PROGRAM", fsize)
                            .sub("HOLD 3 s")
                            .led(on(c.program_mode) * blink)
                            .lit(c.program_mode)
                            .hold()
                            .show(ui);
                        self.program_hold_ui(ui, &r, c, t);
                        if Key::new("MIDI", fsize)
                            .sub("ADD")
                            .led(on(c.pending_add) + on(c.editing_chase.is_some()) * blink)
                            .show(ui)
                            .clicked()
                        {
                            c.press_midi_add();
                            touched = true;
                        }
                        if Key::new("AUTO", fsize)
                            .sub("DEL")
                            .led(on(c.mode == Mode::Auto || c.pending_delete))
                            .led_color(if c.pending_delete { LED_RED } else { LED_GREEN })
                            .show(ui)
                            .clicked()
                        {
                            c.press_auto_del();
                            touched = true;
                        }
                        if Key::new("MUSIC", fsize)
                            .sub("BANK COPY")
                            .led(on(c.mode == Mode::Music || c.bank_copy_source.is_some()))
                            .led_color(if c.bank_copy_source.is_some() {
                                LED_RED
                            } else {
                                LED_BLUE
                            })
                            .show(ui)
                            .clicked()
                        {
                            c.press_music_bank_copy();
                            touched = true;
                        }
                        ui.end_row();
                        if Key::new("BANK", fsize).sub("UP").show(ui).clicked() {
                            c.bank_up();
                        }
                        if Key::new("BANK", fsize).sub("DOWN").show(ui).clicked() {
                            c.bank_down();
                        }
                        let tap_flash =
                            c.mode == Mode::Auto && (t / c.step_seconds()).fract() < 0.12;
                        if Key::new("TAP", fsize)
                            .sub("DISPLAY")
                            .led(on(tap_flash))
                            .led_color(ACCENT)
                            .show(ui)
                            .clicked()
                        {
                            c.press_tap();
                        }
                        if Key::new("BLACKOUT", fsize)
                            .led(on(c.blackout) * blink)
                            .lit(c.blackout)
                            .show(ui)
                            .clicked()
                        {
                            c.press_blackout();
                        }
                        ui.end_row();
                    });
                });

                // ---- scenes and chases ----
                vcard(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    ui.horizontal(|ui| {
                        caption(ui, "Scenes");
                        ui.label(
                            RichText::new(format!("BANK {:02}", c.bank + 1))
                                .monospace()
                                .strong()
                                .color(ACCENT),
                        );
                    });
                    let armed = (c.pending_add || c.pending_delete) && blink > 0.5;
                    egui::Grid::new("scenes")
                        .spacing([6.0, 6.0])
                        .show(ui, |ui| {
                            for s in 0..SCENES_PER_BANK {
                                let stored = c.show.scenes[c.bank][s].is_some();
                                let active = c
                                    .active_scene
                                    .is_some_and(|a| a.bank == c.bank && a.scene == s);
                                let r = Key::new(&(s + 1).to_string(), vec2(54.0, 46.0))
                                    .marker(stored)
                                    .lit(active)
                                    .led(on(active || armed))
                                    .led_color(if c.pending_delete { LED_RED } else { LED_GREEN })
                                    .show(ui)
                                    .on_hover_text(if stored {
                                        "Stored scene"
                                    } else {
                                        "Empty scene"
                                    });
                                if r.clicked() {
                                    c.press_scene(s);
                                    touched = true;
                                }
                                if s % 4 == 3 {
                                    ui.end_row();
                                }
                            }
                        });
                    ui.add_space(4.0);
                    caption(ui, "Chases");
                    egui::Grid::new("chases")
                        .spacing([6.0, 6.0])
                        .show(ui, |ui| {
                            for ch in 0..CHASES {
                                let n = c.show.chases[ch].len();
                                let editing = c.editing_chase == Some(ch);
                                let running = c.active_chases[ch];
                                let sub = if n == 0 {
                                    "empty".to_string()
                                } else {
                                    format!("{n} steps")
                                };
                                let r = Key::new(&(ch + 1).to_string(), vec2(74.0, 40.0))
                                    .sub(&sub)
                                    .lit(running)
                                    .led(on(running) + on(editing) * blink)
                                    .led_color(if editing { LED_RED } else { LED_GREEN })
                                    .show(ui);
                                if r.clicked() {
                                    c.press_chase(ch);
                                    touched = true;
                                }
                                if ch % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                });

                // ---- speed and fade ----
                vcard(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        for (name, value, color, is_speed) in [
                            ("SPEED", c.show.speed as f32, LED_GREEN, true),
                            ("FADE", c.show.fade as f32, LED_BLUE, false),
                        ] {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.set_width(FADER_SIZE.x + 10.0);
                                centered_text(
                                    ui,
                                    name,
                                    FontId::proportional(11.0),
                                    TEXT_FAINT,
                                    FADER_SIZE.x + 10.0,
                                );
                                ui.add_space(14.0);
                                if let Some(nv) =
                                    fader(ui, egui::Id::unique(name), value, FADER_SIZE, color)
                                {
                                    if is_speed {
                                        c.set_speed(nv as f64);
                                    } else {
                                        c.set_fade(nv as f64);
                                    }
                                    touched = true;
                                }
                                let secs = if is_speed {
                                    c.step_seconds()
                                } else {
                                    c.fade_seconds()
                                };
                                readout(ui, &format_seconds(secs), FADER_SIZE.x + 10.0);
                            });
                        }
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
            let track = Rect::from_min_max(
                r.rect.left_bottom() + vec2(8.0, -6.0),
                r.rect.right_bottom() + vec2(-8.0, -3.0),
            );
            let p = ui.painter();
            p.rect_filled(track, 2.0, WELL);
            let bar = Rect::from_min_size(track.min, vec2(track.width() * k, track.height()));
            p.add(RectShape::filled(bar, 2.0, ACCENT.gamma_multiply(0.6)).with_blur_width(6.0));
            p.rect_filled(bar, 2.0, ACCENT);
            if k >= 1.0 && !self.program_hold_fired {
                c.toggle_program();
                self.program_hold_fired = true;
                self.dirty = true;
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
                K::Num1,
                K::Num2,
                K::Num3,
                K::Num4,
                K::Num5,
                K::Num6,
                K::Num7,
                K::Num8,
            ];
            for (s, k) in scene_keys.into_iter().enumerate() {
                if i.key_pressed(k) {
                    c.press_scene(s);
                    self.dirty = true;
                }
            }
            if i.key_pressed(K::Space) {
                c.beat();
            }
            if i.key_pressed(K::B) {
                c.press_blackout();
            }
            if i.key_pressed(K::T) {
                c.press_tap();
            }
            if i.key_pressed(K::ArrowUp) {
                c.bank_up();
            }
            if i.key_pressed(K::ArrowDown) {
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

        egui::Panel::top("top")
            .exact_size(46.0)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(14, 0))
                    .stroke(Stroke::new(1.0, CARD_EDGE)),
            )
            .show(ui, |ui| self.top_bar(ui, &c));
        egui::Panel::bottom("front")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::same(10))
                    .stroke(Stroke::new(1.0, CARD_EDGE)),
            )
            .show(ui, |ui| self.front_panel(ui, &mut c, t));
        egui::Panel::right("side")
            .default_size(360.0)
            .size_range(300.0..=520.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| self.side_panel(ui, &mut c));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::Frame::new()
                .fill(BG)
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        caption(ui, "Stage");
                        ui.label(
                            RichText::new("click a fixture to select it")
                                .size(11.0)
                                .color(TEXT_FAINT),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            toggle(ui, &mut self.visualizer.haze);
                            ui.label(RichText::new("Haze").color(TEXT_DIM));
                        });
                    });
                    ui.add_space(4.0);
                    let output = c.output;
                    egui::Frame::new()
                        .corner_radius(10.0)
                        .stroke(Stroke::new(1.0, CARD_EDGE))
                        .show(ui, |ui| {
                            if let Some(f) =
                                self.visualizer
                                    .show(ui, &output, &self.patch, &c.selected, t)
                            {
                                c.press_fixture(f);
                            }
                        });
                });
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

// ---- small pieces ------------------------------------------------------------------------------

fn format_seconds(s: f64) -> String {
    if s < 10.0 {
        format!("{s:.2} s")
    } else if s < 60.0 {
        format!("{s:.1} s")
    } else {
        format!("{}:{:02}", (s / 60.0) as u32, (s % 60.0) as u32)
    }
}

fn centered_text(ui: &mut Ui, text: &str, font: FontId, color: Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, font.size + 3.0), Sense::hover());
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, text, font, color);
}

/// Value readout in a small recessed window.
fn readout(ui: &mut Ui, text: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
    let r = rect.shrink2(vec2(4.0, 1.0));
    ui.painter().rect_filled(r, 4.0, WELL);
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::monospace(11.5),
        TEXT,
    );
}

fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(10.0).color(TEXT_FAINT));
    ui.label(RichText::new(value).monospace().color(TEXT));
    ui.add_space(6.0);
}

fn led_row(ui: &mut Ui, text: &str, on: bool, color: Color32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
        glow_dot(
            ui.painter(),
            rect.center(),
            3.5,
            color,
            if on { 1.0 } else { 0.0 },
        );
        ui.label(
            RichText::new(text)
                .size(10.5)
                .color(if on { TEXT } else { TEXT_DIM }),
        );
    });
}

fn status_line(ui: &mut Ui, text: &str, ok: bool) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
        glow_dot(
            ui.painter(),
            rect.center(),
            3.5,
            LED_GREEN,
            if ok { 1.0 } else { 0.0 },
        );
        ui.label(
            RichText::new(text)
                .size(11.0)
                .color(if ok { TEXT } else { TEXT_DIM }),
        );
    });
}

/// An iOS-style switch.
fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let (rect, mut r) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let k = ui.ctx().animate_bool_with_time(r.id, *on, 0.12);
    let p = ui.painter();
    let track = Color32::from_rgb(52, 56, 68).lerp_to_gamma(ACCENT, k);
    p.rect_filled(rect, 10.0, track);
    let x = egui::lerp(rect.left() + 10.0..=rect.right() - 10.0, k);
    p.circle_filled(
        pos2(x, rect.center().y),
        8.0,
        Color32::from_rgb(240, 240, 244),
    );
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn tab_button(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let font = FontId::proportional(12.5);
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        font,
        if selected { TEXT } else { TEXT_DIM },
    );
    let (rect, r) = ui.allocate_exact_size(vec2(galley.size().x + 22.0, 28.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(r.id, r.hovered() || selected, 0.1);
    let p = ui.painter();
    p.rect_filled(rect, 7.0, Color32::from_white_alpha((14.0 * hover) as u8));
    if selected {
        p.rect_filled(
            Rect::from_min_max(
                pos2(rect.min.x + 8.0, rect.max.y - 2.0),
                pos2(rect.max.x - 8.0, rect.max.y),
            ),
            1.0,
            ACCENT,
        );
    }
    p.galley(rect.center() - galley.size() / 2.0, galley, TEXT);
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// 12 x 16 grid of the 192 output channels, rows labelled by fixture.
fn dmx_monitor(ui: &mut Ui, output: &[u8], patch: &[Profile; FIXTURES]) {
    let label_w = 44.0;
    let cell = vec2(
        ((ui.available_width() - label_w) / 16.0).clamp(12.0, 22.0),
        16.0,
    );
    let (rect, response) =
        ui.allocate_exact_size(vec2(label_w + cell.x * 16.0, cell.y * 12.0), Sense::hover());
    let p = ui.painter();
    for f in 0..FIXTURES {
        p.text(
            rect.min + vec2(0.0, cell.y * (f as f32 + 0.5)),
            Align2::LEFT_CENTER,
            format!("{:>2} {}", f + 1, patch[f].short()),
            FontId::monospace(9.0),
            TEXT_DIM,
        );
        for ch in 0..CHANNELS_PER_FIXTURE {
            let v = output[f * CHANNELS_PER_FIXTURE + ch];
            let min = rect.min + vec2(label_w + cell.x * ch as f32, cell.y * f as f32);
            let r = Rect::from_min_size(min, cell - vec2(2.0, 2.0));
            p.rect_filled(r, 2.0, WELL);
            if v > 0 {
                let k = v as f32 / 255.0;
                let hgt = r.height() * k;
                p.rect_filled(
                    Rect::from_min_max(pos2(r.min.x, r.max.y - hgt), r.max),
                    2.0,
                    ACCENT.gamma_multiply(0.35 + 0.65 * k),
                );
            }
            p.rect_stroke(
                r,
                2.0,
                Stroke::new(1.0, Color32::from_white_alpha(6)),
                StrokeKind::Inside,
            );
        }
    }
    if let Some(pos) = response.hover_pos() {
        let rel = pos - rect.min - vec2(label_w, 0.0);
        if rel.x >= 0.0 {
            let (ch, f) = ((rel.x / cell.x) as usize, (rel.y / cell.y) as usize);
            if ch < 16 && f < FIXTURES {
                let addr = f * CHANNELS_PER_FIXTURE + ch;
                response.on_hover_text(format!(
                    "DMX {}  ·  scanner {} ch {} ({})\nvalue {}",
                    addr + 1,
                    f + 1,
                    ch + 1,
                    patch[f].channel_label(ch),
                    output[addr]
                ));
            }
        }
    }
}

const HELP: &[(&str, &str)] = &[
    (
        "Program mode",
        "Hold PROGRAM for 3 seconds (its LED blinks). Hold again to leave.",
    ),
    (
        "Build a look",
        "Tap scanner buttons to select fixtures, then move the faders. PAGE SELECT switches between channels 1–8 and 9–16. Scroll a fader for fine steps; double-click it to snap to full or zero.",
    ),
    (
        "Store a scene",
        "MIDI/ADD, then a scene button. BANK UP/DOWN picks one of 30 banks.",
    ),
    (
        "Edit a scene",
        "In program mode press the scene, adjust, then MIDI/ADD and the scene again.",
    ),
    (
        "Delete a scene",
        "In program mode press AUTO/DEL, then the scene.",
    ),
    (
        "Copy a bank",
        "Go to the source bank, press MUSIC/BANK COPY, go to the target bank, press MIDI/ADD.",
    ),
    (
        "Program a chase",
        "In program mode press a chase button. Load a scene with its button, then MIDI/ADD to append it as a step. AUTO/DEL removes the last step. Press the chase again when done. To erase a chase: AUTO/DEL, then the chase.",
    ),
    (
        "Play",
        "Scene buttons play scenes. AUTO runs the bank, or the chases you select, at the SPEED rate; TAP sets the rate by tapping. MUSIC steps on the beat. FADE sets the crossfade. BLACKOUT kills the output.",
    ),
    (
        "Keys",
        "1–8 scenes · ↑/↓ bank · T tap · B blackout · Space beat",
    ),
];
