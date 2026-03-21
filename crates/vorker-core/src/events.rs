use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupervisorEvent {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub timestamp: String,
    pub payload: Value,
}

#[must_use]
pub fn create_supervisor_event(kind: &str, payload: Value) -> SupervisorEvent {
    SupervisorEvent {
        id: Uuid::new_v4().to_string(),
        kind: kind.to_owned(),
        timestamp: now_iso(),
        payload,
    }
}

#[must_use]
pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("rfc3339 timestamp should format")
}
