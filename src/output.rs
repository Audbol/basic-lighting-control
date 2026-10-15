//! The DMX clock: a background thread that ticks the controller ~40 times a
//! second and sends the frame to Art-Net and/or an ENTTEC USB Pro. It runs
//! independently of the window, so a show keeps playing when it is minimised.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::artnet::ArtNetSender;
use crate::engine::Controller;

pub const FRAME_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone)]
pub struct OutputConfig {
    pub artnet_enabled: bool,
    pub artnet_target: Ipv4Addr,
    pub artnet_universe: u16,
    pub enttec_port: Option<String>,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            artnet_enabled: false,
            artnet_target: Ipv4Addr::BROADCAST,
            artnet_universe: 0,
            enttec_port: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct OutputStatus {
    pub artnet: String,
    pub enttec: String,
    pub frames: u64,
}

pub struct Output {
    pub config: Arc<Mutex<OutputConfig>>,
    pub status: Arc<Mutex<OutputStatus>>,
}

impl Output {
    pub fn start(controller: Arc<Mutex<Controller>>, epoch: Instant) -> Self {
        let config = Arc::new(Mutex::new(OutputConfig::default()));
        let status = Arc::new(Mutex::new(OutputStatus::default()));
        let (cfg, st) = (config.clone(), status.clone());
        thread::Builder::new()
            .name("dmx-output".into())
            .spawn(move || run(controller, cfg, st, epoch))
            .expect("spawn output thread");
        Self { config, status }
    }
}

fn run(
    controller: Arc<Mutex<Controller>>,
    config: Arc<Mutex<OutputConfig>>,
    status: Arc<Mutex<OutputStatus>>,
    epoch: Instant,
) {
    let mut artnet: Option<ArtNetSender> = None;
    #[cfg(feature = "enttec")]
    let mut enttec: Option<(String, crate::enttec::UsbPro)> = None;
    let mut universe = [0u8; 512];
    let mut next = Instant::now();

    loop {
        {
            let mut c = controller.lock().unwrap();
            let out = c.tick(epoch.elapsed().as_secs_f64());
            universe[..out.len()].copy_from_slice(out);
        }
        let cfg = config.lock().unwrap().clone();
        let mut st = status.lock().unwrap().clone();
        st.frames += 1;

        if cfg.artnet_enabled {
            if artnet.is_none() {
                match ArtNetSender::new() {
                    Ok(s) => artnet = Some(s),
                    Err(e) => st.artnet = format!("socket error: {e}"),
                }
            }
            if let Some(sender) = &mut artnet {
                st.artnet = match sender.send(cfg.artnet_target, cfg.artnet_universe, &universe) {
                    Ok(()) => format!(
                        "sending to {} universe {}",
                        cfg.artnet_target, cfg.artnet_universe
                    ),
                    Err(e) => format!("send error: {e}"),
                };
            }
        } else {
            artnet = None;
            st.artnet = "off".into();
        }

        #[cfg(feature = "enttec")]
        {
            match &cfg.enttec_port {
                Some(path) => {
                    if enttec.as_ref().map(|(p, _)| p) != Some(path) {
                        enttec = match crate::enttec::UsbPro::open(path) {
                            Ok(dev) => Some((path.clone(), dev)),
                            Err(e) => {
                                st.enttec = format!("{path}: {e}");
                                // Don't retry every frame; the user picks the port again.
                                config.lock().unwrap().enttec_port = None;
                                None
                            }
                        };
                    }
                    if let Some((p, dev)) = &mut enttec {
                        st.enttec = match dev.send(&universe) {
                            Ok(()) => format!("sending on {p}"),
                            Err(e) => format!("{p}: {e}"),
                        };
                    }
                }
                None => {
                    if enttec.take().is_some() {
                        st.enttec = "off".into();
                    }
                }
            }
        }

        *status.lock().unwrap() = st;
        next += FRAME_INTERVAL;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
