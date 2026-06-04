use std::collections::BTreeMap;
use std::fs;
use anyhow::{Context, Result};
use serde_json::Value;
use wasmtime::{Caller, Engine, Linker, Module, Store, TypedFunc};

use crate::types::{Decision, HostState, Route, SubscriptionData, UeRegistration};

const KEY_AI_AGENT_ID: i32 = 1;
const KEY_TRUST_LEVEL: i32 = 2;
const KEY_VENDOR: i32 = 3;
const VALUE_HIGH: i32 = 1;

pub fn amf_verify(
    subscription: &SubscriptionData,
    registration: &UeRegistration,
    route: &Route,
) -> Result<Decision> {
    if subscription.subscriber_id != registration.subscriber_id {
        return Ok(Decision::Reject);
    }

    let wasm = fs::read_to_string(&route.applet_path)
        .with_context(|| format!("failed to read applet {}", route.applet_path.display()))?;
    let wasm = wat::parse_str(&wasm).with_context(|| {
        format!(
            "failed to compile WAT applet {}",
            route.applet_path.display()
        )
    })?;

    let engine = Engine::default();
    let module = Module::new(&engine, wasm)?;
    let mut linker = Linker::new(&engine);

    linker.func_wrap(
        "host",
        "metadata_matches_ue",
        |caller: Caller<'_, HostState>, metadata_key: i32, claim_key: i32| -> i32 {
            bool_code(
                metadata_value(&caller.data().metadata, metadata_key)
                    == metadata_value(&caller.data().ue_claims, claim_key),
            )
        },
    )?;
    linker.func_wrap(
        "host",
        "metadata_is",
        |caller: Caller<'_, HostState>, metadata_key: i32, expected_value: i32| -> i32 {
            bool_code(
                metadata_value(&caller.data().metadata, metadata_key).and_then(Value::as_str)
                    == expected_string(expected_value),
            )
        },
    )?;
    linker.func_wrap(
        "host",
        "mismatch_action",
        |caller: Caller<'_, HostState>| -> i32 { caller.data().action_on_mismatch as i32 },
    )?;

    let host_state = HostState {
        metadata: subscription.metadata.clone(),
        ue_claims: registration.claims.clone(),
        action_on_mismatch: route.action_on_mismatch,
    };
    let mut store = Store::new(&engine, host_state);
    let instance = linker.instantiate(&mut store, &module)?;
    let verify: TypedFunc<(), i32> = instance.get_typed_func(&mut store, "verify")?;
    Decision::from_wasm_code(verify.call(&mut store, ())?)
}

fn metadata_value(values: &BTreeMap<String, Value>, key_id: i32) -> Option<&Value> {
    values.get(metadata_key_name(key_id)?)
}

fn metadata_key_name(key_id: i32) -> Option<&'static str> {
    match key_id {
        KEY_AI_AGENT_ID => Some("aiAgentId"),
        KEY_TRUST_LEVEL => Some("trustLevel"),
        KEY_VENDOR => Some("vendor"),
        _ => None,
    }
}

fn expected_string(value_id: i32) -> Option<&'static str> {
    match value_id {
        VALUE_HIGH => Some("high"),
        _ => None,
    }
}

fn bool_code(value: bool) -> i32 {
    if value {
        1
    } else {
        0
    }
}
