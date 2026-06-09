use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::json;

use crate::types::{Decision, Route, UeRegistration, SubscriptionData, PushPayload};
use crate::net::send_request_and_get_response;

const TRIGGER_UE_REGISTRATION: &str = "UE_REGISTRATION";

#[derive(Debug)]
pub struct DemoReport {
    pub decision: Option<Decision>,
    pub lines: Vec<String>,
}

impl DemoReport {
    pub fn print(&self) {
        for line in &self.lines {
            println!("{line}");
        }
        if let Some(decision) = self.decision {
            let color_decision = visuals::colorize_decision(decision);
            println!("\x1b[1mFinal Decision: {}\x1b[0m", color_decision);
        }
    }
}

pub fn run_demo() -> Result<DemoReport> {
    // 1. Define the static route for subscriber verification logic
    let route = Route {
        trigger: TRIGGER_UE_REGISTRATION.to_string(),
        priority: 20,
        applet_path: PathBuf::from("applets/rel22_vendor.wat"),
        action_on_mismatch: Decision::LimitAccess,
    };

    // 2. UDR generates the subscriber subscription data and UE claims
    let (subscription, registration) = udr_emit_and_ue_register()?;

    // 3. Wrap everything into the PushPayload
    let payload = PushPayload {
        subscription: subscription.clone(),
        registration,
        route: route.clone(),
    };

    // Run startup and Step 1 visuals
    visuals::start();
    visuals::step1_emit(&subscription);

    // Run Step 2 send visuals
    visuals::step2_send();

    // 4. Connect to Intermediate NF (port 8082) and push the payload
    let decision: Decision = send_request_and_get_response("127.0.0.1:8082", &payload)
        .context("Could not connect to Intermediate NF at 127.0.0.1:8082. Are the Intermediate NF and AMF servers running?")?;

    // Run Step 3, 4, 5, 6 visuals
    visuals::step3_forward();
    visuals::step4_wasm(&route.applet_path);
    visuals::step5_return_amf(decision);
    visuals::step6_return_udr(decision);

    // 5. Format lines for output
    let decision_str = visuals::colorize_decision(decision);
    let mut lines = Vec::new();
    lines.push("\x1b[1;36m=== UDR CLIENT REPORT ===\x1b[0m".to_string());
    lines.push(format!(
        "1. UDR emits subscription data and registration:\n  - Subscription metadata keys: {:?}\n  - Registration claims keys: {:?}",
        subscription.metadata.keys().collect::<Vec<_>>(),
        payload.registration.claims.keys().collect::<Vec<_>>()
    ));
    lines.push(format!(
        "2. UDR serializes and pushes the complete PushPayload to Intermediate NF (port 8082):\n{}",
        serde_json::to_string_pretty(&payload).unwrap()
    ));
    lines.push(
        "3. Intermediate NF reads basic fields (subscriber_id, slice) and forwards the full payload unchanged to AMF (port 8083)."
            .to_string(),
    );
    lines.push(format!(
        "4. AMF selects route from the payload: trigger={} priority={} applet={}",
        route.trigger,
        route.priority,
        route.applet_path.display()
    ));
    lines.push(format!(
        "5. AMF executes WASM applet dynamically and returns the authorization decision: {}",
        decision_str
    ));

    Ok(DemoReport {
        decision: Some(decision),
        lines,
    })
}

pub fn udr_emit_and_ue_register() -> Result<(SubscriptionData, UeRegistration)> {
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

mod visuals {
    use std::io::Write;
    use std::path::Path;
    use crate::types::{Decision, SubscriptionData};

    fn sleep(ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    pub fn start() {
        println!("\n\x1b[1;36m=== Starting Programmable Parameter Flow Simulation ===\x1b[0m\n");
        sleep(1000);
    }

    pub fn step1_emit(sub: &SubscriptionData) {
        println!("\x1b[1m[Step 1] UDR Database emits subscriber metadata & registration claims\x1b[0m");
        println!("  - subscriber_id: {}", sub.subscriber_id);
        println!("  - slice: {}", sub.slice);
        println!("  - metadata keys: \x1b[33m{:?}\x1b[0m", sub.metadata.keys().collect::<Vec<_>>());
        sleep(2000);
    }

    pub fn step2_send() {
        println!("\n\x1b[1m[Step 2] Sending PushPayload from UDR to Intermediate NF (port 8082)...\x1b[0m");
        for i in 0..15 {
            let arrow = "=".repeat(i);
            let remaining = "=".repeat(14 - i);
            print!("\r  \x1b[1;36m[ UDR ]\x1b[0m {arrow}\x1b[1;33m( PushPayload )\x1b[0m{remaining}> \x1b[1;33m[ Intermediate NF ]\x1b[0m");
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r  \x1b[1;36m[ UDR ]\x1b[0m ================================> \x1b[1;33m[ Intermediate NF ]\x1b[0m \x1b[1;32m[DELIVERED]\x1b[0m");
        sleep(1500);
    }

    pub fn step3_forward() {
        println!("\n\x1b[1m[Step 3] Intermediate NF reads slice headers and forwards payload unchanged to AMF (port 8083)...\x1b[0m");
        for i in 0..15 {
            let arrow = "=".repeat(i);
            let remaining = "=".repeat(14 - i);
            print!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m {arrow}\x1b[1;35m( Forward Payload )\x1b[0m{remaining}> \x1b[1;35m[ AMF ]\x1b[0m");
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m ================================> \x1b[1;35m[ AMF ]\x1b[0m \x1b[1;32m[DELIVERED]\x1b[0m");
        sleep(1500);
    }

    pub fn step4_wasm(applet_path: &Path) {
        println!("\n\x1b[1m[Step 4] AMF compiles WASM applet ({}) and executes verify()...\x1b[0m", applet_path.display());
        
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

        // Phase 1: Compilation
        for i in 0..10 {
            print!("\r  \x1b[1;35m[ AMF ]\x1b[0m Compiling WebAssembly Text (.wat) to bytecode... {}", spinner[i % spinner.len()]);
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r  \x1b[1;35m[ AMF ]\x1b[0m Compiling WebAssembly Text (.wat) to bytecode... \x1b[1;32m[DONE]\x1b[0m");

        // Phase 2: Linkage
        for i in 0..10 {
            print!("\r  \x1b[1;35m[ AMF ]\x1b[0m Linking host functions (metadata_matches_ue, metadata_is)... {}", spinner[(i + 3) % spinner.len()]);
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r  \x1b[1;35m[ AMF ]\x1b[0m Linking host functions (metadata_matches_ue, metadata_is)... \x1b[1;32m[DONE]\x1b[0m");

        // Phase 3: Execution
        for i in 0..8 {
            print!("\r  \x1b[1;35m[ AMF ]\x1b[0m Executing verify() entrypoint inside WASM sandbox... {}", spinner[(i + 6) % spinner.len()]);
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r  \x1b[1;35m[ AMF ]\x1b[0m Executing verify() entrypoint inside WASM sandbox... \x1b[1;32m[DONE]\x1b[0m");
        sleep(1000);
    }

    pub fn step5_return_amf(decision: Decision) {
        let decision_str = colorize_decision(decision);
        println!("\n\x1b[1m[Step 5] WASM VM returns decision. AMF sends decision back to Intermediate NF...\x1b[0m");
        for i in 0..15 {
            let arrow = "=".repeat(14 - i);
            let remaining = "=".repeat(i);
            print!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m <{arrow}( {decision_str} ){remaining} \x1b[1;35m[ AMF ]\x1b[0m");
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m <================================ \x1b[1;35m[ AMF ]\x1b[0m \x1b[1;32m[RETURNED]\x1b[0m");
        sleep(1500);
    }

    pub fn step6_return_udr(decision: Decision) {
        let decision_str = colorize_decision(decision);
        println!("\n\x1b[1m[Step 6] Intermediate NF returns decision to UDR client...\x1b[0m");
        for i in 0..15 {
            let arrow = "=".repeat(14 - i);
            let remaining = "=".repeat(i);
            print!("\r  \x1b[1;36m[ UDR ]\x1b[0m <{arrow}( {decision_str} ){remaining} \x1b[1;33m[ Intermediate NF ]\x1b[0m");
            let _ = std::io::stdout().flush();
            sleep(120);
        }
        println!("\r  \x1b[1;36m[ UDR ]\x1b[0m <================================ \x1b[1;33m[ Intermediate NF ]\x1b[0m \x1b[1;32m[COMPLETE]\x1b[0m\n");
        sleep(1000);
    }

    pub fn colorize_decision(decision: Decision) -> &'static str {
        match decision {
            Decision::Allow => "\x1b[1;32mALLOW\x1b[0m",
            Decision::LimitAccess => "\x1b[1;33mLIMIT_ACCESS\x1b[0m",
            Decision::Reject => "\x1b[1;31mREJECT\x1b[0m",
        }
    }
}
