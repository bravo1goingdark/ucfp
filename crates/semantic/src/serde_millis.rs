//! Serialization helpers for Duration types.
//!
//! Provides serde serialization for std::time::Duration as milliseconds.

use serde::{Deserialize, Deserializer, Serializer};
use std::time::Duration;

/// Serialize Duration as milliseconds.
#[allow(dead_code)]
pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(duration.as_millis() as u64)
}

/// Deserialize Duration from milliseconds.
#[allow(dead_code)]
pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    Ok(Duration::from_millis(millis))
}
