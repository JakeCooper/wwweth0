use smoltcp::phy::{self, DeviceCapabilities, Medium};
use smoltcp::time::Instant;
use std::collections::VecDeque;

use crate::log::log;

#[derive(Debug)]
pub struct VirtualDevice {
    packets: VecDeque<Vec<u8>>,
}

#[derive(Debug)]
pub struct RxToken(Vec<u8>);

#[derive(Debug)]
pub struct TxToken<'a> {
    device: &'a mut VirtualDevice,
}

impl VirtualDevice {
    pub fn new() -> Self {
        log("Creating new virtual device");
        Self {
            packets: VecDeque::new(),
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
        Some(TxToken { device: self })
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.packets.pop_front().map(|buffer| {
            log(&format!("📥 Device receive: {:?}", buffer));
            (RxToken(buffer), TxToken { device: self })
        })
    }
}

impl<'a> phy::TxToken for TxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0; len];
        let result = f(&mut buffer);
        log(&format!("📤 Device TX: {:?}", buffer));

        // Save packet in device queue
        self.device.packets.push_back(buffer.clone());

        // Process ICMP packets
        if buffer.len() >= 28 && buffer[9] == 1 {
            // Has IP header and is ICMP
            let icmp_start = 20; // Skip IP header
            if buffer[icmp_start] == 8 {
                // Is echo request
                let mut reply = buffer.clone();

                // Swap IP addresses in header
                reply[12..16].clone_from_slice(&buffer[16..20]);
                reply[16..20].clone_from_slice(&buffer[12..16]);

                // Change ICMP type to echo reply
                reply[icmp_start] = 0;

                // Clear IP checksum to force recalculation
                reply[10] = 0;
                reply[11] = 0;

                log(&format!("📤 Created echo reply: {:?}", reply));
                self.device.packets.push_back(reply);
            }
        }

        result
    }
}

impl phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}
