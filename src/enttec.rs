//! ENTTEC DMX USB Pro (and compatible) output over a serial port.
//! Built with the default `enttec` feature.

use std::io::Write;
use std::time::Duration;

/// Label 6 "Output Only Send DMX": 0x7E, label, length LSB/MSB, start code + slots, 0xE7.
pub fn usb_pro_packet(data: &[u8]) -> Vec<u8> {
    let len = data.len().min(512) + 1;
    let mut pkt = Vec::with_capacity(len + 5);
    pkt.extend_from_slice(&[0x7e, 6, (len & 0xff) as u8, (len >> 8) as u8, 0]);
    pkt.extend_from_slice(&data[..len - 1]);
    pkt.push(0xe7);
    pkt
}

pub fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .unwrap_or_default()
}

pub struct UsbPro {
    port: Box<dyn serialport::SerialPort>,
}

impl UsbPro {
    pub fn open(path: &str) -> Result<Self, String> {
        let port = serialport::new(path, 57_600)
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| e.to_string())?;
        Ok(Self { port })
    }

    pub fn send(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.port.write_all(&usb_pro_packet(data))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn packet_framing() {
        let p = super::usb_pro_packet(&[7; 512]);
        assert_eq!(&p[..5], &[0x7e, 6, 0x01, 0x02, 0]);
        assert_eq!(p.len(), 518);
        assert_eq!(*p.last().unwrap(), 0xe7);
    }
}
