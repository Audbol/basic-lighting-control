//! Fixture profiles: what each of the (up to 16) channels of a scanner slot does.
//! The visualizer renders from these; the fader strip uses them as labels.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Dimmer,
    Red,
    Green,
    Blue,
    White,
    Strobe,
    Macro,
    Speed,
    Pan,
    PanFine,
    Tilt,
    TiltFine,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Dimmer => "Dim",
            Role::Red => "Red",
            Role::Green => "Green",
            Role::Blue => "Blue",
            Role::White => "White",
            Role::Strobe => "Strobe",
            Role::Macro => "Macro",
            Role::Speed => "Speed",
            Role::Pan => "Pan",
            Role::PanFine => "Pan F",
            Role::Tilt => "Tilt",
            Role::TiltFine => "Tilt F",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    Empty,
    Dimmer,
    Rgb3,
    Rgb7,
    Rgbw8,
    MovingHead,
    Strobe,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    None,
    Par,
    Mover,
    Strobe,
    Generic,
}

impl Profile {
    pub const ALL: [Profile; 8] = [
        Profile::Empty,
        Profile::Dimmer,
        Profile::Rgb3,
        Profile::Rgb7,
        Profile::Rgbw8,
        Profile::MovingHead,
        Profile::Strobe,
        Profile::Generic,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Profile::Empty => "Empty",
            Profile::Dimmer => "Dimmer (1ch)",
            Profile::Rgb3 => "RGB Par (3ch)",
            Profile::Rgb7 => "RGB Par (7ch)",
            Profile::Rgbw8 => "RGBW Par (8ch)",
            Profile::MovingHead => "Moving Head (11ch)",
            Profile::Strobe => "Strobe (2ch)",
            Profile::Generic => "Generic (16ch)",
        }
    }

    /// Short tag for the scanner buttons.
    pub fn short(self) -> &'static str {
        match self {
            Profile::Empty => "—",
            Profile::Dimmer => "DIM",
            Profile::Rgb3 | Profile::Rgb7 => "RGB",
            Profile::Rgbw8 => "RGBW",
            Profile::MovingHead => "MOVER",
            Profile::Strobe => "STROBE",
            Profile::Generic => "GEN",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Profile::Empty => Kind::None,
            Profile::Dimmer | Profile::Rgb3 | Profile::Rgb7 | Profile::Rgbw8 => Kind::Par,
            Profile::MovingHead => Kind::Mover,
            Profile::Strobe => Kind::Strobe,
            Profile::Generic => Kind::Generic,
        }
    }

    pub fn channels(self) -> &'static [Role] {
        use Role::*;
        match self {
            Profile::Empty | Profile::Generic => &[],
            Profile::Dimmer => &[Dimmer],
            Profile::Rgb3 => &[Red, Green, Blue],
            Profile::Rgb7 => &[Dimmer, Red, Green, Blue, Strobe, Macro, Speed],
            Profile::Rgbw8 => &[Dimmer, Red, Green, Blue, White, Strobe, Macro, Speed],
            Profile::MovingHead => &[
                Pan, PanFine, Tilt, TiltFine, Speed, Dimmer, Strobe, Red, Green, Blue, White,
            ],
            Profile::Strobe => &[Dimmer, Strobe],
        }
    }

    /// Fader label for channel `ch` (0..16) of this profile.
    pub fn channel_label(self, ch: usize) -> String {
        match self {
            Profile::Generic => format!("Ch {}", ch + 1),
            _ => self
                .channels()
                .get(ch)
                .map_or("—".into(), |r| r.label().into()),
        }
    }
}

/// A starter rig: movers on the ends, colour pars across the truss, a strobe and a dimmer.
pub const DEFAULT_PATCH: [Profile; 12] = [
    Profile::MovingHead,
    Profile::Rgbw8,
    Profile::Rgbw8,
    Profile::Rgb7,
    Profile::Rgb7,
    Profile::Strobe,
    Profile::Dimmer,
    Profile::Rgb7,
    Profile::Rgb7,
    Profile::Rgbw8,
    Profile::Rgbw8,
    Profile::MovingHead,
];

/// One fixture's 16 channel values, interpreted for rendering. All 0..1.
#[derive(Clone, Copy, Debug)]
pub struct FixtureState {
    pub kind: Kind,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub dimmer: f32,
    /// 0 = open; above that, the flash rate rises with the value.
    pub strobe: f32,
    pub pan: f32,
    pub tilt: f32,
}

pub fn fixture_state(profile: Profile, values: &[u8]) -> FixtureState {
    let roles = profile.channels();
    let get = |role: Role, fallback: f32| {
        roles
            .iter()
            .position(|&r| r == role)
            .map_or(fallback, |i| values[i] as f32 / 255.0)
    };
    // Dimmers and strobes are white light; colour fixtures without a dimmer run at full.
    let base = if roles.contains(&Role::Red) { 0.0 } else { 1.0 };
    let white = get(Role::White, 0.0);
    FixtureState {
        kind: profile.kind(),
        r: (get(Role::Red, base) + white).min(1.0),
        g: (get(Role::Green, base) + white).min(1.0),
        b: (get(Role::Blue, base) + white).min(1.0),
        dimmer: get(Role::Dimmer, 1.0),
        strobe: get(Role::Strobe, 0.0),
        pan: get(Role::Pan, 0.5) + get(Role::PanFine, 0.0) / 256.0,
        tilt: get(Role::Tilt, 0.5) + get(Role::TiltFine, 0.0) / 256.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgbw_par_reads_its_channels() {
        let mut v = [0u8; 16];
        v[0] = 255; // dimmer
        v[1] = 255; // red
        v[4] = 51; // white
        let s = fixture_state(Profile::Rgbw8, &v);
        assert_eq!(s.dimmer, 1.0);
        assert_eq!(s.r, 1.0);
        assert!((s.g - 0.2).abs() < 1e-6);
        assert_eq!(s.strobe, 0.0);
    }

    #[test]
    fn dimmer_is_white() {
        let s = fixture_state(Profile::Dimmer, &[128; 16]);
        assert_eq!((s.r, s.g, s.b), (1.0, 1.0, 1.0));
        assert!((s.dimmer - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn labels() {
        assert_eq!(Profile::MovingHead.channel_label(2), "Tilt");
        assert_eq!(Profile::Rgb3.channel_label(5), "—");
        assert_eq!(Profile::Generic.channel_label(15), "Ch 16");
    }
}
