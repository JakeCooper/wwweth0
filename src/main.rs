use std::thread;
use std::time::Duration;
use wwweth0::network_stack::NetworkStack;

fn main() {
    println!("Testing WebRTC Netstack");

    // Create network stack
    let mut stack = NetworkStack::new().expect("Failed to create network stack");

    // Test ping to Google's DNS with sequence number
    let sequence = 1;
    match stack.send_ping_with_sequence("8.8.8.8", sequence) {
        Ok(_) => println!("Ping sent successfully"),
        Err(e) => {
            println!("Failed to send ping: {:?}", e);
            std::process::exit(1); // Exit with a non-zero status code to indicate failure
        }
    }

    // Wait for responses with timeout
    let start = std::time::SystemTime::now();
    let timeout = Duration::from_secs(1);

    while start.elapsed().unwrap() < timeout {
        // Process the TX and RX queues
        match stack.receive_ping_response() {
            Ok(Some(response)) => {
                if response.sequence == sequence {
                    println!(
                        "Received ping response: bytes={} time={}ms",
                        response.bytes, response.time_ms
                    );
                    break;
                }
            }
            Ok(None) => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                println!("Error receiving response: {:?}", e);
                break;
            }
        }
    }

    if start.elapsed().unwrap() >= timeout {
        println!("Request timed out");
    }
}
