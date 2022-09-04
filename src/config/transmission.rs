use std::fmt;

use chrono::Duration;
use gazebo::any::AnyLifetime;
use starlark::{
    starlark_simple_value, starlark_type,
    values::{NoSerialize, StarlarkValue},
};

pub const DEFAULT_POLL_INTERVAL_MINS: i64 = 5;

/// A transmission instance
#[derive(Clone, PartialEq, Eq, NoSerialize, AnyLifetime)]
pub struct Transmission {
    pub url: String,
    pub user: Option<String>,
    pub password: Option<String>,
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

use rhai::plugin::*;

#[export_module]
mod rhai_transmission {
    #[rhai_fn(global)]
    pub fn transmission(url: &str) -> Transmission {
        Transmission {
            url: url.to_string(),
            user: None,
            password: None,
            poll_interval: Duration::minutes(DEFAULT_POLL_INTERVAL_MINS),
        }
    }

    pub fn with_user(mut transmission: Transmission, user: &str) -> Transmission {
        transmission.user = Some(user.to_string());
        transmission
    }

    pub fn with_password(mut transmission: Transmission, password: &str) -> Transmission {
        transmission.password = Some(password.to_string());
        transmission
    }

    pub fn with_poll_interval_minutes(
        mut transmission: Transmission,
        minutes: i64,
    ) -> Transmission {
        transmission.poll_interval = Duration::minutes(minutes);
        transmission
    }
}
