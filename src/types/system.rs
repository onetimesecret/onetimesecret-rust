//! Server information.

use serde::Deserialize;

/// `GET /status`: whether the server is up.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SystemStatus {
    /// The server's self-assessment, e.g. `"nominal"`.
    pub status: String,
    /// The locale the server chose for this request.
    #[serde(default)]
    pub locale: Option<String>,
}

impl SystemStatus {
    /// `true` when the server reports itself healthy.
    pub fn is_nominal(&self) -> bool {
        self.status == "nominal"
    }
}

/// `GET /version`: the server's build. Requires credentials.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SystemVersion {
    /// The version components as the server sends them, e.g. `["0", "23", "1"]`
    /// or `[0, 23, 1]`.
    #[serde(deserialize_with = "version_parts")]
    pub version: Vec<String>,
    /// The locale the server chose for this request.
    #[serde(default)]
    pub locale: Option<String>,
}

impl SystemVersion {
    /// The version joined with dots, e.g. `"0.23.1"`.
    pub fn to_dotted(&self) -> String {
        self.version.join(".")
    }
}

fn version_parts<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Part {
        Text(String),
        Number(serde_json::Number),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Parts(Vec<Part>),
        Whole(String),
    }
    Ok(match Raw::deserialize(deserializer)? {
        Raw::Parts(parts) => parts
            .into_iter()
            .map(|p| match p {
                Part::Text(s) => s,
                Part::Number(n) => n.to_string(),
            })
            .collect(),
        Raw::Whole(s) => s.split('.').map(str::to_owned).collect(),
    })
}

/// `GET /supported-locales`.
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct SupportedLocales {
    /// Locale codes the server can respond in.
    pub locales: Vec<String>,
    /// The server's default.
    pub default_locale: String,
    /// The locale the server chose for this request.
    #[serde(default)]
    pub locale: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_accepts_numbers_strings_and_dotted() {
        let v: SystemVersion =
            serde_json::from_str(r#"{"version":[0,23,1],"locale":"en"}"#).unwrap();
        assert_eq!(v.to_dotted(), "0.23.1");
        let v: SystemVersion = serde_json::from_str(r#"{"version":["0","23","1"]}"#).unwrap();
        assert_eq!(v.to_dotted(), "0.23.1");
        let v: SystemVersion = serde_json::from_str(r#"{"version":"0.23.1"}"#).unwrap();
        assert_eq!(v.version, vec!["0", "23", "1"]);
    }
}
