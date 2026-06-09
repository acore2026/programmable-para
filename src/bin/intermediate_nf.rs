use std::net::TcpListener;
use anyhow::Result;

use programmable_parameter_demo::types::{PushPayload, Decision, SubscriptionData};
use programmable_parameter_demo::net::{read_payload, write_payload, send_request_and_get_response};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8082")?;
    println!("Intermediate NF listening on 127.0.0.1:8082");

    for stream in listener.incoming() {
        let mut stream = stream?;
        
        // Read the PushPayload from UDR
        let mut payload: PushPayload = match read_payload(&mut stream) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Intermediate NF failed to read push payload: {}", e);
                continue;
            }
        };

        // Simulate Intermediate NF forwarding check
        payload.subscription = intermediate_nf_forward(payload.subscription);

        // Forward to AMF server (port 8083) and retrieve decision
        match send_request_and_get_response::<PushPayload, Decision>("127.0.0.1:8083", &payload) {
            Ok(decision) => {
                if let Err(e) = write_payload(&mut stream, &decision) {
                    eprintln!("Intermediate NF failed to send decision back to UDR: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Intermediate NF failed to query AMF: {}", e);
            }
        }
    }
    Ok(())
}

fn intermediate_nf_forward(subscription: SubscriptionData) -> SubscriptionData {
    let _known_fields = (&subscription.subscriber_id, &subscription.slice);
    subscription
}
