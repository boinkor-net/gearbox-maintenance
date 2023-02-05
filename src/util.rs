/// Parses and serializes an optional chrono duration
pub mod chrono_optional_duration {
    use chrono::Duration;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Some(
            Duration::from_std(parse_duration::parse(&s).map_err(serde::de::Error::custom)?)
                .map_err(serde::de::Error::custom)?,
        ))
    }

    pub fn serialize<S>(dur: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(dur) = dur {
            let s = format!("{dur}");
            serializer.serialize_str(&s)
        } else {
            serializer.serialize_str("")
        }
    }
}

/// Parses and serializes a regular chrono duration
pub mod chrono_duration {
    use chrono::Duration;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Duration::from_std(parse_duration::parse(&s).map_err(serde::de::Error::custom)?)
            .map_err(serde::de::Error::custom)
    }

    pub fn serialize<S>(dur: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{dur}");
        serializer.serialize_str(&s)
    }
}
