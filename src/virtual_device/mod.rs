use rx::RxToken;
use smoltcp::phy::{self, DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use std::collections::VecDeque;
use tx::TxToken;

mod rx;
mod tx;

use crate::log::log;

#[derive(Debug)]
pub struct VirtualDevice {
    rx_queue: VecDeque<Vec<u8>>,
    tx_queue: VecDeque<Vec<u8>>,
}

impl VirtualDevice {
    pub fn new() -> Self {
        log("Creating new virtual device");
        Self {
            rx_queue: VecDeque::new(),
            tx_queue: VecDeque::new(),
        }
    }

    pub fn enqueue_rx_packet(&mut self, packet: Vec<u8>) {
        log(&format!("📥 Enqueue RX packet: {:?}", packet));
        self.rx_queue.push_back(packet);
    }

    pub fn dequeue_rx_packet(&mut self) -> Option<Vec<u8>> {
        let packet = self.rx_queue.pop_front();
        if let Some(ref p) = packet {
            log(&format!("📤 Dequeue RX packet: {:?}", p));
        }
        packet
    }

    pub fn enqueue_tx_packet(&mut self, packet: Vec<u8>) {
        log(&format!("📤 Enqueue TX packet: {:?}", packet));
        self.tx_queue.push_back(packet);
    }

    pub fn dequeue_tx_packet(&mut self) -> Option<Vec<u8>> {
        let packet = self.tx_queue.pop_front();
        if let Some(ref p) = packet {
            log(&format!("📤 Dequeue TX packet: {:?}", p));
        }
        packet
    }
}

/// Helper function to generate ICMP echo reply
fn generate_icmp_reply(packet: &[u8]) -> Option<Vec<u8>> {
    // Check if the packet is at least 28 bytes (IP header + ICMP header)
    if packet.len() >= 28 && packet[9] == 1 {
        // Check if it's an ICMP echo request
        let icmp_start = 20; // ICMP starts after the IP header
        if packet[icmp_start] == 8 {
            let mut reply = packet.to_vec();

            // Swap source and destination IP addresses in the IP header
            reply[12..16].clone_from_slice(&packet[16..20]);
            reply[16..20].clone_from_slice(&packet[12..16]);

            // Change ICMP type to echo reply
            reply[icmp_start] = 0;

            // Clear the ICMP checksum (forcing recalculation)
            reply[22] = 0;
            reply[23] = 0;

            return Some(reply);
        }
    }

    None
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
        if self.tx_queue.is_empty() {
            log("TX queue is empty, nothing to transmit");
            None
        } else {
            log("Providing TxToken for transmission");
            Some(TxToken { device: self })
        }
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(buffer) = self.dequeue_rx_packet() {
            log(&format!("📥 Device received packet: {:?}", buffer));
            Some((RxToken(buffer), TxToken { device: self }))
        } else {
            log("RX queue is empty, nothing to receive");
            None
        }
    }
}
