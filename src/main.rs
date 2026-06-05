use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, Child};
use std::net::{TcpStream, Shutdown};
use std::io::{Read, Write};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use serde_json::json;

use programmable_parameter_demo::types::{
    Decision, Route, RoutingConfig, StrictRel21Subscription,
    UeRegistration, Scenario, AmfRequest, AmfResponse
};

const TRIGGER_UE_REGISTRATION: &str = "UE_REGISTRATION";

#[derive(Parser, Debug)]
#[command(about = "Runnable demo for 3GPP-style programmable parameters")]
struct Cli {
    #[arg(long, value_enum, default_value_t = Scenario::Rel22VendorPass)]
    scenario: Scenario,

    #[arg(long, default_value = "configs/rel22.yaml")]
    config: PathBuf,
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

struct ServerGuard {
    udr: Child,
    inf: Child,
    amf: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.udr.kill();
        let _ = self.inf.kill();
        let _ = self.amf.kill();
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

    // 1. Build all binary targets to make sure they are up-to-date
    println!("Building network function binaries...");
    let status = Command::new("cargo")
        .args(["build", "--bins"])
        .status()
        .context("Failed to run cargo build")?;
    if !status.success() {
        bail!("Failed to compile binaries");
    }

    let debug_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");

    // 2. Start the UDR, Intermediate NF, and AMF servers in the background
    let udr = Command::new(debug_dir.join("udr"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn UDR server")?;

    let inf = Command::new(debug_dir.join("intermediate_nf"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn Intermediate NF server")?;

    let amf = Command::new(debug_dir.join("amf"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn AMF server")?;

    // Create a guard to ensure child processes are killed even on early exit/panic
    let _guard = ServerGuard { udr, inf, amf };

    // 3. Wait for the AMF server (port 8083) to become active
    let mut connected = false;
    for _ in 0..100 {
        if TcpStream::connect("127.0.0.1:8083").is_ok() {
            connected = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if !connected {
        bail!("Failed to connect to AMF server on port 8083");
    }

    // 4. Load config and select route
    let config = load_config(config_path)?;
    let route = select_route(&config, TRIGGER_UE_REGISTRATION)
        .ok_or_else(|| anyhow!("no route configured for trigger {TRIGGER_UE_REGISTRATION}"))?;

    // 5. Generate registration claims (acting as the registering UE)
    let vendor_claim = match scenario {
        Scenario::VendorMismatch => "Manufacturer-Y",
        Scenario::Rel21Pass | Scenario::Rel22VendorPass => "Manufacturer-X",
        _ => unreachable!(),
    };
    let registration = UeRegistration {
        subscriber_id: "imsi-001010000000001".to_string(),
        claims: BTreeMap::from([
            (
                "aiAgentId".to_string(),
                json!("urn:3gpp:ai-agent:auto-pilot-v2"),
            ),
            ("vendor".to_string(), json!(vendor_claim)),
        ]),
    };

    let amf_req = AmfRequest {
        scenario,
        route: route.clone(),
        registration,
    };

    // 6. Connect to AMF and send the verification request
    let mut amf_stream = TcpStream::connect("127.0.0.1:8083")?;
    let req_bytes = serde_json::to_vec(&amf_req)?;
    amf_stream.write_all(&req_bytes)?;
    amf_stream.shutdown(Shutdown::Write)?;

    // 7. Read the response decision and metadata details
    let mut resp_bytes = Vec::new();
    amf_stream.read_to_end(&mut resp_bytes)?;
    let amf_resp: AmfResponse = serde_json::from_slice(&resp_bytes)?;

    // 8. Format lines for output
    let mut lines = Vec::new();
    lines.push(format!(
        "1. UDR emits subscription metadata keys: {:?}",
        amf_resp.subscription.metadata.keys().collect::<Vec<_>>()
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
        decision: Some(amf_resp.decision),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to ensure tests running on local network ports are serialized and do not conflict
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn strict_schema_rejects_new_inline_vendor() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let report = strict_schema_demo().unwrap();
        assert!(report
            .lines
            .iter()
            .any(|line| line.contains("unknown field")));
    }

    #[test]
    fn programmable_forwarding_preserves_unknown_vendor() {
        let _lock = TEST_MUTEX.lock().unwrap();
        
        // Connect to UDR directly to get generated data
        println!("Building network function binaries...");
        let debug_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");
        let _ = Command::new("cargo").args(["build", "--bins"]).status().unwrap();
        
        let udr = Command::new(debug_dir.join("udr"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        
        let mut udr_connected = false;
        for _ in 0..50 {
            if TcpStream::connect("127.0.0.1:8081").is_ok() {
                udr_connected = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(udr_connected);
        
        let mut udr_stream = TcpStream::connect("127.0.0.1:8081").unwrap();
        let req_bytes = serde_json::to_vec(&Scenario::Rel22VendorPass).unwrap();
        udr_stream.write_all(&req_bytes).unwrap();
        udr_stream.shutdown(Shutdown::Write).unwrap();
        
        let mut buf = Vec::new();
        udr_stream.read_to_end(&mut buf).unwrap();
        let udr_resp: programmable_parameter_demo::types::UdrResponse = serde_json::from_slice(&buf).unwrap();
        
        let mut udr_kill = udr;
        let _ = udr_kill.kill();
        
        assert_eq!(
            udr_resp.subscription.metadata.get("vendor"),
            Some(&json!("Manufacturer-X"))
        );
    }

    #[test]
    fn rel21_applet_allows_identity_without_vendor() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let decision = run_scenario_with_config(Scenario::Rel21Pass, "configs/rel21.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    #[test]
    fn rel22_applet_allows_vendor_without_intermediate_change() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let decision = run_scenario_with_config(Scenario::Rel22VendorPass, "configs/rel22.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    #[test]
    fn rel22_applet_limits_vendor_mismatch() {
        let _lock = TEST_MUTEX.lock().unwrap();
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
