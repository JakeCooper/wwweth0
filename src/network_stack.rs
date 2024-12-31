use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::socket::icmp::{PacketBuffer, PacketMetadata, Socket as IcmpSocket};
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::time::SystemTime;

use crate::log::log;
use crate::virtual_device::VirtualDevice;
#[derive(Debug)]
pub struct PingResponse {
    pub sequence: u16,
    pub bytes: usize,
    pub time_ms: u64,
}

pub struct NetworkStack {
    device: VirtualDevice,
    interface: Interface,
    sockets: SocketSet<'static>,
    icmp_handle: SocketHandle,
}

impl NetworkStack {
    pub fn new() -> Result<NetworkStack, String> {
        let mut device = VirtualDevice::new();
        let config = Config::new(HardwareAddress::Ip);
        let mut interface = Interface::new(config, &mut device, Instant::now());

        interface.update_ip_addrs(|ip_addrs| {
            ip_addrs
                .push(IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24))
                .unwrap();
        });

        let rx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 64], vec![0; 65535 * 64]);
        let tx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 64], vec![0; 65535 * 64]);
        let icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);

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

        // Create a minimal echo request
        let mut echo_payload = vec![0u8; 8];
        echo_payload[0] = 8; // Echo request
        echo_payload[4] = (sequence >> 8) as u8;
        echo_payload[5] = sequence as u8;
        echo_payload[6] = (sequence >> 8) as u8;
        echo_payload[7] = sequence as u8;

        log(&format!("📝 Created ICMP: {:?}", echo_payload));

        let socket = self.sockets.get_mut::<IcmpSocket>(self.icmp_handle);
        socket
            .send_slice(&echo_payload, target_ip)
            .map_err(|e| format!("Failed to send: {:?}", e))?;

        // Call first poll to send the packet
        let outbound = self
            .interface
            .poll(Instant::now(), &mut self.device, &mut self.sockets);

        // If none response, error out
        if outbound == smoltcp::iface::PollResult::None {
            return Err("No packet sent".to_string());
        }

        log("📤 Packet sent successfully");

        // Call second poll to receive the reply
        let reply = self
            .interface
            .poll(Instant::now(), &mut self.device, &mut self.sockets);

        if reply == smoltcp::iface::PollResult::None {
            return Err("No response received".to_string());
        }

        log("📥 Response received");

        let empty = self
            .interface
            .poll(Instant::now(), &mut self.device, &mut self.sockets);

        // Calling a third time should return empty because no new payloads added
        if empty == smoltcp::iface::PollResult::SocketStateChanged {
            return Err("Socket state changed".to_string());
        }

        Ok(())
    }

    pub fn receive_ping_response(&mut self) -> Result<Option<PingResponse>, String> {
        let start = SystemTime::now();
        log("👂 Checking for response...");

        // Poll multiple times to handle the reply
        for _ in 0..3 {
            let result = self
                .interface
                .poll(Instant::now(), &mut self.device, &mut self.sockets);
            log(&format!("📥 Poll result: {:?}", result));

            // Check for response
            let socket = self.sockets.get_mut::<IcmpSocket>(self.icmp_handle);
            if socket.can_recv() {
                if let Ok((payload, _)) = socket.recv() {
                    log(&format!("📥 ICMP packet: {:?}", payload));
                    if payload.len() >= 8 && payload[0] == 0 {
                        // Echo reply
                        let sequence = ((payload[6] as u16) << 8) | (payload[7] as u16);
                        let elapsed = start.elapsed().unwrap().as_millis() as u64;
                        log(&format!("🎯 Got reply for sequence {}", sequence));
                        return Ok(Some(PingResponse {
                            sequence,
                            bytes: payload.len(),
                            time_ms: elapsed,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }
}
