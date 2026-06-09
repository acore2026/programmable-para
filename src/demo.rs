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
            println!("Decision: {decision}");
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

    // 4. Connect to Intermediate NF (port 8082) and push the payload
    let decision: Decision = send_request_and_get_response("127.0.0.1:8082", &payload)
        .context("Could not connect to Intermediate NF at 127.0.0.1:8082. Are the Intermediate NF and AMF servers running?")?;

    // 5. Format lines for output
    let mut lines = Vec::new();
    lines.push("=== UDR CLIENT REPORT ===".to_string());
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
        decision
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
