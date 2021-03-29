use serde::Serialize;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Serialize)]
pub struct RequestId {
    id: String,
}

impl RequestId {
    pub fn generate() -> RequestId {
        RequestId {
            id: Uuid::new_v4().to_hyphenated().to_string(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "id: {}", self.id.as_str())
    }
}
