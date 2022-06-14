use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Msg {
    pub did: String,
    pub timestamp: u64,
    pub data: MsgData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MsgData {
    Join,
}

impl TryFrom<&str> for Msg {
    type Error = serde_json::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(s)
    }
}

impl TryFrom<&Msg> for String {
    type Error = serde_json::Error;

    fn try_from(msg: &Msg) -> Result<Self, Self::Error> {
        serde_json::to_string(msg)
    }
}

impl Msg {
    pub fn new(did: &str, data: MsgData) -> Self {
        Msg {
            did: did.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            data,
        }
    }

    pub fn join(did: &str) -> Self {
        Msg::new(did, MsgData::Join)
    }
}
