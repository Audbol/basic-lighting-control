//! Art-Net 4 output (ArtDmx over UDP port 6454).

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

pub const ARTNET_PORT: u16 = 6454;
const OP_DMX: u16 = 0x5000;
const PROTOCOL_VERSION: u16 = 14;

/// Builds an ArtDmx packet. `universe` is the 15-bit Port-Address
/// (Net 7 bits | Sub-Net 4 bits | Universe 4 bits).
pub fn art_dmx(data: &[u8], universe: u16, sequence: u8) -> Vec<u8> {
    // The data length must be even and 2..=512.
    let mut len = data.len().clamp(2, 512);
    len += len % 2;
    let mut pkt = Vec::with_capacity(18 + len);
    pkt.extend_from_slice(b"Art-Net\0");
    pkt.extend_from_slice(&OP_DMX.to_le_bytes());
    pkt.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    pkt.push(sequence);
    pkt.push(0); // physical port
    pkt.push((universe & 0xff) as u8); // SubUni
    pkt.push(((universe >> 8) & 0x7f) as u8); // Net
    pkt.extend_from_slice(&(len as u16).to_be_bytes());
    let n = data.len().min(len);
    pkt.extend_from_slice(&data[..n]);
    pkt.resize(18 + len, 0);
    pkt
}

pub struct ArtNetSender {
    socket: UdpSocket,
    sequence: u8,
}

impl ArtNetSender {
    pub fn new() -> io::Result<Self> {
        // Nodes expect traffic from port 6454; fall back to any port if another app has it.
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, ARTNET_PORT))
            .or_else(|_| UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)))?;
        socket.set_broadcast(true)?;
        Ok(Self {
            socket,
            sequence: 0,
        })
    }

    pub fn send(&mut self, target: Ipv4Addr, universe: u16, data: &[u8]) -> io::Result<()> {
        // 1..=255; 0 would disable sequencing at the receiver.
        self.sequence = self.sequence % 255 + 1;
        let pkt = art_dmx(data, universe, self.sequence);
        self.socket
            .send_to(&pkt, SocketAddrV4::new(target, ARTNET_PORT))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_dmx_layout() {
        let mut data = [0u8; 512];
        data[0] = 0xaa;
        data[511] = 0x55;
        let p = art_dmx(&data, 0x1234, 7);
        assert_eq!(&p[..8], b"Art-Net\0");
        assert_eq!(&p[8..10], &[0x00, 0x50]); // OpDmx, little-endian
        assert_eq!(&p[10..12], &[0, 14]);
        assert_eq!(p[12], 7);
        assert_eq!(p[14], 0x34); // SubUni
        assert_eq!(p[15], 0x12); // Net
        assert_eq!(&p[16..18], &[0x02, 0x00]); // 512, big-endian
        assert_eq!(p.len(), 18 + 512);
        assert_eq!(p[18], 0xaa);
        assert_eq!(p[18 + 511], 0x55);
    }

    #[test]
    fn odd_length_is_padded() {
        let p = art_dmx(&[1, 2, 3], 0, 1);
        assert_eq!(&p[16..18], &[0, 4]);
        assert_eq!(&p[18..], &[1, 2, 3, 0]);
    }

    #[test]
    fn sends_over_loopback() {
        let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = rx.local_addr().unwrap().port();
        let tx = UdpSocket::bind("127.0.0.1:0").unwrap();
        tx.send_to(&art_dmx(&[9; 512], 3, 1), ("127.0.0.1", port))
            .unwrap();
        let mut buf = [0u8; 600];
        let n = rx.recv(&mut buf).unwrap();
        assert_eq!(n, 530);
        assert_eq!(buf[14], 3);
    }
}
