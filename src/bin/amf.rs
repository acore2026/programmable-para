use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};
use anyhow::Result;

use programmable_parameter_demo::types::{AmfRequest, SubscriptionData, Scenario, AmfResponse};
use programmable_parameter_demo::wasm::amf_verify;

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8083")?;
    println!("AMF listening on 127.0.0.1:8083");

    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut buf = Vec::new();
        
        // Read the verification request from the orchestrator
        if let Err(e) = stream.read_to_end(&mut buf) {
            eprintln!("AMF failed to read stream: {}", e);
            continue;
        }

        let request: AmfRequest = match serde_json::from_slice(&buf) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("AMF failed to parse verification request: {}", e);
                continue;
            }
        };

        // Query the intermediate NF for subscription data
        match query_intermediate_nf_for_subscription(request.scenario) {
            Ok(subscription) => {
                // Execute the verification applet (via wasmtime engine)
                match amf_verify(&subscription, &request.registration, &request.route) {
                    Ok(decision) => {
                        let response = AmfResponse {
                            decision,
                            subscription,
                        };
                        if let Ok(res_bytes) = serde_json::to_vec(&response) {
                            if let Err(e) = stream.write_all(&res_bytes) {
                                eprintln!("AMF failed to send decision: {}", e);
                            }
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
    let mut inf_stream = TcpStream::connect("127.0.0.1:8082")?;
    
    // Request subscription from intermediate NF
    let req_bytes = serde_json::to_vec(&scenario)?;
    inf_stream.write_all(&req_bytes)?;
    inf_stream.shutdown(Shutdown::Write)?;

    // Read the forwarded subscription data
    let mut buf = Vec::new();
    inf_stream.read_to_end(&mut buf)?;
    
    let subscription: SubscriptionData = serde_json::from_slice(&buf)?;
    Ok(subscription)
}
