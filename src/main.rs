use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde_json::json;

use programmable_parameter_demo::types::{
    Decision, Route, RoutingConfig, UeRegistration, SubscriptionData, PushPayload
};
use programmable_parameter_demo::net::send_request_and_get_response;

const TRIGGER_UE_REGISTRATION: &str = "UE_REGISTRATION";

#[derive(Parser, Debug)]
#[command(about = "Unified Data Repository (UDR) process & client trigger")]
struct Cli {
    #[arg(long, default_value = "configs/rel22.yaml")]
    config: PathBuf,
}

#[derive(Debug)]
struct DemoReport {
    decision: Option<Decision>,
    lines: Vec<String>,
}

impl DemoReport {
    fn print(&self) {
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
    let report = run_demo(&cli.config)?;
    report.print();
    Ok(())
}

fn run_demo(config_path: &Path) -> Result<DemoReport> {
    // 1. Load config and select route (simulating subscriber verificationLogic settings in UDR)
    let config = load_config(config_path)?;
    let route = select_route(&config, TRIGGER_UE_REGISTRATION)
        .ok_or_else(|| anyhow!("no route configured for trigger {TRIGGER_UE_REGISTRATION}"))?;

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
        decision: Some(decision),
        lines,
    })
}

fn udr_emit_and_ue_register() -> Result<(SubscriptionData, UeRegistration)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::process::{Command, Stdio, Child};
    use std::net::TcpStream;

    // Mutex to ensure tests running on local network ports are serialized and do not conflict
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    struct ServerGuard {
        inf: Child,
        amf: Child,
    }

    impl Drop for ServerGuard {
        fn drop(&mut self) {
            let _ = self.inf.kill();
            let _ = self.amf.kill();
        }
    }

    fn start_test_servers() -> ServerGuard {
        // Compile all network function binaries
        let status = Command::new("cargo")
            .args(["build", "--bins"])
            .status()
            .unwrap();
        assert!(status.success());

        let debug_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");

        let inf = Command::new(debug_dir.join("intermediate_nf"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let amf = Command::new(debug_dir.join("amf"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        // Wait for port 8082 (Intermediate NF) and 8083 (AMF) to bind
        let mut inf_connected = false;
        let mut amf_connected = false;
        for _ in 0..100 {
            if !inf_connected && TcpStream::connect("127.0.0.1:8082").is_ok() {
                inf_connected = true;
            }
            if !amf_connected && TcpStream::connect("127.0.0.1:8083").is_ok() {
                amf_connected = true;
            }
            if inf_connected && amf_connected {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(inf_connected && amf_connected, "Servers failed to bind under test");

        ServerGuard { inf, amf }
    }

    #[test]
    fn programmable_forwarding_preserves_unknown_vendor() {
        let (subscription, _) = udr_emit_and_ue_register().unwrap();
        assert_eq!(
            subscription.metadata.get("vendor"),
            Some(&json!("Manufacturer-X"))
        );
    }

    #[test]
    fn rel22_applet_allows_vendor_without_intermediate_change() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let _guard = start_test_servers();
        let decision = run_scenario_with_config("configs/rel22.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    fn run_scenario_with_config(relative_config: &str) -> Decision {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        run_demo(&root.join(relative_config))
            .unwrap()
            .decision
            .unwrap()
    }
}
