//! One coverage owner can select independently scheduled tests without calling
//! those tests again from an umbrella. The manifest binds membership; normal
//! executable-owner validation checks every selector against today's Rust tree.
use super::*;

const SCHEMA: &str = "excalidraw-render/test-group/v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    schema: String,
    package: String,
    binary: String,
    tests: Vec<String>,
}

fn validate(group: &Group) -> Result<(), String> {
    if group.schema != SCHEMA
        || group.tests.is_empty()
        || group.binary == "lib"
        || group.binary == EXCEPTION_BINARY
    {
        return Err("invalid or empty executable test group".into());
    }
    let mut seen = BTreeSet::new();
    for selector in &group.tests {
        if !seen.insert(selector) {
            return Err(format!("duplicate test group member: {selector}"));
        }
        let (module, function) = selector.rsplit_once("::").unwrap_or(("", selector));
        test_owner(&group.package, &group.binary, module, function)
            .map_err(|error| format!("invalid test group member {selector}: {error}"))?;
    }
    Ok(())
}

fn load(group: &Descriptor) -> Result<Group, String> {
    if !group
        .path
        .starts_with("tests/conformance/coverage/test-groups/")
        || !group.path.ends_with(".json")
        || descriptor(&group.path)? != *group
    {
        return Err("stale or redirected test group descriptor".into());
    }
    let value = parse(&read(&group.path)?)?;
    validate(&value)?;
    Ok(value)
}

pub(super) fn verify(group: &Descriptor) -> Result<(), String> {
    load(group).map(|_| ())
}

/// Exact binary and test-name matches; never a shell command or fuzzy pattern.
pub fn filter(path: &str) -> Result<String, String> {
    let group = load(&descriptor(path)?)?;
    let members = group
        .tests
        .iter()
        .map(|name| format!("test(={name})"))
        .collect::<Vec<_>>()
        .join(" | ");
    Ok(format!(
        "package(={}) & binary(={}) & ({members})",
        group.package, group.binary
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const GROUP: &str = "tests/conformance/coverage/test-groups/unsupported-property-values.json";

    #[test]
    fn all_unsupported_value_members_are_independently_executable() {
        let group = load(&descriptor(GROUP).unwrap()).unwrap();
        assert_eq!(group.tests.len(), 33);
        assert!(group
            .tests
            .iter()
            .any(|s| s.ends_with("::invalid_property_link_values")));
        assert!(group
            .tests
            .iter()
            .any(|s| s
                .ends_with("::malformed_points_keep_pressure_length_based_on_source_occurrences")));
    }

    #[test]
    fn rejects_empty_duplicate_unknown_and_non_test_members() {
        for members in [
            vec![],
            vec!["compatibility_recovery_contract::property_disposition_inventory_is_closed"; 2],
            vec!["compatibility_recovery_contract::not_a_test"],
            vec!["geometry_style_contract::scene"],
        ] {
            let group = Group {
                schema: SCHEMA.into(),
                package: executable::RENDERER.into(),
                binary: "fast".into(),
                tests: members.into_iter().map(str::to_owned).collect(),
            };
            assert!(validate(&group).is_err());
        }
    }

    #[test]
    fn descriptor_binds_the_exact_membership() {
        let mut source = descriptor(GROUP).unwrap();
        source.sha256 = "0".repeat(64);
        assert!(load(&source).unwrap_err().contains("descriptor"));
    }

    #[test]
    fn groups_are_regular_integration_targets_not_special_owner_adapters() {
        for binary in ["lib", EXCEPTION_BINARY] {
            let group = Group {
                schema: SCHEMA.into(),
                package: executable::RENDERER.into(),
                binary: binary.into(),
                tests: vec!["some_test".into()],
            };
            assert!(validate(&group).is_err());
        }
    }
}
