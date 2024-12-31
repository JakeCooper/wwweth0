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
        self.rx_queue.pop_front().map(|buffer| {
            log(&format!("📥 Device receive: {:?}", buffer));
            (RxToken(buffer), TxToken { device: self })
        })
    }
}
