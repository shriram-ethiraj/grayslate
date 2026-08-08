use serde::Serialize;

/// Controls whether this particular binary may replace itself.
///
/// Package-manager builds set `GRAYSLATE_UPDATE_POLICY=system-managed` at
/// compile time. Direct-download builds set it to `self-update`. An absent or
/// invalid value fails closed so development and ad-hoc builds never contact
/// the production updater accidentally.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdatePolicy {
    Disabled,
    SelfUpdate,
    SystemManaged,
}

pub fn current_update_policy() -> UpdatePolicy {
    // The packaged E2E binary uses a deterministic updater response so the
    // self-update-only settings and UI can be exercised without network access.
    #[cfg(feature = "e2e")]
    return UpdatePolicy::SelfUpdate;

    #[cfg(not(feature = "e2e"))]
    parse_update_policy(option_env!("GRAYSLATE_UPDATE_POLICY"))
}

#[cfg(any(not(feature = "e2e"), test))]
fn parse_update_policy(value: Option<&str>) -> UpdatePolicy {
    match value {
        Some("self-update") => UpdatePolicy::SelfUpdate,
        Some("system-managed") => UpdatePolicy::SystemManaged,
        _ => UpdatePolicy::Disabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_missing_policies_fail_closed() {
        assert_eq!(parse_update_policy(None), UpdatePolicy::Disabled);
        assert_eq!(
            parse_update_policy(Some("self_update")),
            UpdatePolicy::Disabled
        );
    }

    #[test]
    fn supported_policies_are_parsed_exactly() {
        assert_eq!(
            parse_update_policy(Some("self-update")),
            UpdatePolicy::SelfUpdate
        );
        assert_eq!(
            parse_update_policy(Some("system-managed")),
            UpdatePolicy::SystemManaged
        );
    }
}
