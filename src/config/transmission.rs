use std::fmt;

use crate::util::chrono_duration;
use chrono::Duration;
use rhai::{CustomType, EvalAltResult, TypeBuilder};
use serde::{Deserialize, Serialize};

pub const DEFAULT_POLL_INTERVAL_MINS: i64 = 5;

/// A transmission instance
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, CustomType)]
#[rhai_type(extra = Self::build_rhai)]
pub struct Transmission {
    #[rhai_type(readonly)]
    pub url: String,
    #[rhai_type(readonly)]
    pub user: Option<String>,
    #[rhai_type(readonly)]
    pub password: Option<String>,
    #[rhai_type(readonly)]
    #[serde(with = "chrono_duration")]
    pub poll_interval: Duration,
}

impl Transmission {
    fn build_rhai(builder: &mut TypeBuilder<Self>) {
        builder
            .with_fn("transmission", Self::new)
            .with_fn("user", Self::with_user)
            .with_fn("password", Self::with_password)
            .with_fn("poll_interval", Self::with_poll_interval);
    }

    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            user: None,
            password: None,
            poll_interval: Duration::minutes(DEFAULT_POLL_INTERVAL_MINS),
        }
    }

    pub fn with_user(mut self, user: &str) -> Self {
        self.user = Some(user.to_string());
        self
    }

    pub fn with_password(mut self, password: &str) -> Self {
        self.password = Some(password.to_string());
        self
    }

    pub fn with_poll_interval(mut self, interval: &str) -> Result<Self, Box<EvalAltResult>> {
        self.poll_interval =
            Duration::from_std(parse_duration::parse(interval).map_err(|e| format!("{e}"))?)
                .map_err(|e| format!("{e}"))?;
        Ok(self)
    }
}

impl fmt::Debug for Transmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transmission({self})")
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
