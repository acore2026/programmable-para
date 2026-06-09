use std::collections::BTreeMap;
use std::path::PathBuf;
use std::io::Write;

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
            let color_decision = match decision {
                Decision::Allow => "\x1b[1;32mALLOW\x1b[0m",
                Decision::LimitAccess => "\x1b[1;33mLIMIT_ACCESS\x1b[0m",
                Decision::Reject => "\x1b[1;31mREJECT\x1b[0m",
            };
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

    println!("\n\x1b[1;36m=== Starting Programmable Parameter Flow Simulation ===\x1b[0m\n");
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Animation: Step 1 (UDR preparing data)
    println!("\x1b[1m[Step 1] UDR Database emits subscriber metadata & registration claims\x1b[0m");
    println!("  - subscriber_id: {}", subscription.subscriber_id);
    println!("  - slice: {}", subscription.slice);
    println!("  - metadata keys: \x1b[33m{:?}\x1b[0m", subscription.metadata.keys().collect::<Vec<_>>());
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Animation: Step 2 (UDR -> Intermediate NF)
    println!("\n\x1b[1m[Step 2] Sending PushPayload from UDR to Intermediate NF (port 8082)...\x1b[0m");
    for i in 0..15 {
        let arrow = "=".repeat(i);
        let remaining = "=".repeat(14 - i);
        print!("\r  \x1b[1;36m[ UDR ]\x1b[0m {arrow}\x1b[1;33m( PushPayload )\x1b[0m{remaining}> \x1b[1;33m[ Intermediate NF ]\x1b[0m");
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    println!("\r  \x1b[1;36m[ UDR ]\x1b[0m ================================> \x1b[1;33m[ Intermediate NF ]\x1b[0m \x1b[1;32m[DELIVERED]\x1b[0m");
    std::thread::sleep(std::time::Duration::from_millis(800));

    // 4. Connect to Intermediate NF (port 8082) and push the payload
    let decision: Decision = send_request_and_get_response("127.0.0.1:8082", &payload)
        .context("Could not connect to Intermediate NF at 127.0.0.1:8082. Are the Intermediate NF and AMF servers running?")?;

    // Animation: Step 3 (Intermediate NF processes & forwards to AMF)
    println!("\n\x1b[1m[Step 3] Intermediate NF reads slice headers and forwards payload unchanged to AMF (port 8083)...\x1b[0m");
    for i in 0..15 {
        let arrow = "=".repeat(i);
        let remaining = "=".repeat(14 - i);
        print!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m {arrow}\x1b[1;35m( Forward Payload )\x1b[0m{remaining}> \x1b[1;35m[ AMF ]\x1b[0m");
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    println!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m ================================> \x1b[1;35m[ AMF ]\x1b[0m \x1b[1;32m[DELIVERED]\x1b[0m");
    std::thread::sleep(std::time::Duration::from_millis(800));

    // Animation: Step 4 (AMF executes WASM verification)
    println!("\n\x1b[1m[Step 4] AMF compiles WASM applet ({}) and executes verify()...\x1b[0m", route.applet_path.display());
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Animation: Step 5 (AMF returns decision to Intermediate NF)
    let decision_str = match decision {
        Decision::Allow => "\x1b[1;32mALLOW\x1b[0m",
        Decision::LimitAccess => "\x1b[1;33mLIMIT_ACCESS\x1b[0m",
        Decision::Reject => "\x1b[1;31mREJECT\x1b[0m",
    };
    println!("\n\x1b[1m[Step 5] WASM VM returns decision. AMF sends decision back to Intermediate NF...\x1b[0m");
    for i in 0..15 {
        let arrow = "=".repeat(14 - i);
        let remaining = "=".repeat(i);
        print!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m <{arrow}( {decision_str} ){remaining} \x1b[1;35m[ AMF ]\x1b[0m");
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    println!("\r                                \x1b[1;33m[ Intermediate NF ]\x1b[0m <================================ \x1b[1;35m[ AMF ]\x1b[0m \x1b[1;32m[RETURNED]\x1b[0m");
    std::thread::sleep(std::time::Duration::from_millis(800));

    // Animation: Step 6 (Intermediate NF returns decision to UDR)
    println!("\n\x1b[1m[Step 6] Intermediate NF returns decision to UDR client...\x1b[0m");
    for i in 0..15 {
        let arrow = "=".repeat(14 - i);
        let remaining = "=".repeat(i);
        print!("\r  \x1b[1;36m[ UDR ]\x1b[0m <{arrow}( {decision_str} ){remaining} \x1b[1;33m[ Intermediate NF ]\x1b[0m");
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    println!("\r  \x1b[1;36m[ UDR ]\x1b[0m <================================ \x1b[1;33m[ Intermediate NF ]\x1b[0m \x1b[1;32m[COMPLETE]\x1b[0m\n");
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 5. Format lines for output
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
