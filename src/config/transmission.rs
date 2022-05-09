use std::fmt;

use chrono::Duration;
use gazebo::any::AnyLifetime;
use serde::Serialize;
use starlark::{starlark_simple_value, starlark_type, values::StarlarkValue};

/// A transmission instance
#[derive(Clone, PartialEq, Eq, Serialize, AnyLifetime)]
pub struct Transmission {
    pub url: String,
    pub user: Option<String>,
    pub password: Option<String>,
    #[serde(with = "parse_duration")]
    pub poll_interval: Duration,
}

impl fmt::Debug for Transmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transmission({})", self)
    }
}

impl fmt::Display for Transmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.user, &self.password) {
            (Some(user), Some(_password)) => write!(f, "{} # u:{}:***", self.url, user),
            (Some(user), None) => write!(f, "{} # u:{}", self.url, user),
            (None, _) => write!(f, "{}", self.url),
        }
    }
}

starlark_simple_value!(Transmission);
impl<'v> StarlarkValue<'v> for Transmission {
    starlark_type!("transmission");
}
