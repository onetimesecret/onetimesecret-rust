//! Serde helpers for the API's scalar conventions.
//!
//! Timestamps are Unix epoch seconds, sometimes fractional; durations are
//! whole seconds. Both have been sent as numbers and as numeric strings,
//! and booleans as `true` and as `"true"`, so every helper here accepts
//! both forms.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde::de::{self, Deserializer, Unexpected};

#[derive(Deserialize)]
#[serde(untagged)]
enum Scalar {
    Number(f64),
    Bool(bool),
    Text(String),
}

fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    match Option::<Scalar>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Scalar::Number(n)) => Ok(Some(n)),
        Some(Scalar::Text(s)) if s.trim().is_empty() => Ok(None),
        Some(Scalar::Text(s)) => s
            .trim()
            .parse::<f64>()
            .map(Some)
            .map_err(|_| de::Error::invalid_value(Unexpected::Str(&s), &"a number")),
        Some(Scalar::Bool(b)) => Err(de::Error::invalid_type(Unexpected::Bool(b), &"a number")),
    }
}

fn finite_non_negative<E: de::Error>(n: f64) -> Result<f64, E> {
    if n.is_finite() && n >= 0.0 {
        Ok(n)
    } else {
        Err(de::Error::invalid_value(
            Unexpected::Float(n),
            &"a non-negative number",
        ))
    }
}

/// Epoch seconds (number or string) to [`SystemTime`].
pub(crate) mod timestamp {
    use super::{Deserializer, Duration, SystemTime, UNIX_EPOCH, de, finite_non_negative, number};

    pub(crate) fn opt<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<SystemTime>, D::Error> {
        number(deserializer)?
            .map(|n| finite_non_negative(n).map(|n| UNIX_EPOCH + Duration::from_secs_f64(n)))
            .transpose()
    }

    pub(crate) fn required<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SystemTime, D::Error> {
        opt(deserializer)?.ok_or_else(|| de::Error::custom("missing timestamp"))
    }
}

/// Seconds (number or string) to [`Duration`].
pub(crate) mod seconds {
    use super::{Deserializer, Duration, de, finite_non_negative, number};

    pub(crate) fn opt<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        number(deserializer)?
            .map(|n| finite_non_negative(n).map(Duration::from_secs_f64))
            .transpose()
    }

    pub(crate) fn required<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Duration, D::Error> {
        opt(deserializer)?.ok_or_else(|| de::Error::custom("missing duration"))
    }
}

/// Booleans, also from `"true"`/`"false"`/`"1"`/`"0"`; `null` and absence
/// read as `false`.
pub(crate) mod flag {
    use super::{Deserialize, Deserializer, Scalar, Unexpected, de};

    pub(crate) fn lenient<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
        match Option::<Scalar>::deserialize(deserializer)? {
            None => Ok(false),
            Some(Scalar::Bool(b)) => Ok(b),
            Some(Scalar::Number(n)) => Ok(n != 0.0),
            Some(Scalar::Text(s)) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" | "" => Ok(false),
                other => Err(de::Error::invalid_value(
                    Unexpected::Str(other),
                    &"a boolean",
                )),
            },
        }
    }
}

/// Recipient lists arrive as an array, a comma-separated string, an empty
/// string, or `null`.
pub(crate) mod recipients {
    use super::{Deserialize, Deserializer};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Many(Vec<String>),
        One(String),
    }

    pub(crate) fn lenient<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<String>, D::Error> {
        Ok(match Option::<Raw>::deserialize(deserializer)? {
            None => Vec::new(),
            Some(Raw::Many(list)) => list.into_iter().filter(|s| !s.is_empty()).collect(),
            Some(Raw::One(s)) => s
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Probe {
        #[serde(default, deserialize_with = "timestamp::opt")]
        at: Option<SystemTime>,
        #[serde(default, deserialize_with = "seconds::opt")]
        ttl: Option<Duration>,
        #[serde(default, deserialize_with = "flag::lenient")]
        on: bool,
        #[serde(default, deserialize_with = "recipients::lenient")]
        to: Vec<String>,
    }

    fn probe(json: &str) -> Probe {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn numbers_and_numeric_strings_both_parse() {
        let p = probe(r#"{"at": 1792537823.5, "ttl": "600"}"#);
        assert_eq!(
            p.at.unwrap().duration_since(UNIX_EPOCH).unwrap(),
            Duration::from_secs_f64(1_792_537_823.5)
        );
        assert_eq!(p.ttl, Some(Duration::from_secs(600)));
        let p = probe(r#"{"at": "1792537823", "ttl": 600}"#);
        assert!(p.at.is_some());
        assert_eq!(p.ttl, Some(Duration::from_secs(600)));
    }

    #[test]
    fn null_empty_and_missing_are_none() {
        let p = probe(r#"{"at": null, "ttl": ""}"#);
        assert!(p.at.is_none());
        assert!(p.ttl.is_none());
        let p = probe("{}");
        assert!(p.at.is_none());
        assert!(!p.on);
        assert_eq!(p.to, Vec::<String>::new());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(serde_json::from_str::<Probe>(r#"{"ttl": "soon"}"#).is_err());
        assert!(serde_json::from_str::<Probe>(r#"{"ttl": -1}"#).is_err());
        assert!(serde_json::from_str::<Probe>(r#"{"on": "maybe"}"#).is_err());
    }

    #[test]
    fn flags_accept_the_stringly_forms() {
        assert!(probe(r#"{"on": true}"#).on);
        assert!(probe(r#"{"on": "true"}"#).on);
        assert!(probe(r#"{"on": "1"}"#).on);
        assert!(!probe(r#"{"on": "false"}"#).on);
        assert!(!probe(r#"{"on": null}"#).on);
        assert!(!probe(r#"{"on": 0}"#).on);
    }

    #[test]
    fn recipients_in_every_shape() {
        assert_eq!(probe(r#"{"to": ["a@x", "b@x"]}"#).to, vec!["a@x", "b@x"]);
        assert_eq!(probe(r#"{"to": "a@x, b@x"}"#).to, vec!["a@x", "b@x"]);
        assert_eq!(probe(r#"{"to": ""}"#).to, Vec::<String>::new());
        assert_eq!(probe(r#"{"to": null}"#).to, Vec::<String>::new());
        assert_eq!(probe(r#"{"to": [""]}"#).to, Vec::<String>::new());
    }
}
