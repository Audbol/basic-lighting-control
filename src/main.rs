//! Obey-Lite: a 192-channel DMX lighting controller that works like a
//! Chauvet Obey 40, with a stage visualizer and Art-Net / ENTTEC output.
//!
//! Usage: obey-lite [SHOW_FILE]
//! Without a path the show lives in the per-user data folder (see `default_show_path`).

// No console window behind the GUI in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

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
        .unwrap_or_else(default_show_path);
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

/// `default.show.json` in the platform's per-user data folder:
/// `%APPDATA%\Obey-Lite` on Windows, `~/Library/Application Support/Obey-Lite` on macOS,
/// `$XDG_DATA_HOME/obey-lite` (or `~/.local/share/obey-lite`) elsewhere. Falls back to
/// the working directory if none of those are known.
fn default_show_path() -> PathBuf {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(windows) {
        env("APPDATA").map(|d| d.join("Obey-Lite"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/Obey-Lite"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("obey-lite"))
    };
    match dir {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => dir.join("default.show.json"),
        _ => PathBuf::from("default.show.json"),
    }
}
