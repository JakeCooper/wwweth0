use smoltcp::time::Instant;
use std::time::Duration;
use tokio::time::sleep;
use wwweth0::network_stack::NetworkStack;

mod log;

#[tokio::main]
async fn main() {
    println!("Testing ICMP Ping with smoltcp");

    // Create network stack
    let mut stack = NetworkStack::new().expect("Failed to initialize network stack");

    // Destination IP address
    let dest_ip = "8.8.8.8";
    let sequence = 1;

    // Send ICMP Echo Request
    match stack.send_ping_with_sequence(dest_ip, sequence) {
        Ok(_) => println!("Ping sent successfully"),
        Err(e) => {
            eprintln!("Failed to send ping: {}", e);
            return;
        }
    }

    // Wait for Echo Reply
    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            println!("Request timed out");
            break;
        }

        stack.device.process_rx_queue();

        // Poll the interface more frequently with current timestamp
        let timestamp = Instant::now();
        match stack
            .interface
            .poll(timestamp, &mut stack.device, &mut stack.sockets)
        {
            smoltcp::iface::PollResult::SocketStateChanged => {
                println!("🔄 Socket state changed");

                // Poll again immediately after state change
                stack
                    .interface
                    .poll(timestamp, &mut stack.device, &mut stack.sockets);

                if let Ok(Some(reply)) = stack.receive_ping_reply(sequence) {
                    println!("✅ Received reply: {:?}", reply);
                    break;
                }
            }
            smoltcp::iface::PollResult::None => {
                // Process any queued transmissions
                stack.device.process_tx_queue();
            }
        }

        // Check for ping reply
        match stack.receive_ping_reply(sequence) {
            Ok(Some(reply)) => {
                println!("✅ Received reply: {:?}", reply);
                break;
            }
            Ok(None) => {
                // No reply yet, continue waiting
            }
            Err(e) => {
                eprintln!("❌ Error receiving reply: {}", e);
                break;
            }
        }

        // Sleep briefly to avoid busy-waiting, but poll more frequently
        sleep(Duration::from_millis(10)).await;
    }
}
