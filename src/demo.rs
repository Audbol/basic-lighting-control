//! A starter show for the default patch, loaded when no show file exists yet:
//! bank 1 holds six looks, chase 1 cycles colours, chase 2 sweeps the movers.

use crate::engine::{CHANNELS, CHANNELS_PER_FIXTURE, FIXTURES, Look, Show};
use crate::fixtures::{Kind, Profile, Role};

fn set(look: &mut Look, patch: &[Profile; FIXTURES], f: usize, role: Role, v: u8) {
    if let Some(ch) = patch[f].channels().iter().position(|&r| r == role) {
        look[f * CHANNELS_PER_FIXTURE + ch] = v;
    }
}

/// Every colour fixture at one colour; movers aimed at (pan, tilt).
fn wash(
    patch: &[Profile; FIXTURES],
    rgb: impl Fn(usize) -> (u8, u8, u8),
    aim: impl Fn(usize) -> (u8, u8),
) -> Look {
    let mut look = [0; CHANNELS];
    for f in 0..FIXTURES {
        let kind = patch[f].kind();
        if kind == Kind::Strobe {
            continue;
        }
        let (r, g, b) = rgb(f);
        set(&mut look, patch, f, Role::Dimmer, 255);
        set(&mut look, patch, f, Role::Red, r);
        set(&mut look, patch, f, Role::Green, g);
        set(&mut look, patch, f, Role::Blue, b);
        if patch[f] == Profile::Dimmer {
            look[f * CHANNELS_PER_FIXTURE] = 90;
        }
        if kind == Kind::Mover {
            let (pan, tilt) = aim(f);
            set(&mut look, patch, f, Role::Pan, pan);
            set(&mut look, patch, f, Role::Tilt, tilt);
        }
    }
    look
}

pub fn demo_show(patch: &[Profile; FIXTURES]) -> Show {
    let mut show = Show::default();
    let centre = |_| (128, 140);
    let cross = |f: usize| {
        if f < FIXTURES / 2 {
            (200, 200)
        } else {
            (56, 200)
        }
    };
    let out = |f: usize| {
        if f < FIXTURES / 2 {
            (40, 90)
        } else {
            (216, 90)
        }
    };

    let mut looks = vec![
        wash(patch, |_| (255, 110, 20), centre),
        wash(
            patch,
            |f| {
                if f % 2 == 0 {
                    (0, 40, 255)
                } else {
                    (220, 0, 200)
                }
            },
            cross,
        ),
        wash(
            patch,
            |f| {
                if f % 2 == 0 {
                    (0, 255, 90)
                } else {
                    (0, 160, 255)
                }
            },
            out,
        ),
        wash(patch, |_| (255, 0, 30), centre),
        wash(
            patch,
            |f| (((f * 40) % 256) as u8, 60, (255 - (f * 20) % 256) as u8),
            cross,
        ),
    ];
    // Full white with the strobe going.
    let mut white = wash(patch, |_| (255, 255, 255), centre);
    for f in 0..FIXTURES {
        if patch[f].kind() == Kind::Strobe {
            set(&mut white, patch, f, Role::Dimmer, 255);
            set(&mut white, patch, f, Role::Strobe, 180);
        }
    }
    looks.push(white);
    for (s, look) in looks.iter().enumerate() {
        show.scenes[0][s] = Some(*look);
    }

    let colours = [
        (255, 0, 0),
        (255, 140, 0),
        (0, 255, 0),
        (0, 200, 255),
        (0, 0, 255),
        (200, 0, 255),
    ];
    show.chases[0] = colours
        .iter()
        .map(|&c| wash(patch, move |_| c, centre))
        .collect();
    show.chases[1] = [(30, 60), (128, 200), (225, 60), (128, 120)]
        .iter()
        .map(|&(p, t)| {
            wash(
                patch,
                |_| (255, 255, 255),
                move |f| {
                    if f < FIXTURES / 2 {
                        (p, t)
                    } else {
                        (255 - p, t)
                    }
                },
            )
        })
        .collect();
    show
}
