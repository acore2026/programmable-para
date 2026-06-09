use std::net::TcpListener;
use anyhow::Result;

use programmable_parameter_demo::types::PushPayload;
use programmable_parameter_demo::wasm::amf_verify;
use programmable_parameter_demo::net::{read_payload, write_payload};

fn main() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8083")?;
    println!("AMF listening on 127.0.0.1:8083");

    for stream in listener.incoming() {
        let mut stream = stream?;
        
        // Read the PushPayload from Intermediate NF
        let payload: PushPayload = match read_payload(&mut stream) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[AMF] Failed to read dynamic push payload: {}", e);
                continue;
            }
        };

        println!(
            "[AMF] Received payload from Intermediate NF:\n{}",
            serde_json::to_string_pretty(&payload).unwrap()
        );
        println!("[AMF] Executing WASM verification using applet: {}", payload.route.applet_path.display());

        // Execute verification using WASM engine
        match amf_verify(&payload.subscription, &payload.registration, &payload.route) {
            Ok(decision) => {
                println!("[AMF] WASM execution complete. Decision: {}", decision);
                if let Err(e) = write_payload(&mut stream, &decision) {
                    eprintln!("[AMF] Failed to write decision response: {}", e);
                }
            }
            Err(e) => {
                eprintln!("[AMF] Error during WASM execution: {}", e);
            }
        }
    }
    Ok(())
}
