use std::net::TcpListener;
use anyhow::Result;

use programmable_parameter_demo::types::{Scenario, SubscriptionData, UdrResponse};
use programmable_parameter_demo::net::{read_payload, write_payload, send_request_and_get_response};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8082")?;
    println!("Intermediate NF listening on 127.0.0.1:8082");

    for stream in listener.incoming() {
        let mut stream = stream?;
        
        // Read scenario request from AMF
        let scenario: Scenario = match read_payload(&mut stream) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Intermediate NF failed to read scenario: {}", e);
                continue;
            }
        };

        // Query UDR for subscription data using helper
        match query_udr_for_subscription(scenario) {
            Ok(subscription) => {
                let forwarded = intermediate_nf_forward(subscription);
                if let Err(e) = write_payload(&mut stream, &forwarded) {
                    eprintln!("Intermediate NF failed to write response: {}", e);
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
    let udr_response: UdrResponse = send_request_and_get_response("127.0.0.1:8081", &scenario)?;
    Ok(udr_response.subscription)
}

fn intermediate_nf_forward(subscription: SubscriptionData) -> SubscriptionData {
    let _known_fields = (&subscription.subscriber_id, &subscription.slice);
    subscription
}
