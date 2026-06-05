use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};
use anyhow::Result;

use programmable_parameter_demo::types::{Scenario, SubscriptionData, UdrResponse};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8082")?;
    println!("Intermediate NF listening on 127.0.0.1:8082");

    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut buf = Vec::new();
        
        // Read scenario request from AMF
        if let Err(e) = stream.read_to_end(&mut buf) {
            eprintln!("Intermediate NF failed to read stream: {}", e);
            continue;
        }

        let scenario: Scenario = match serde_json::from_slice(&buf) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Intermediate NF failed to parse scenario request: {}", e);
                continue;
            }
        };

        // Query UDR for subscription data
        match query_udr_for_subscription(scenario) {
            Ok(subscription) => {
                // Simulate intermediate NF forwarding logic:
                let forwarded = intermediate_nf_forward(subscription);
                if let Ok(res_bytes) = serde_json::to_vec(&forwarded) {
                    if let Err(e) = stream.write_all(&res_bytes) {
                        eprintln!("Intermediate NF failed to send response: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Intermediate NF failed to retrieve subscription from UDR: {}", e);
            }
        }
    }
    Ok(())
}

fn query_udr_for_subscription(scenario: Scenario) -> Result<SubscriptionData> {
    let mut udr_stream = TcpStream::connect("127.0.0.1:8081")?;
    
    // Send scenario to UDR
    let req_bytes = serde_json::to_vec(&scenario)?;
    udr_stream.write_all(&req_bytes)?;
    udr_stream.shutdown(Shutdown::Write)?;

    // Read response from UDR
    let mut buf = Vec::new();
    udr_stream.read_to_end(&mut buf)?;
    
    let udr_response: UdrResponse = serde_json::from_slice(&buf)?;
    Ok(udr_response.subscription)
}

fn intermediate_nf_forward(subscription: SubscriptionData) -> SubscriptionData {
    let _known_fields = (&subscription.subscriber_id, &subscription.slice);
    subscription
}
