use super::log;
use smoltcp::phy::{self};

#[derive(Debug)]
pub struct RxToken(pub(crate) Vec<u8>);

impl phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        // Log the raw packet
        log(&format!("📥 RxToken consuming packet: {:02x?}", &self.0));

        let payload = if self.0[0] >> 4 == 4 {
            // If it has IP header
            let ip_ihl = (self.0[0] & 0x0f) as usize * 4; // Calculate actual IP header length
            log(&format!("📥 Stripping IP header of {} bytes", ip_ihl));
            &self.0[ip_ihl..] // Use actual header length
        } else {
            &self.0
        };

        log(&format!("📥 Final payload to socket: {:02x?}", payload));

        f(payload)
    }
}
