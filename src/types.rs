use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Result, bail};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionData {
    pub subscriber_id: String,
    pub slice: String,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UeRegistration {
    pub subscriber_id: String,
    pub claims: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct StrictRel21Subscription {
    pub subscriber_id: String,
    pub slice: String,
    pub ai_agent_id: String,
    pub trust_level: String,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Allow = 0,
    LimitAccess = 1,
    Reject = 2,
}

impl Decision {
    pub fn from_wasm_code(code: i32) -> Result<Self> {
        match code {
            0 => Ok(Self::Allow),
            1 => Ok(Self::LimitAccess),
            2 => Ok(Self::Reject),
            other => bail!("WASM applet returned unknown decision code {other}"),
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => write!(f, "ALLOW"),
            Self::LimitAccess => write!(f, "LIMIT_ACCESS"),
            Self::Reject => write!(f, "REJECT"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RoutingConfig {
    pub routes: Vec<Route>,
}

#[derive(Debug, Deserialize)]
pub struct Route {
    pub trigger: String,
    pub priority: u32,
    pub applet_path: PathBuf,
    pub action_on_mismatch: Decision,
}

#[derive(Clone, Debug)]
pub struct HostState {
    pub metadata: BTreeMap<String, Value>,
    pub ue_claims: BTreeMap<String, Value>,
    pub action_on_mismatch: Decision,
}
