use std::thread;
use std::time::Duration;
use wwweth0::NetworkStack;

fn main() {
    println!("Testing WebRTC Netstack");

    // Create network stack
    let mut stack = NetworkStack::new().expect("Failed to create network stack");

    // Test ping to Google's DNS
    match stack.send_ping("8.8.8.8") {
        Ok(_) => println!("Ping sent successfully"),
        Err(e) => println!("Failed to send ping: {:?}", e),
    }

    // Wait for responses
    for _ in 0..5 {
        thread::sleep(Duration::from_millis(100));
        if let Err(e) = stack.process_incoming() {
            println!("Error processing packets: {:?}", e);
        }
    }
}
