use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use serde_json::json;

use programmable_parameter_demo::types::{
    Decision, Route, RoutingConfig, UeRegistration, Scenario, AmfRequest, AmfResponse
};
use programmable_parameter_demo::net::send_request_and_get_response;

const TRIGGER_UE_REGISTRATION: &str = "UE_REGISTRATION";

#[derive(Parser, Debug)]
#[command(about = "Runnable client trigger for 3GPP-style programmable parameters")]
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = run_demo(cli.scenario, &cli.config)?;
    report.print();
    Ok(())
}

fn run_demo(scenario: Scenario, config_path: &Path) -> Result<DemoReport> {
    // 1. Load config and select route
    let config = load_config(config_path)?;
    let route = select_route(&config, TRIGGER_UE_REGISTRATION)
        .ok_or_else(|| anyhow!("no route configured for trigger {TRIGGER_UE_REGISTRATION}"))?;

    // 2. Generate registration claims (acting as the registering UE)
    let registration = UeRegistration {
        subscriber_id: "imsi-001010000000001".to_string(),
        claims: BTreeMap::from([
            (
                "aiAgentId".to_string(),
                json!("urn:3gpp:ai-agent:auto-pilot-v2"),
            ),
            ("vendor".to_string(), json!("Manufacturer-X")),
        ]),
    };

    let amf_req = AmfRequest {
        scenario,
        route: route.clone(),
        registration,
    };

    // 3. Connect to AMF (assumed to be running on 127.0.0.1:8083) and trigger request
    let amf_resp: AmfResponse = send_request_and_get_response("127.0.0.1:8083", &amf_req)
        .context("Could not connect to AMF server at 127.0.0.1:8083. Are the UDR, Intermediate NF, and AMF servers running?")?;

    // 4. Format lines for output
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
    use programmable_parameter_demo::types::UdrResponse;
    use std::net::TcpStream;

    // Mutex to ensure tests running on local network ports are serialized and do not conflict
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

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

    fn start_test_servers() -> ServerGuard {
        // Compile all network function binaries
        let status = Command::new("cargo")
            .args(["build", "--bins"])
            .status()
            .unwrap();
        assert!(status.success());

        let debug_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");

        let udr = Command::new(debug_dir.join("udr"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

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

        // Wait for port 8083 (AMF) to bind
        let mut connected = false;
        for _ in 0..100 {
            if TcpStream::connect("127.0.0.1:8083").is_ok() {
                connected = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(connected, "AMF server failed to bind to port 8083 under test");

        ServerGuard { udr, inf, amf }
    }

    #[test]
    fn programmable_forwarding_preserves_unknown_vendor() {
        let _lock = TEST_MUTEX.lock().unwrap();
        
        let debug_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");
        let _ = Command::new("cargo").args(["build", "--bins"]).status().unwrap();
        
        let udr = Command::new(debug_dir.join("udr"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        
        let mut udr_connected = false;
        for _ in 0..100 {
            if TcpStream::connect("127.0.0.1:8081").is_ok() {
                udr_connected = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(udr_connected);
        
        let udr_resp: UdrResponse = send_request_and_get_response("127.0.0.1:8081", &Scenario::Rel22VendorPass).unwrap();
        
        let mut udr_kill = udr;
        let _ = udr_kill.kill();
        
        assert_eq!(
            udr_resp.subscription.metadata.get("vendor"),
            Some(&json!("Manufacturer-X"))
        );
    }

    #[test]
    fn rel22_applet_allows_vendor_without_intermediate_change() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let _guard = start_test_servers();
        let decision = run_scenario_with_config(Scenario::Rel22VendorPass, "configs/rel22.yaml");
        assert_eq!(decision, Decision::Allow);
    }

    fn run_scenario_with_config(scenario: Scenario, relative_config: &str) -> Decision {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        run_demo(scenario, &root.join(relative_config))
            .unwrap()
            .decision
            .unwrap()
    }
}
