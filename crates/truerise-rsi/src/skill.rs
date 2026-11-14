//! Skill schema + loader (spec §6 RSI skill row): a skill is a JSON package
//! (schema-versioned); the loader VALIDATES and the core knows nothing beyond
//! the schema - skills are plugins.

use serde::{Deserialize, Serialize};

/// The skill package schema (v1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPackage {
    /// Schema version (must be "truerise.skill.v1").
    pub schema_version: String,
    /// Skill name (kebab-case, non-empty).
    pub name: String,
    /// Human description.
    pub description: String,
    /// Entry point reference (opaque to the core; resolved by the host).
    pub entry: String,
    /// Declared triggers (free-form tags).
    #[serde(default)]
    pub triggers: Vec<String>,
}

/// Loader errors.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// Schema version mismatch.
    #[error("skill: unsupported schema version {0}")]
    UnsupportedSchema(String),
    /// Malformed package.
    #[error("skill: malformed: {0}")]
    Malformed(String),
}

/// Validate + load a skill package from raw JSON.
///
/// # Errors
/// [`SkillError::UnsupportedSchema`] / [`SkillError::Malformed`].
pub fn load_skill(raw: &str) -> Result<SkillPackage, SkillError> {
    let pkg: SkillPackage =
        serde_json::from_str(raw).map_err(|e| SkillError::Malformed(e.to_string()))?;
    if pkg.schema_version != "truerise.skill.v1" {
        return Err(SkillError::UnsupportedSchema(pkg.schema_version));
    }
    if pkg.name.trim().is_empty() {
        return Err(SkillError::Malformed("empty name".into()));
    }
    Ok(pkg)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const VALID: &str = r#"{
        "schema_version": "truerise.skill.v1",
        "name": "window-check",
        "description": "check a fold window for leakage",
        "entry": "skills/window_check.py",
        "triggers": ["window", "leak"]
    }"#;

    #[test]
    fn valid_package_loads() {
        let pkg = load_skill(VALID).unwrap();
        assert_eq!(pkg.name, "window-check");
        assert_eq!(pkg.triggers.len(), 2);
    }

    #[test]
    fn wrong_schema_version_refused() {
        let bad = VALID.replace("truerise.skill.v1", "something.else.v9");
        assert!(matches!(
            load_skill(&bad),
            Err(SkillError::UnsupportedSchema(_))
        ));
    }

    #[test]
    fn extra_fields_refused() {
        let bad = format!("{}{}", VALID.trim_end_matches('}'), ", \"sneaky\": 1}");
        assert!(matches!(load_skill(&bad), Err(SkillError::Malformed(_))));
    }
}
