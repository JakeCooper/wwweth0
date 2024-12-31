use smoltcp::phy::{self};

#[derive(Debug)]
pub struct RxToken(pub(crate) Vec<u8>);

impl phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}
