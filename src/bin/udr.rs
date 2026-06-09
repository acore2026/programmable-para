use std::net::TcpListener;
use std::collections::BTreeMap;
use serde_json::json;
use anyhow::Result;

use programmable_parameter_demo::types::{Scenario, SubscriptionData, UeRegistration, UdrResponse};
use programmable_parameter_demo::net::{read_payload, write_payload};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8081")?;
    println!("UDR listening on 127.0.0.1:8081");

    for stream in listener.incoming() {
        let mut stream = stream?;
        
        // Read scenario request from Intermediate NF using helper
        let scenario: Scenario = match read_payload(&mut stream) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("UDR failed to read/parse scenario: {}", e);
                continue;
            }
        };

        match udr_emit_and_ue_register(scenario) {
            Ok((subscription, registration)) => {
                let response = UdrResponse { subscription, registration };
                if let Err(e) = write_payload(&mut stream, &response) {
                    eprintln!("UDR failed to write response: {}", e);
                }
            }
            Err(e) => {
                eprintln!("UDR error during data generation for scenario {:?}: {}", scenario, e);
            }
        }
    }
    Ok(())
}

fn udr_emit_and_ue_register(_scenario: Scenario) -> Result<(SubscriptionData, UeRegistration)> {
    let metadata = BTreeMap::from([
        (
            "aiAgentId".to_string(),
            json!("urn:3gpp:ai-agent:auto-pilot-v2"),
        ),
        ("trustLevel".to_string(), json!("high")),
        ("vendor".to_string(), json!("Manufacturer-X")),
    ]);

    let subscription = SubscriptionData {
        subscriber_id: "imsi-001010000000001".to_string(),
        slice: "enterprise-ai".to_string(),
        metadata,
    };

    let registration = UeRegistration {
        subscriber_id: subscription.subscriber_id.clone(),
        claims: BTreeMap::from([
            (
                "aiAgentId".to_string(),
                json!("urn:3gpp:ai-agent:auto-pilot-v2"),
            ),
            ("vendor".to_string(), json!("Manufacturer-X")),
        ]),
    };

    Ok((subscription, registration))
}
