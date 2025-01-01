use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::ChecksumCapabilities;
use smoltcp::socket::icmp::{Endpoint, PacketBuffer, PacketMetadata, Socket as IcmpSocket};
use smoltcp::time::Instant;
use smoltcp::wire::{
    HardwareAddress, Icmpv4Packet, Icmpv4Repr, IpAddress, IpCidr, IpListenEndpoint,
};
use std::time::SystemTime;
use virtual_device::VirtualDevice;

use crate::log::log;

mod virtual_device;
#[derive(Debug)]
pub struct PingResponse {
    pub sequence: u16,
    pub bytes: usize,
    pub time_ms: u64,
}

pub struct NetworkStack {
    pub device: VirtualDevice,
    pub interface: Interface,
    pub sockets: SocketSet<'static>,
    pub icmp_handle: SocketHandle,
}

fn compute_checksum(packet: &[u8]) -> u16 {
    let mut sum = 0u32;

    // Sum all 16-bit words
    for chunk in packet.chunks(2) {
        let word = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], 0])
        };
        sum += word as u32;
    }

    // Fold 32-bit sum to 16 bits and invert
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

impl NetworkStack {
    pub fn new() -> Result<NetworkStack, String> {
        let mut device = VirtualDevice::new();
        let config = Config::new(HardwareAddress::Ip);
        let mut interface = Interface::new(config, &mut device, Instant::now());

        interface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(192, 168, 5, 66), 24))
                .unwrap();
        });

        let rx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 256], vec![0; 65535]);
        let tx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 256], vec![0; 65535]);
        let mut icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);

        let mut sockets = SocketSet::new(vec![]);
        let icmp_handle = sockets.add(icmp_socket);

        Ok(NetworkStack {
            device,
            interface,
            sockets,
            icmp_handle,
        })
    }

    pub fn send_ping_with_sequence(&mut self, dest_ip: &str, sequence: u16) -> Result<(), String> {
        let target_ip = dest_ip
            .parse::<IpAddress>()
            .map_err(|e| format!("Invalid IP: {:?}", e))?;

        log(&format!(
            "🎯 Sending ping to {} with sequence {}",
            dest_ip, sequence
        ));

        // Create a minimal echo request - ICMP only, no IP header
        let mut echo_payload = vec![0u8; 8];
        echo_payload[0] = 8; // Type 8 = Echo Request
        echo_payload[1] = 0; // Code 0
        echo_payload[2..4].copy_from_slice(&[0, 0]); // Checksum placeholder
        echo_payload[4..6].copy_from_slice(&[0x12, 0x34]); // Identifier (0x1234)
        echo_payload[6..8].copy_from_slice(&sequence.to_be_bytes()); // Sequence number

        // Compute and set checksum
        let checksum = compute_checksum(&echo_payload);
        echo_payload[2..4].copy_from_slice(&checksum.to_be_bytes());

        log(&format!("📝 Created ICMP packet: {:02x?}", echo_payload));

        let socket = self.sockets.get_mut::<IcmpSocket>(self.icmp_handle);
        socket
            .send_slice(&echo_payload, target_ip)
            .map_err(|e| format!("Failed to send: {:?}", e))?;

        Ok(())
    }

    pub fn receive_ping_reply(&mut self, sequence: u16) -> Result<Option<Icmpv4Repr>, String> {
        let socket = self.sockets.get_mut::<IcmpSocket>(self.icmp_handle);

        // Log socket internals
        log(&format!(
            "📥 Socket state - can_recv: {}, can_send: {}",
            socket.can_recv(),
            socket.can_send()
        ));

        if socket.can_recv() {
            let (packet, _) = socket
                .recv()
                .map_err(|e| format!("Failed to receive: {:?}", e))?;

            log(&format!("📥 Raw socket packet: {:02x?}", packet));

            match Icmpv4Packet::new_checked(packet) {
                Ok(icmp_packet) => {
                    log(&format!(
                        "📥 Valid ICMP packet - type: {}, code: {}",
                        icmp_packet.msg_type(),
                        icmp_packet.msg_code()
                    ));

                    match Icmpv4Repr::parse(&icmp_packet, &ChecksumCapabilities::default()) {
                        Ok(Icmpv4Repr::EchoReply {
                            seq_no,
                            ident,
                            data,
                        }) => {
                            log(&format!(
                                "📥 Echo Reply - seq: {}, ident: {:#x}",
                                seq_no, ident
                            ));
                            if seq_no == sequence && ident == 0x1234 {
                                return Ok(Some(Icmpv4Repr::EchoReply {
                                    seq_no,
                                    ident,
                                    data,
                                }));
                            }
                        }
                        Ok(other) => log(&format!("📥 Other ICMP type: {:?}", other)),
                        Err(e) => log(&format!("❌ Parse error: {:?}", e)),
                    }
                }
                Err(e) => log(&format!("❌ Invalid ICMP packet: {:?}", e)),
            }
        }

        Ok(None)
    }
}
