use std::net::TcpListener;
use anyhow::Result;

use programmable_parameter_demo::types::{AmfRequest, SubscriptionData, Scenario, AmfResponse};
use programmable_parameter_demo::wasm::amf_verify;
use programmable_parameter_demo::net::{read_payload, write_payload, send_request_and_get_response};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8083")?;
    println!("AMF listening on 127.0.0.1:8083");

    for stream in listener.incoming() {
        let mut stream = stream?;
        
        // Read the verification request from the orchestrator client
        let request: AmfRequest = match read_payload(&mut stream) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("AMF failed to read verification request: {}", e);
                continue;
            }
        };

        // Query the intermediate NF for subscription data
        match query_intermediate_nf_for_subscription(request.scenario) {
            Ok(subscription) => {
                // Execute the verification applet (via wasmtime engine)
                match amf_verify(&subscription, &request.registration, &request.route) {
                    Ok(decision) => {
                        let response = AmfResponse { decision, subscription };
                        if let Err(e) = write_payload(&mut stream, &response) {
                            eprintln!("AMF failed to write response: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("AMF error during Wasm execution: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("AMF failed to fetch subscription data: {}", e);
            }
        }
    }
    Ok(())
}

fn query_intermediate_nf_for_subscription(scenario: Scenario) -> Result<SubscriptionData> {
    send_request_and_get_response("127.0.0.1:8082", &scenario)
}
