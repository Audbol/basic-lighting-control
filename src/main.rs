//! Obey-Lite: a 192-channel DMX lighting controller that works like a
//! Chauvet Obey 40, with a stage visualizer and Art-Net / ENTTEC output.
//!
//! Usage: obey-lite [SHOW_FILE]    (default: default.show.json)

mod app;
mod artnet;
#[cfg(feature = "audio")]
mod audio;
#[cfg(any(feature = "audio", test))]
mod beat;
mod demo;
mod engine;
#[cfg(feature = "enttec")]
mod enttec;
mod fixtures;
mod output;
mod visualizer;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let show = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "default.show.json".into());
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Obey-Lite DMX Controller")
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([1100.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "obey-lite",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, show)))),
    )
}
