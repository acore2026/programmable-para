mod types;
mod wasm;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, ValueEnum};
use serde_json::json;

use crate::types::{Decision, Route, RoutingConfig, StrictRel21Subscription, SubscriptionData, UeRegistration};
use crate::wasm::amf_verify;

const TRIGGER_UE_REGISTRATION: &str = "UE_REGISTRATION";

#[derive(Parser, Debug)]
#[command(about = "Runnable demo for 3GPP-style programmable parameters")]
struct Cli {
    #[arg(long, value_enum, default_value_t = Scenario::Rel22VendorPass)]
    scenario: Scenario,

    #[arg(long, default_value = "configs/rel22.yaml")]
    config: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Scenario {
    StrictBreaks,
    Rel21Pass,
    Rel22VendorPass,
    VendorMismatch,
}

#[derive(Debug)]
struct DemoReport {
    scenario: Scenario,
    decision: Option<Decision>,
    lines: Vec<String>,
}

impl DemoReport {
    fn print(&self) {
        println!("Scenario: {:?}", self.scenario);
        for line in &self.lines {
            println!("{line}");
        }
        if let Some(decision) = self.decision {
            println!("Decision: {decision}");
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = run_demo(cli.scenario, &cli.config)?;
    report.print();
    Ok(())
}

fn run_demo(scenario: Scenario, config_path: &Path) -> Result<DemoReport> {
    if matches!(scenario, Scenario::StrictBreaks) {
        return strict_schema_demo();
    }

    let config = load_config(config_path)?;
    let route = select_route(&config, TRIGGER_UE_REGISTRATION)
        .ok_or_else(|| anyhow!("no route configured for trigger {TRIGGER_UE_REGISTRATION}"))?;
    let (subscription, registration) = udr_emit_and_ue_register(scenario)?;
    let forwarded = intermediate_nf_forward(subscription.clone());
    let decision = amf_verify(&forwarded, &registration, route)?;

    let mut lines = Vec::new();
    lines.push(format!(
        "1. UDR emits subscription metadata keys: {:?}",
        subscription.metadata.keys().collect::<Vec<_>>()
    ));
    lines.push(
        "2. Intermediate NF reads subscriber_id and slice, then forwards metadata unchanged."
            .to_string(),
    );
    lines.push(format!(
        "3. AMF selects route trigger={} priority={} applet={}",
        route.trigger,
        route.priority,
        route.applet_path.display()
    ));
    lines.push(format!(
        "4. WASM applet returns a host-defined decision using action_on_mismatch={}.",
        route.action_on_mismatch
    ));

    Ok(DemoReport {
        scenario,
        decision: Some(decision),
        lines,
    })
}

fn strict_schema_demo() -> Result<DemoReport> {
    let rel22_inline_payload = json!({
        "subscriber_id": "imsi-001010000000001",
        "slice": "enterprise-ai",
        "ai_agent_id": "urn:3gpp:ai-agent:auto-pilot-v2",
        "trust_level": "high",
        "vendor": "Manufacturer-X"
    });

    let err = serde_json::from_value::<StrictRel21Subscription>(rel22_inline_payload)
        .expect_err("strict Rel-21 schema must reject the new vendor parameter");

    Ok(DemoReport {
        scenario: Scenario::StrictBreaks,
        decision: None,
        lines: vec![
            "1. UDR emits Rel-22 data with a new inline vendor parameter.".to_string(),
            "2. A strict Rel-21 intermediate NF tries to deserialize the full payload.".to_string(),
            format!("3. The NF fails before AMF can see the data: {err}"),
            "Result: this is the pass-through bottleneck the proposal is trying to avoid."
                .to_string(),
        ],
    })
}

fn load_config(path: &Path) -> Result<RoutingConfig> {
    let body = fs::read_to_string(path)
        .with_context(|| format!("failed to read routing config {}", path.display()))?;
    let mut config: RoutingConfig = serde_yaml::from_str(&body)
        .with_context(|| format!("failed to parse routing config {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

    for route in &mut config.routes {
        if route.applet_path.is_relative() {
            route.applet_path = base_dir.join(&route.applet_path);
        }
    }

    Ok(config)
}

fn select_route<'a>(config: &'a RoutingConfig, trigger: &str) -> Option<&'a Route> {
    config
        .routes
        .iter()
        .filter(|route| route.trigger == trigger)
        .max_by_key(|route| route.priority)
}

fn udr_emit_and_ue_register(scenario: Scenario) -> Result<(SubscriptionData, UeRegistration)> {
    let vendor_claim = match scenario {
        Scenario::VendorMismatch => "Manufacturer-Y",
        Scenario::Rel21Pass | Scenario::Rel22VendorPass => "Manufacturer-X",
        Scenario::StrictBreaks => bail!("strict scenario does not use metadata payloads"),
    };

    let mut metadata = BTreeMap::from([
        (
            "aiAgentId".to_string(),
            json!("urn:3gpp:ai-agent:auto-pilot-v2"),
        ),
        ("trustLevel".to_string(), json!("high")),
    ]);

    if !matches!(scenario, Scenario::Rel21Pass) {
        metadata.insert("vendor".to_string(), json!("Manufacturer-X"));
    }

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
            ("vendor".to_string(), json!(vendor_claim)),
        ]),
    };

    Ok((subscription, registration))
}

fn intermediate_nf_forward(subscription: SubscriptionData) -> SubscriptionData {
    let _known_fields = (&subscription.subscriber_id, &subscription.slice);
    subscription
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_schema_rejects_new_inline_vendor() {
        let report = strict_schema_demo().unwrap();
        assert!(report
            .lines
            .iter()
            .any(|line| line.contains("unknown field")));
    }

    #[test]
    fn programmable_forwarding_preserves_unknown_vendor() {
        let (subscription, _) = udr_emit_and_ue_register(Scenario::Rel22VendorPass).unwrap();
        let forwarded = intermediate_nf_forward(subscription);
        assert_eq!(
            forwarded.metadata.get("vendor"),
            Some(&json!("Manufacturer-X"))
        );
    }

    #[test]
    fn rel21_applet_allows_identity_without_vendor() {
        let decision = run_scenario_with_config(Scenario::Rel21Pass, "configs/rel21.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    #[test]
    fn rel22_applet_allows_vendor_without_intermediate_change() {
        let decision = run_scenario_with_config(Scenario::Rel22VendorPass, "configs/rel22.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    #[test]
    fn rel22_applet_limits_vendor_mismatch() {
        let decision = run_scenario_with_config(Scenario::VendorMismatch, "configs/rel22.yaml");
        assert_eq!(decision, Decision::LimitAccess);
    }

    fn run_scenario_with_config(scenario: Scenario, relative_config: &str) -> Decision {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        run_demo(scenario, &root.join(relative_config))
            .unwrap()
            .decision
            .unwrap()
    }
}
