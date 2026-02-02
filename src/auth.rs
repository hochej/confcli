use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthMethod {
    Basic { email: String, token: String },
    Bearer { token: String },
}

impl AuthMethod {
    pub fn is_basic(&self) -> bool {
        matches!(self, AuthMethod::Basic { .. })
    }

    pub fn description(&self) -> &'static str {
        match self {
            AuthMethod::Basic { .. } => "basic",
            AuthMethod::Bearer { .. } => "bearer",
        }
    }
}
