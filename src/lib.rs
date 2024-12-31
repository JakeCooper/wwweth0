use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{self, DeviceCapabilities, Medium};
use smoltcp::socket::icmp::{PacketBuffer, PacketMetadata, Socket as IcmpSocket};
use smoltcp::time::Instant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr};
use std::cell::RefCell;
use std::collections::VecDeque;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Conditional logging
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[cfg(not(target_arch = "wasm32"))]
fn log(s: &str) {
    println!("{}", s);
}

pub struct VirtualDevice {
    rx_buffer: VecDeque<Vec<u8>>,
    tx_buffer: VecDeque<Vec<u8>>,
}

pub struct RxToken(Vec<u8>);
pub struct TxToken(Vec<u8>);

impl VirtualDevice {
    pub fn new() -> Self {
        Self {
            rx_buffer: VecDeque::new(),
            tx_buffer: VecDeque::new(),
        }
    }

    pub fn queue_tx_packet(&mut self, packet: Vec<u8>) {
        self.tx_buffer.push_back(packet);
    }
}

impl Default for VirtualDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl phy::Device for VirtualDevice {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = 1500;
        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        self.rx_buffer
            .pop_front()
            .map(|buffer| (RxToken(buffer), TxToken(Vec::new())))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken(Vec::with_capacity(
            self.capabilities().max_transmission_unit,
        )))
    }
}

impl phy::TxToken for TxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0; len];
        f(&mut buffer)
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct NetworkStack {
    device: RefCell<VirtualDevice>,
    interface: RefCell<Interface>,
    sockets: RefCell<SocketSet<'static>>,
    icmp_handle: RefCell<Option<SocketHandle>>,
    local_addr: IpAddress,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl NetworkStack {
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> Result<NetworkStack, String> {
        let hardware_addr = HardwareAddress::Ip;
        let config = Config::new(hardware_addr);
        let mut device = VirtualDevice::new();
        let now = Instant::now();

        let local_addr = IpAddress::v4(192, 168, 69, 1);
        let mut interface = Interface::new(config, &mut device, now);

        interface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(IpCidr::new(local_addr, 24)).unwrap();
        });

        // Create ICMP socket with proper buffers
        let rx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 8], vec![0; 64 * 8]);
        let tx_buffer = PacketBuffer::new(vec![PacketMetadata::EMPTY; 8], vec![0; 64 * 8]);
        let icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);

        // Create socket set and add ICMP socket
        let mut sockets = SocketSet::new(vec![]);
        let icmp_handle = sockets.add(icmp_socket);

        Ok(NetworkStack {
            device: RefCell::new(device),
            interface: RefCell::new(interface),
            sockets: RefCell::new(sockets),
            icmp_handle: RefCell::new(Some(icmp_handle)),
            local_addr,
        })
    }

    pub fn send_ping(&mut self, dest_ip: &str) -> Result<(), String> {
        let target_ip = dest_ip
            .parse::<IpAddress>()
            .map_err(|e| format!("Invalid IP: {:?}", e))?;

        let now = Instant::now();

        log(&format!(
            "Sending ping from {} to {}",
            self.local_addr, target_ip
        ));

        // Get the ICMP socket
        let icmp_handle = *self
            .icmp_handle
            .borrow()
            .as_ref()
            .ok_or_else(|| "ICMP socket not initialized".to_string())?;

        // Create ICMP Echo Request payload with a simple pattern
        let echo_payload = [0x42; 32];

        {
            let mut sockets = self.sockets.borrow_mut();
            let socket = sockets.get_mut::<IcmpSocket>(icmp_handle);

            socket
                .send_slice(&echo_payload, target_ip)
                .map_err(|e| format!("Failed to send: {:?}", e))?;
        }

        // Poll the interface multiple times to give it a chance to send/receive
        for _ in 0..5 {
            let mut iface = self.interface.borrow_mut();
            let mut device = self.device.borrow_mut();
            let mut sockets = self.sockets.borrow_mut();
            iface.poll(now, &mut *device, &mut *sockets);
        }

        // Check for response
        {
            let mut sockets = self.sockets.borrow_mut();
            let socket = sockets.get_mut::<IcmpSocket>(icmp_handle);

            while socket.can_recv() {
                let (payload, reply_addr) = socket
                    .recv()
                    .map_err(|e| format!("Failed to receive: {:?}", e))?;

                log(&format!(
                    "Received {} bytes ICMP response from {}",
                    payload.len(),
                    reply_addr
                ));
            }
        }

        log(&format!("Ping requested to {}", target_ip));
        Ok(())
    }

    pub fn process_incoming(&mut self) -> Result<(), String> {
        let now = Instant::now();

        {
            let mut iface = self.interface.borrow_mut();
            let mut device = self.device.borrow_mut();
            let mut sockets = self.sockets.borrow_mut();
            iface.poll(now, &mut *device, &mut sockets);
        }

        // Process ICMP responses
        if let Some(icmp_handle) = *self.icmp_handle.borrow() {
            let mut sockets = self.sockets.borrow_mut();
            let socket = sockets.get_mut::<IcmpSocket>(icmp_handle);

            while socket.can_recv() {
                let (payload, reply_addr) = socket
                    .recv()
                    .map_err(|e| format!("Failed to receive: {:?}", e))?;

                log(&format!(
                    "Received {} bytes ICMP response from {}",
                    payload.len(),
                    reply_addr
                ));
            }
        }

        Ok(())
    }
}
