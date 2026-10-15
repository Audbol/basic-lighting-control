//! Controller engine modelled on the Chauvet Obey 40 (DMX-40):
//!
//! * 12 scanners x 16 channels = 192 DMX channels
//! * 8 faders; page A = channels 1-8, page B = channels 9-16
//! * 30 banks x 8 scenes = 240 scenes
//! * 6 chases of up to 240 steps
//! * speed fader (0.1 s .. 10 min), fade-time fader (0 .. 30 s), tap sync, blackout
//!
//! Pure logic, no GUI or I/O: the front panel calls the `press_*` / `move_*`
//! methods and calls [`Controller::tick`] every frame to get the output.

use serde::{Deserialize, Serialize};

pub const FIXTURES: usize = 12;
pub const CHANNELS_PER_FIXTURE: usize = 16;
pub const CHANNELS: usize = FIXTURES * CHANNELS_PER_FIXTURE; // 192
pub const FADERS: usize = 8;
pub const BANKS: usize = 30;
pub const SCENES_PER_BANK: usize = 8;
pub const CHASES: usize = 6;
pub const MAX_CHASE_STEPS: usize = 240;

pub const MIN_STEP_SECONDS: f64 = 0.1;
pub const MAX_STEP_SECONDS: f64 = 600.0;
pub const MAX_FADE_SECONDS: f64 = 30.0;

pub type Look = [u8; CHANNELS];

/// Speed fader position (0..1) -> step time in seconds, on a log scale.
pub fn speed_to_seconds(v: f64) -> f64 {
    MIN_STEP_SECONDS * (MAX_STEP_SECONDS / MIN_STEP_SECONDS).powf(v.clamp(0.0, 1.0))
}

pub fn seconds_to_speed(s: f64) -> f64 {
    let s = s.clamp(MIN_STEP_SECONDS, MAX_STEP_SECONDS);
    (s / MIN_STEP_SECONDS).ln() / (MAX_STEP_SECONDS / MIN_STEP_SECONDS).ln()
}

/// Fade fader position (0..1) -> crossfade time in seconds.
pub fn fade_to_seconds(v: f64) -> f64 {
    let v = v.clamp(0.0, 1.0);
    v * v * MAX_FADE_SECONDS
}

/// 1-based DMX start address of a scanner (0-based index), fixed at 16-channel spacing.
pub fn fixture_address(fixture: usize) -> usize {
    fixture * CHANNELS_PER_FIXTURE + 1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    A,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Manual,
    Auto,
    Music,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneRef {
    pub bank: usize,
    pub scene: usize,
}

/// The stored programming: what a show file holds.
#[derive(Clone, Serialize, Deserialize)]
pub struct Show {
    #[serde(with = "looks_2d")]
    pub scenes: Vec<Vec<Option<Look>>>,
    #[serde(with = "looks_2d_steps")]
    pub chases: Vec<Vec<Look>>,
    pub speed: f64,
    pub fade: f64,
}

impl Default for Show {
    fn default() -> Self {
        Self {
            scenes: vec![vec![None; SCENES_PER_BANK]; BANKS],
            chases: vec![Vec::new(); CHASES],
            speed: seconds_to_speed(2.0),
            fade: 0.0,
        }
    }
}

impl Show {
    /// Brings a loaded show to the exact bank/scene/chase shape the panel expects.
    pub fn normalize(mut self) -> Self {
        self.scenes.resize(BANKS, vec![None; SCENES_PER_BANK]);
        for bank in &mut self.scenes {
            bank.resize(SCENES_PER_BANK, None);
        }
        self.chases.resize(CHASES, Vec::new());
        for steps in &mut self.chases {
            steps.truncate(MAX_CHASE_STEPS);
        }
        self.speed = self.speed.clamp(0.0, 1.0);
        self.fade = self.fade.clamp(0.0, 1.0);
        self
    }
}

pub struct Controller {
    pub show: Show,
    pub program_mode: bool,
    pub page: Page,
    pub bank: usize,
    /// Selected scanners; bit i = scanner i.
    pub selected: [bool; FIXTURES],
    pub faders: [u8; FADERS],
    /// The live / programmer look.
    pub buffer: Look,
    pub mode: Mode,
    pub active_scene: Option<SceneRef>,
    pub active_chases: [bool; CHASES],
    /// Chase being programmed.
    pub editing_chase: Option<usize>,
    /// MIDI/ADD pressed, waiting for a scene button.
    pub pending_add: bool,
    /// AUTO/DEL pressed in program mode, waiting for a scene or chase button.
    pub pending_delete: bool,
    /// Source bank while a bank copy is armed.
    pub bank_copy_source: Option<usize>,
    pub blackout: bool,
    /// Tap tempo overrides the speed fader until the fader moves.
    pub tap_seconds: Option<f64>,
    taps: Vec<f64>,
    step_index: Option<usize>,
    next_step_at: f64,
    pending_beat: bool,
    fade_from: Look,
    fade_start: f64,
    fade_duration: f64,
    pub output: Look,
    message: Option<(String, f64)>,
    pub now: f64,
}

impl Default for Controller {
    fn default() -> Self {
        Self::new(Show::default())
    }
}

impl Controller {
    pub fn new(show: Show) -> Self {
        Self {
            show: show.normalize(),
            program_mode: false,
            page: Page::A,
            bank: 0,
            selected: [false; FIXTURES],
            faders: [0; FADERS],
            buffer: [0; CHANNELS],
            mode: Mode::Manual,
            active_scene: None,
            active_chases: [false; CHASES],
            editing_chase: None,
            pending_add: false,
            pending_delete: false,
            bank_copy_source: None,
            blackout: false,
            tap_seconds: None,
            taps: Vec::new(),
            step_index: None,
            next_step_at: 0.0,
            pending_beat: false,
            fade_from: [0; CHANNELS],
            fade_start: 0.0,
            fade_duration: 0.0,
            output: [0; CHANNELS],
            message: None,
            now: 0.0,
        }
    }

    // ---- display ----------------------------------------------------------------------

    fn flash(&mut self, text: impl Into<String>, seconds: f64) {
        self.message = Some((text.into(), self.now + seconds));
    }

    /// The 4-character LED readout.
    pub fn display(&self) -> String {
        if let Some((text, until)) = &self.message
            && self.now < *until
        {
            return text.clone();
        }
        if let Some(src) = self.bank_copy_source {
            return format!("C{:02}", src + 1);
        }
        if let Some(c) = self.editing_chase {
            return format!("C{}{:02}", c + 1, self.show.chases[c].len().min(99));
        }
        if self.blackout {
            return "bLAc".into();
        }
        format!("b {:02}", self.bank + 1)
    }

    pub fn step_seconds(&self) -> f64 {
        self.tap_seconds
            .unwrap_or_else(|| speed_to_seconds(self.show.speed))
    }

    pub fn fade_seconds(&self) -> f64 {
        fade_to_seconds(self.show.fade)
    }

    /// Channel (0..16) that fader `f` drives on the current page.
    pub fn fader_channel(&self, f: usize) -> usize {
        f + if self.page == Page::B { FADERS } else { 0 }
    }

    pub fn first_selected(&self) -> Option<usize> {
        self.selected.iter().position(|&s| s)
    }

    // ---- program mode -------------------------------------------------------------------

    /// The hardware toggles program mode on a 3-second hold of PROGRAM; the panel times the hold.
    pub fn toggle_program(&mut self) {
        self.program_mode = !self.program_mode;
        self.pending_add = false;
        self.pending_delete = false;
        self.bank_copy_source = None;
        self.editing_chase = None;
        if self.program_mode {
            self.stop_playback();
            self.flash("Prog", 0.9);
        } else {
            self.flash("run ", 0.9);
        }
    }

    // ---- scanners & faders --------------------------------------------------------------

    pub fn press_fixture(&mut self, i: usize) {
        if i >= FIXTURES {
            return;
        }
        self.selected[i] = !self.selected[i];
        self.sync_faders();
    }

    /// Faders follow the lowest selected scanner, as motorised faders would.
    pub fn sync_faders(&mut self) {
        let first = self.first_selected();
        for f in 0..FADERS {
            self.faders[f] = match first {
                Some(fx) => self.buffer[fx * CHANNELS_PER_FIXTURE + self.fader_channel(f)],
                None => 0,
            };
        }
    }

    pub fn move_fader(&mut self, f: usize, value: u8) {
        if f >= FADERS {
            return;
        }
        self.faders[f] = value;
        let ch = self.fader_channel(f);
        let mut any = false;
        for fx in 0..FIXTURES {
            if self.selected[fx] {
                self.buffer[fx * CHANNELS_PER_FIXTURE + ch] = value;
                any = true;
            }
        }
        if any {
            self.flash(format!("{value:>4}"), 0.6);
        } else {
            self.flash("SEL ", 0.6);
        }
    }

    pub fn toggle_page(&mut self) {
        self.page = if self.page == Page::A {
            Page::B
        } else {
            Page::A
        };
        self.sync_faders();
        self.flash(if self.page == Page::A { "PG A" } else { "PG B" }, 0.9);
    }

    // ---- banks --------------------------------------------------------------------------

    pub fn bank_up(&mut self) {
        self.bank = (self.bank + 1) % BANKS;
        self.after_bank_change();
    }

    pub fn bank_down(&mut self) {
        self.bank = (self.bank + BANKS - 1) % BANKS;
        self.after_bank_change();
    }

    fn after_bank_change(&mut self) {
        // An auto/music run over the bank's scenes follows the bank change.
        if self.mode != Mode::Manual && !self.any_chase_active() {
            self.step_index = None;
        }
    }

    fn any_chase_active(&self) -> bool {
        self.active_chases.iter().any(|&a| a)
    }

    // ---- function buttons ---------------------------------------------------------------

    /// MIDI/ADD: in program mode, arms a scene store, appends a chase step, or
    /// completes a bank copy.
    pub fn press_midi_add(&mut self) {
        if !self.program_mode {
            return;
        }
        if let Some(src) = self.bank_copy_source {
            self.copy_bank(src, self.bank);
            return;
        }
        if let Some(c) = self.editing_chase {
            let steps = &mut self.show.chases[c];
            if steps.len() >= MAX_CHASE_STEPS {
                self.flash("FULL", 0.9);
            } else {
                steps.push(self.buffer);
                let n = steps.len();
                self.flash(format!("S{n:03}"), 0.9);
            }
            return;
        }
        self.pending_add = !self.pending_add;
        self.pending_delete = false;
    }

    /// AUTO/DEL: auto run in run mode; the delete modifier in program mode.
    pub fn press_auto_del(&mut self) {
        if self.program_mode {
            if let Some(c) = self.editing_chase {
                // Removes the last step of the chase being programmed.
                if self.show.chases[c].pop().is_some() {
                    self.flash("dEL ", 0.9);
                }
            } else {
                self.pending_delete = !self.pending_delete;
                self.pending_add = false;
            }
            return;
        }
        self.set_mode(if self.mode == Mode::Auto {
            Mode::Manual
        } else {
            Mode::Auto
        });
    }

    /// MUSIC/BANK COPY: sound-active run in run mode; arms a bank copy in program mode.
    pub fn press_music_bank_copy(&mut self) {
        if self.program_mode {
            if self.editing_chase.is_some() {
                return;
            }
            self.bank_copy_source = match self.bank_copy_source {
                None => Some(self.bank),
                Some(_) => None,
            };
            self.pending_add = false;
            self.pending_delete = false;
            return;
        }
        self.set_mode(if self.mode == Mode::Music {
            Mode::Manual
        } else {
            Mode::Music
        });
    }

    pub fn copy_bank(&mut self, from: usize, to: usize) {
        self.show.scenes[to] = self.show.scenes[from].clone();
        self.bank_copy_source = None;
        self.flash(format!("CP{:02}", to + 1), 0.9);
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.step_index = None;
        self.next_step_at = self.now;
        if mode == Mode::Manual {
            self.active_chases = [false; CHASES];
        }
        self.flash(
            match mode {
                Mode::Auto => "AUtO",
                Mode::Music => "Snd ",
                Mode::Manual => "MAn ",
            },
            0.9,
        );
    }

    fn stop_playback(&mut self) {
        self.mode = Mode::Manual;
        self.active_chases = [false; CHASES];
        self.step_index = None;
    }

    pub fn press_scene(&mut self, i: usize) {
        if i >= SCENES_PER_BANK {
            return;
        }
        let here = SceneRef {
            bank: self.bank,
            scene: i,
        };

        if self.program_mode && self.pending_delete {
            self.show.scenes[self.bank][i] = None;
            self.pending_delete = false;
            if self.active_scene == Some(here) {
                self.active_scene = None;
            }
            self.flash("dEL ", 0.9);
            return;
        }

        if self.program_mode && self.pending_add {
            self.show.scenes[self.bank][i] = Some(self.buffer);
            self.pending_add = false;
            self.active_scene = Some(here);
            self.flash("Sto ", 0.9);
            return;
        }

        // Recall. In program mode this loads the scene for editing (or as a chase
        // step); in run mode it plays the scene and ends an auto/music run.
        if !self.program_mode && self.mode != Mode::Manual {
            self.stop_playback();
        }
        let look = self.show.scenes[self.bank][i].unwrap_or([0; CHANNELS]);
        self.recall(look);
        self.active_scene = Some(here);
    }

    pub fn press_chase(&mut self, i: usize) {
        if i >= CHASES {
            return;
        }
        if self.program_mode {
            if self.pending_delete {
                self.show.chases[i].clear();
                self.pending_delete = false;
                self.flash("dEL ", 0.9);
            } else {
                self.editing_chase = if self.editing_chase == Some(i) {
                    None
                } else {
                    Some(i)
                };
                self.pending_add = false;
                self.bank_copy_source = None;
            }
            return;
        }
        self.active_chases[i] = !self.active_chases[i];
        if self.any_chase_active() && self.mode == Mode::Manual {
            self.mode = Mode::Auto;
        }
        self.step_index = None;
        self.next_step_at = self.now;
    }

    pub fn press_blackout(&mut self) {
        self.blackout = !self.blackout;
    }

    /// TAP/DISPLAY: two or more taps set the step time.
    pub fn press_tap(&mut self) {
        let now = self.now;
        self.taps.retain(|&t| now - t < 4.0);
        self.taps.push(now);
        if self.taps.len() >= 2 {
            let gaps: Vec<f64> = self.taps.windows(2).map(|w| w[1] - w[0]).collect();
            let avg = gaps.iter().sum::<f64>() / gaps.len() as f64;
            let s = avg.max(MIN_STEP_SECONDS);
            self.tap_seconds = Some(s);
            self.flash(format!("{s:.2}").chars().take(4).collect::<String>(), 0.9);
        }
        if self.taps.len() > 4 {
            self.taps.remove(0);
        }
    }

    pub fn set_speed(&mut self, v: f64) {
        self.show.speed = v.clamp(0.0, 1.0);
        self.tap_seconds = None;
        let s = self.step_seconds();
        let text = if s < 10.0 {
            format!("{s:.2}").chars().take(4).collect()
        } else if s < 60.0 {
            format!("{:>3}S", s.round())
        } else {
            format!("{:>3}M", (s / 60.0).round())
        };
        self.flash(text, 0.6);
    }

    pub fn set_fade(&mut self, v: f64) {
        self.show.fade = v.clamp(0.0, 1.0);
        let text: String = format!("F{:.1}", self.fade_seconds())
            .chars()
            .take(4)
            .collect();
        self.flash(text, 0.6);
    }

    /// Sound-active trigger (microphone beat, or a manual beat key).
    pub fn beat(&mut self) {
        if self.mode == Mode::Music {
            self.pending_beat = true;
        }
    }

    // ---- playback ---------------------------------------------------------------------------

    fn recall(&mut self, look: Look) {
        self.fade_from = self.output;
        self.fade_start = self.now;
        self.fade_duration = self.fade_seconds();
        self.buffer = look;
        self.sync_faders();
    }

    /// What an auto/music run steps through: the active chases in order, otherwise
    /// the stored scenes of the current bank.
    fn playlist(&self) -> Vec<Look> {
        if self.any_chase_active() {
            (0..CHASES)
                .filter(|&c| self.active_chases[c])
                .flat_map(|c| self.show.chases[c].iter().copied())
                .collect()
        } else {
            self.show.scenes[self.bank]
                .iter()
                .flatten()
                .copied()
                .collect()
        }
    }

    fn step(&mut self) {
        let list = self.playlist();
        if list.is_empty() {
            return;
        }
        let idx = self.step_index.map_or(0, |i| (i + 1) % list.len());
        self.step_index = Some(idx);
        self.recall(list[idx]);
        self.active_scene = if self.any_chase_active() {
            None
        } else {
            // Light the scene LED of the stored scene being played.
            self.show.scenes[self.bank]
                .iter()
                .enumerate()
                .filter(|(_, s)| s.is_some())
                .nth(idx)
                .map(|(scene, _)| SceneRef {
                    bank: self.bank,
                    scene,
                })
        };
    }

    /// Advances the clock (seconds) and returns the 192-channel output.
    pub fn tick(&mut self, now: f64) -> &Look {
        self.now = now;
        if !self.program_mode {
            match self.mode {
                Mode::Auto if now >= self.next_step_at => {
                    self.step();
                    self.next_step_at = now + self.step_seconds();
                }
                Mode::Music if self.pending_beat => self.step(),
                _ => {}
            }
        }
        self.pending_beat = false;

        let t = if self.fade_duration > 0.0 {
            ((now - self.fade_start) / self.fade_duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        for c in 0..CHANNELS {
            let from = self.fade_from[c] as f64;
            let v = from + (self.buffer[c] as f64 - from) * t;
            self.output[c] = if self.blackout { 0 } else { v.round() as u8 };
        }
        &self.output
    }
}

// Looks are stored as plain arrays of numbers in the show file.
mod looks_2d {
    use super::{CHANNELS, Look};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &[Vec<Option<Look>>], s: S) -> Result<S::Ok, S::Error> {
        let out: Vec<Vec<Option<&[u8]>>> = v
            .iter()
            .map(|b| b.iter().map(|l| l.as_ref().map(|l| &l[..])).collect())
            .collect();
        out.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<Vec<Option<Look>>>, D::Error> {
        let raw: Vec<Vec<Option<Vec<u8>>>> = Deserialize::deserialize(d)?;
        Ok(raw
            .into_iter()
            .map(|b| b.into_iter().map(|l| l.and_then(to_look)).collect())
            .collect())
    }

    pub fn to_look(v: Vec<u8>) -> Option<Look> {
        (v.len() == CHANNELS).then(|| {
            let mut l = [0; CHANNELS];
            l.copy_from_slice(&v);
            l
        })
    }
}

mod looks_2d_steps {
    use super::Look;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &[Vec<Look>], s: S) -> Result<S::Ok, S::Error> {
        let out: Vec<Vec<&[u8]>> = v
            .iter()
            .map(|c| c.iter().map(|l| &l[..]).collect())
            .collect();
        out.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<Look>>, D::Error> {
        let raw: Vec<Vec<Vec<u8>>> = Deserialize::deserialize(d)?;
        Ok(raw
            .into_iter()
            .map(|c| c.into_iter().filter_map(super::looks_2d::to_look).collect())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn programmed() -> Controller {
        let mut c = Controller::default();
        c.toggle_program();
        c
    }

    #[test]
    fn faders_drive_selected_fixtures_and_pages() {
        let mut c = programmed();
        c.press_fixture(0);
        c.press_fixture(2);
        c.move_fader(0, 200);
        c.toggle_page();
        c.move_fader(1, 77); // page B, fader 2 = channel 10
        assert_eq!(c.buffer[0], 200);
        assert_eq!(c.buffer[32], 200);
        assert_eq!(c.buffer[16], 0, "unselected scanner untouched");
        assert_eq!(c.buffer[9], 77);
        assert_eq!(c.buffer[32 + 9], 77);
    }

    #[test]
    fn record_recall_and_delete_scene() {
        let mut c = programmed();
        c.press_fixture(1);
        c.move_fader(3, 128);
        c.bank_up();
        c.press_midi_add();
        c.press_scene(4);
        assert_eq!(c.show.scenes[1][4].unwrap()[16 + 3], 128);

        c.toggle_program();
        c.move_fader(3, 0);
        c.press_scene(4);
        assert_eq!(c.tick(1.0)[16 + 3], 128);
        assert_eq!(c.faders[3], 128, "faders follow the recalled look");

        c.toggle_program();
        c.press_auto_del();
        c.press_scene(4);
        assert!(c.show.scenes[1][4].is_none());
    }

    #[test]
    fn scene_buttons_do_not_store_outside_program_mode() {
        let mut c = Controller::default();
        c.press_midi_add();
        c.press_scene(0);
        assert!(c.show.scenes[0][0].is_none());
    }

    #[test]
    fn chase_programming_and_playback() {
        let mut c = programmed();
        c.press_fixture(0);
        c.press_chase(2);
        for v in [10, 20, 30] {
            c.move_fader(0, v);
            c.press_midi_add();
        }
        assert_eq!(c.show.chases[2].len(), 3);
        c.press_auto_del(); // removes the last step while editing
        assert_eq!(c.show.chases[2].len(), 2);
        c.toggle_program();

        c.set_speed(seconds_to_speed(1.0));
        c.press_chase(2);
        assert_eq!(c.mode, Mode::Auto);
        assert_eq!(c.tick(10.0)[0], 10);
        assert_eq!(c.tick(10.5)[0], 10);
        assert_eq!(c.tick(11.0)[0], 20);
        assert_eq!(c.tick(12.0)[0], 10, "wraps around");
    }

    #[test]
    fn auto_runs_the_bank_and_music_steps_on_beats() {
        let mut c = programmed();
        c.press_fixture(0);
        for (scene, v) in [(1, 50), (5, 60)] {
            c.move_fader(0, v);
            c.press_midi_add();
            c.press_scene(scene);
        }
        c.toggle_program();
        c.press_music_bank_copy();
        assert_eq!(c.mode, Mode::Music);
        c.tick(0.0);
        c.beat();
        assert_eq!(c.tick(0.1)[0], 50);
        assert_eq!(c.active_scene, Some(SceneRef { bank: 0, scene: 1 }));
        assert_eq!(c.tick(5.0)[0], 50, "no beat, no step");
        c.beat();
        assert_eq!(c.tick(5.1)[0], 60);
        assert_eq!(c.active_scene, Some(SceneRef { bank: 0, scene: 5 }));
    }

    #[test]
    fn bank_copy() {
        let mut c = programmed();
        c.press_fixture(5);
        c.move_fader(0, 99);
        c.press_midi_add();
        c.press_scene(7);
        c.press_music_bank_copy();
        for _ in 0..4 {
            c.bank_up();
        }
        c.press_midi_add();
        assert_eq!(c.show.scenes[4][7].unwrap()[80], 99);
    }

    #[test]
    fn fade_and_blackout() {
        let mut c = programmed();
        c.press_fixture(0);
        c.move_fader(0, 200);
        c.press_midi_add();
        c.press_scene(0);
        c.toggle_program();
        c.press_scene(1); // empty scene -> all zero
        c.tick(0.0);
        c.set_fade(((2.0 / MAX_FADE_SECONDS) as f64).sqrt()); // 2 s fade
        c.tick(1.0);
        c.press_scene(0);
        assert_eq!(c.tick(2.0)[0], 100, "halfway through the fade");
        assert_eq!(c.tick(3.0)[0], 200);
        c.press_blackout();
        assert_eq!(c.tick(3.1)[0], 0);
    }

    #[test]
    fn tap_tempo_overrides_speed_fader() {
        let mut c = Controller::default();
        for t in [0.0, 0.5, 1.0, 1.5] {
            c.now = t;
            c.press_tap();
        }
        assert!((c.step_seconds() - 0.5).abs() < 1e-9);
        c.set_speed(seconds_to_speed(3.0));
        assert!((c.step_seconds() - 3.0).abs() < 1e-6);
    }

    #[test]
    fn show_round_trips_through_json() {
        let mut c = programmed();
        c.press_fixture(11);
        c.move_fader(7, 255);
        c.press_midi_add();
        c.press_scene(3);
        c.press_chase(5);
        c.press_midi_add();
        let json = serde_json::to_string(&c.show).unwrap();
        let back: Show = serde_json::from_str(&json).unwrap();
        assert_eq!(back.scenes[0][3].unwrap()[CHANNELS - 9], 255);
        assert_eq!(back.chases[5].len(), 1);
    }

    #[test]
    fn speed_curve_spans_hardware_range() {
        assert!((speed_to_seconds(0.0) - 0.1).abs() < 1e-9);
        assert!((speed_to_seconds(1.0) - 600.0).abs() < 1e-6);
        assert!((speed_to_seconds(seconds_to_speed(7.5)) - 7.5).abs() < 1e-9);
        assert_eq!(fixture_address(11), 177);
    }
}
