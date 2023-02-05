use std::fmt;

use crate::util::chrono_duration;
use chrono::Duration;
use rhai::EvalAltResult;
use serde::{Deserialize, Serialize};

pub const DEFAULT_POLL_INTERVAL_MINS: i64 = 5;

/// A transmission instance
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transmission {
    pub url: String,
    pub user: Option<String>,
    pub password: Option<String>,
    #[serde(with = "chrono_duration")]
    pub poll_interval: Duration,
}

impl Transmission {
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
