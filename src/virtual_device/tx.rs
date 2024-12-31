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
        log(&format!("📤 Enqueueing TX packet: {:?}", buffer));

        // Enqueue the buffer into the TX queue
        self.device.enqueue_tx_packet(buffer);

        result
    }
}
