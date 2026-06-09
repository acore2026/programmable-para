use std::sync::Mutex;
use std::process::{Command, Stdio, Child};
use std::net::TcpStream;
use std::path::PathBuf;
use serde_json::json;

use programmable_parameter_demo::demo::{run_demo, udr_emit_and_ue_register};
use programmable_parameter_demo::types::Decision;

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

    let debug_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("debug");

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
    let decision = run_demo().unwrap().decision.unwrap();
    assert_eq!(decision, Decision::Allow);
}
