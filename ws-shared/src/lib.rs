use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Msg {
    pub did: String,
    pub timestamp: u64,
    pub data: MsgData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerResp {
    pub timestamp: u64,
    pub data: ServerRespData,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MsgData {
    Join,
    Leave,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ServerRespData {
    List(Vec<String>),
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

    pub fn leave(did: &str) -> Self {
        Msg::new(did, MsgData::Leave)
    }
}

impl ServerResp {
    pub fn new(data: ServerRespData) -> Self {
        ServerResp {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            data,
        }
    }
    pub fn list(dids: Vec<String>) -> Self {
        ServerResp::new(ServerRespData::List(dids))
    }
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

impl TryFrom<&str> for ServerResp {
    type Error = serde_json::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(s)
    }
}

impl TryFrom<&ServerResp> for String {
    type Error = serde_json::Error;

    fn try_from(msg: &ServerResp) -> Result<Self, Self::Error> {
        serde_json::to_string(msg)
    }
}
