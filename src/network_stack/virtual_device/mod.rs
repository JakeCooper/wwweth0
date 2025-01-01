use smoltcp::phy::{self, DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use std::collections::VecDeque;
use std::mem::MaybeUninit;

pub mod rx;
pub mod tx;
use rx::RxToken;
use tx::TxToken;

use crate::log::log;

#[derive(Debug)]
pub struct VirtualDevice {
    rx_queue: VecDeque<Vec<u8>>,
    tx_queue: VecDeque<Vec<u8>>,
    raw_socket: Option<Socket>, // Store the socket for receiving
}

use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;

impl VirtualDevice {
    pub fn new() -> Self {
        log("Creating new virtual device");
        // Create raw socket for receiving ICMP
        let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
            .expect("Failed to create raw socket");

        // Bind to all interfaces
        let addr = SocketAddr::from(([0, 0, 0, 0], 0));
        socket.bind(&addr.into()).expect("Failed to bind socket");

        // Set non-blocking mode
        socket
            .set_nonblocking(true)
            .expect("Failed to set non-blocking");

        Self {
            rx_queue: VecDeque::new(),
            tx_queue: VecDeque::new(),
            raw_socket: Some(socket),
        }
    }
    pub fn process_tx_queue(&mut self) {
        while let Some(packet) = self.tx_queue.pop_front() {
            log(&format!("📤 Transmitting packet: {:?}", packet));

            // Skip IP header for sending
            let icmp_data = if packet[0] >> 4 == 4 {
                &packet[20..]
            } else {
                &packet
            };

            // Create new socket for each send (or reuse existing one)
            let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
                .expect("Failed to create raw socket");
            let dest = SocketAddr::from(([8, 8, 8, 8], 0));
            if let Err(e) = socket.connect(&dest.into()) {
                log(&format!("❌ Failed to connect: {:?}", e));
                continue;
            }

            match socket.send(icmp_data) {
                Ok(_) => log("📤 Packet sent successfully"),
                Err(e) => log(&format!("❌ Failed to send packet: {:?}", e)),
            }
        }
    }

    pub fn process_rx_queue(&mut self) {
        if let Some(socket) = &self.raw_socket {
            let mut buf = [MaybeUninit::uninit(); 2048];

            match socket.recv(&mut buf) {
                Ok(n) => {
                    let received: Vec<u8> = buf[..n]
                        .iter()
                        .map(|b| unsafe { b.assume_init() })
                        .collect();

                    if n >= 28 && received[9] == 1 {
                        // ICMP protocol
                        // Extract IP info from header
                        let ttl = received[8];
                        let src_ip = &received[12..16];
                        let dst_ip = &received[16..20];
                        let icmp_data = &received[20..];

                        log(&format!("📥 Full received packet: {:02x?}", received));
                        log(&format!("📥 ICMP portion: {:02x?}", icmp_data));

                        // Construct IP header
                        let mut ip_packet = vec![0u8; n];
                        ip_packet[0] = 0x45; // IPv4, 5 words header
                        ip_packet[1] = 0; // DSCP
                        ip_packet[2..4].copy_from_slice(&(n as u16).to_be_bytes()); // Total length
                        ip_packet[8] = ttl; // TTL
                        ip_packet[9] = 1; // ICMP
                        ip_packet[12..16].copy_from_slice(src_ip); // Source IP
                        ip_packet[16..20].copy_from_slice(dst_ip); // Dest IP
                        ip_packet[20..].copy_from_slice(icmp_data); // ICMP data

                        log(&format!("📥 Constructed packet: {:02x?}", ip_packet));
                        self.rx_queue.push_back(ip_packet);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(e) => log(&format!("❌ Error receiving: {:?}", e)),
            }
        }
    }
}

impl Default for VirtualDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl phy::Device for VirtualDevice {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = 1500;
        caps
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        log("📤 Device transmit");
        Some(TxToken { device: self })
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(packet) = self.rx_queue.pop_front() {
            // Take the full packet - RxToken will handle stripping headers
            Some((RxToken(packet), TxToken { device: self }))
        } else {
            None
        }
    }
}
