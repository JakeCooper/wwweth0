use smoltcp::phy::{self};

use super::{log, VirtualDevice};

#[derive(Debug)]
pub struct TxToken<'a> {
    pub(crate) device: &'a mut VirtualDevice,
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
