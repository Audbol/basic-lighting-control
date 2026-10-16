# Obey-Lite

A desktop DMX lighting controller, written in Rust, that works like a **Chauvet Obey 40**. It has a live stage visualizer and outputs **Art-Net** or to an **ENTTEC DMX USB Pro**.

## What it copies from the Obey 40

| | Obey 40 | Obey-Lite |
|---|---|---|
| Channels | 192 (12 scanners × 16 ch) | same, scanner *n* starts at DMX address 16·(n−1)+1 |
| Faders | 8, page A = ch 1-8, page B = ch 9-16 | same |
| Scenes | 30 banks × 8 scenes | same |
| Chases | 6, up to 240 steps | same |
| Speed / fade | speed 0.1 s … 10 min, fade time | same (fade 0 … 30 s), plus tap sync |
| Modes | manual, auto, music (built-in mic), blackout | same; music uses your mic (`--features audio`) or the Space bar |
| Display | 4-digit LED | 4-character readout: bank, values, `Sto`, `dEL`, `Prog`… |

### Programming

* **Program mode**: hold **PROGRAM** for 3 seconds (the LED blinks). Hold again to leave.
* **Build a look**: tap scanner buttons to select fixtures (several at once is fine), move the faders. **PAGE SELECT** flips between channels 1-8 and 9-16. Fader labels follow the patched fixture type (Pan, Tilt, Red, …).
* **Store a scene**: **MIDI/ADD**, then a scene button. **BANK UP/DOWN** picks the bank.
* **Edit a scene**: in program mode press its scene button, adjust, then **MIDI/ADD** + scene.
* **Delete a scene**: in program mode **AUTO/DEL**, then the scene button.
* **Copy a bank**: go to the source bank, **MUSIC/BANK COPY**, go to the target bank, **MIDI/ADD**.
* **Program a chase**: in program mode press a chase button (1-6). Pick a bank and press a scene button to load it, then **MIDI/ADD** to append it as a step. **AUTO/DEL** removes the last step. Press the chase button again when done. To erase a chase: **AUTO/DEL**, then the chase button.

### Playing

* Press a scene button for manual playback.
* **AUTO/DEL** runs the current bank's scenes, or the selected chases (press chase buttons; several run one after another), at the **SPEED** fader rate. **TAP/DISPLAY** sets the rate by tapping.
* **MUSIC/BANK COPY** steps on the beat (microphone, or the Space bar).
* **FADE** sets the crossfade time. **BLACKOUT** kills the output.

Faders: drag the cap, click the track to jump, scroll for single DMX steps (Shift for ten), double-click to snap to full or zero. When a scene recalls, the faders glide to their new positions like motorised faders.

Keyboard: `1`-`8` scenes · `Up`/`Down` bank · `T` tap · `B` blackout · `Space` beat.

On the hardware some functions need two buttons held at once (AUTO/DEL + scene). With a mouse, press them one after the other; the button's LED shows it's armed.

## Visualizer and patch

The stage view shows the 12 scanner slots on a box truss, with soft beams through drifting haze, pools and spill on the floor, and light on the back wall. Moving heads turn to follow pan/tilt, strobes flash at the strobe-channel rate, hovering a fixture shows its address and live values, and clicking it selects it. The **Patch** panel sets each slot's fixture type:

* Dimmer (1ch) · RGB Par (3ch) · RGB Par (7ch: dim R G B strobe macro speed)
* RGBW Par (8ch: dim R G B W strobe macro speed)
* Moving Head (11ch: pan panF tilt tiltF speed dim strobe R G B W)
* Strobe (2ch: dim strobe) · Generic (16ch, output only)

Set each real fixture's DMX address to the slot's start address (shown in the patch list: 1, 17, 33, …, 177) and choose a matching channel mode on the fixture.

The **DMX monitor** shows all 192 output channels; hover a cell to see its address and value.

## Output

* **Art-Net**: tick *Art-Net*, set the target IP (broadcast `255.255.255.255`, `2.255.255.255`, `10.255.255.255`, or a node's own IP) and the universe (15-bit Port-Address; the net/sub-net/universe split is shown). Frames go out at 40 Hz from UDP port 6454.
* **ENTTEC DMX USB Pro** (and compatibles): pick the serial port, or type it (`/dev/ttyUSB0`, `COM3`), and press *Connect*.

Both can run at once. Output runs on its own thread, so chases keep running when the window is minimised.

## Shows

Scenes, chases, speed/fade and the patch are saved as JSON to the show file (by default `default.show.json` in your user data folder, see Download; or pass a path: `obey-lite myshow.show.json`). Changes are autosaved; **Save** / **Load** / **New** are in the top bar. On first run a demo show is loaded: six looks in bank 1, a colour chase on chase 1 and a mover sweep on chase 2.

## Download

Every push is built by GitHub Actions ([Build workflow](../../actions/workflows/build.yml)); open a run and download the package for your system from **Artifacts**. Tagged versions (`v*`) are also published under [Releases](../../releases).

| Package | For |
|---|---|
| `obey-lite-windows-x86_64.zip` | Windows 10/11 (Intel/AMD) |
| `obey-lite-windows-arm64.zip` | Windows on ARM |
| `obey-lite-macos-universal.zip` | macOS 11+, Apple silicon and Intel |
| `obey-lite-linux-x86_64.tar.gz` | Linux PCs |
| `obey-lite-raspberry-pi-arm64.tar.gz` | Raspberry Pi OS 64-bit (Pi 3, 4, 5, Zero 2 W) |
| `obey-lite-raspberry-pi-armhf.tar.gz` | Raspberry Pi OS 32-bit (Pi 2, 3, 4) |

All builds include the microphone (music mode).

* **Windows**: unzip and run `obey-lite.exe`. SmartScreen may warn because the exe isn't signed: *More info → Run anyway*.
* **macOS**: unzip and move `Obey-Lite.app` to Applications. The app isn't notarized, so the first time right-click it → *Open* (or run `xattr -dr com.apple.quarantine /Applications/Obey-Lite.app`).
* **Linux / Raspberry Pi**: `tar xzf obey-lite-*.tar.gz`, then run `./obey-lite` in the folder, or `./install.sh` to install it to `~/.local/bin` with a menu entry. Needs a desktop session and `libasound2` (present on Raspberry Pi OS Desktop). A Pi 4 or 5 is recommended for a smooth visualizer.

Without a path argument the show is kept in your user data folder: `%APPDATA%\Obey-Lite` (Windows), `~/Library/Application Support/Obey-Lite` (macOS), `~/.local/share/obey-lite` (Linux).

## Build from source

Needs a stable Rust toolchain (edition 2024).

```sh
cargo run --release                     # Art-Net + ENTTEC
cargo run --release --features audio    # also the microphone for music mode
cargo test
```

On Linux, `--features audio` needs the ALSA headers (`libasound2-dev` / `alsa-lib-devel`). The GUI uses OpenGL, so it runs on most machines, VMs included.

## Layout

| File | |
|---|---|
| `src/engine.rs` | The Obey 40 logic: modes, banks, scenes, chases, fades, tap. No GUI or I/O, unit tested |
| `src/app.rs` | Window layout: status bar, stage, side tabs, front panel |
| `src/widgets.rs`, `src/theme.rs` | Panel keys, faders, 7-segment display; colours and style |
| `src/visualizer.rs` | Stage rendering |
| `src/fixtures.rs` | Fixture profiles |
| `src/output.rs` | 40 Hz DMX thread |
| `src/artnet.rs` | ArtDmx packets over UDP |
| `src/enttec.rs` | ENTTEC USB Pro framing |
| `src/beat.rs`, `src/audio.rs` | Beat detection, microphone input |
| `src/demo.rs` | First-run demo show |
