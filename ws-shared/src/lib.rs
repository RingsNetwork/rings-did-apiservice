use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, Clone)]
#[repr(u8)]
pub enum DidType {
    DEFAULT = 0,
    ED25519 = 1,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct Did {
    pub id: String,
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub type_: DidType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Msg {
    pub did: Did,
    pub data: MsgData,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ServerResp {
    pub data: ServerRespData,
    pub timestamp: u64,
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
    List(Vec<Did>),
}

impl Did {
    pub fn default(id: &str) -> Self {
        Self {
            id: id.to_string(),
            type_: DidType::DEFAULT,
        }
    }
    pub fn ed25519(id: &str) -> Self {
        Self {
            id: id.to_string(),
            type_: DidType::ED25519,
        }
    }
}

impl Msg {
    pub fn new(did: Did, data: MsgData) -> Self {
        Msg {
            did,
            data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn join(did: Did) -> Self {
        Msg::new(did, MsgData::Join)
    }

    pub fn leave(did: Did) -> Self {
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
    pub fn list(dids: Vec<Did>) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_str_to_msg() {
        let s = r#"{"did":{"id":"0x77FdC5c3937E509d2aB0DeAA84bD47741B58d0d9","type":0},"timestamp":1683788104386,"data":"join"}"#;
        let _: Msg = s.try_into().unwrap();
    }
}
