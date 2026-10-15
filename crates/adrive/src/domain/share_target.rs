//! Management share targets, independent of file/space and public-page URL parsing.

use super::{
    client::{Error, Result},
    share_types::validate_required,
    share_url::decode_component,
};

/// Validated instance/share identity for an authenticated management operation.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ShareTarget {
    pub instance: String,
    pub share_id: String,
}

/// Resolve an instance-only URI or the legacy instance flag for share listing.
/// URI components decode once; a root slash and matching duplicate flags are accepted.
/// Returns sanitized errors for missing identity, conflicting flags or malformed syntax.
pub(crate) fn resolve_share_instance(
    target: Option<&str>,
    instance: Option<&str>,
) -> Result<String> {
    let Some(target) = target else {
        let instance = instance.ok_or_else(|| {
            Error::Client("share list requires adrive://instance or --instance".into())
        })?;
        validate_required(instance, "instance")?;
        return Ok(instance.into());
    };
    // [Review Fix #ShareList1] Instance selection has its own scope; a share/file path must not silently broaden a listing.
    let (scheme, remainder) = target
        .split_once("://")
        .ok_or_else(invalid_instance_target)?;
    if !scheme.eq_ignore_ascii_case("adrive")
        || target.trim() != target
        || target.contains(['?', '#', '\\'])
    {
        return Err(invalid_instance_target());
    }
    let encoded_instance = remainder.strip_suffix('/').unwrap_or(remainder);
    if encoded_instance.contains('/') {
        return Err(invalid_instance_target());
    }
    let resolved =
        decode_component(encoded_instance, false).map_err(|_| invalid_instance_target())?;
    validate_required(&resolved, "instance")?;
    if resolved.contains(['/', ':', '@', '?', '#', '\\']) || matches!(resolved.as_str(), "." | "..")
    {
        return Err(invalid_instance_target());
    }
    if instance.is_some_and(|instance| instance != resolved) {
        return Err(Error::Client(
            "--instance conflicts with the instance in the list URI".into(),
        ));
    }
    Ok(resolved)
}

fn invalid_instance_target() -> Error {
    Error::Client(
        "share list target must be exactly adrive://instance or adrive://instance/".into(),
    )
}

/// Resolve a bare ShareID with `instance`, or exactly `adrive://instance/share-id`.
/// URI components decode once; matching duplicate instances are accepted.
/// Returns sanitized input errors for missing/conflicting identity or malformed URI syntax.
pub(crate) fn resolve_share_target(target: &str, instance: Option<&str>) -> Result<ShareTarget> {
    validate_required(target, "share target")?;
    let Some((scheme, remainder)) = target.split_once("://") else {
        if target.to_ascii_lowercase().starts_with("adrive:") {
            return Err(invalid_target());
        }
        let instance =
            instance.ok_or_else(|| Error::Client("bare ShareID requires --instance".into()))?;
        validate_required(instance, "instance")?;
        return Ok(ShareTarget {
            instance: instance.into(),
            share_id: target.into(),
        });
    };
    if !scheme.eq_ignore_ascii_case("adrive")
        || target.trim() != target
        || target.contains(['?', '#', '\\'])
    {
        return Err(invalid_target());
    }
    let (encoded_instance, encoded_id) = remainder.split_once('/').ok_or_else(invalid_target)?;
    if encoded_id.contains('/') {
        return Err(invalid_target());
    }
    let resolved = ShareTarget {
        instance: decode_component(encoded_instance, false).map_err(|_| invalid_target())?,
        share_id: decode_component(encoded_id, false).map_err(|_| invalid_target())?,
    };
    validate_required(&resolved.instance, "instance")?;
    validate_required(&resolved.share_id, "share id")?;
    if resolved.instance.contains(['/', ':', '@', '?', '#', '\\'])
        || matches!(resolved.instance.as_str(), "." | "..")
        || matches!(resolved.share_id.as_str(), "." | "..")
    {
        return Err(invalid_target());
    }
    if instance.is_some_and(|instance| instance != resolved.instance) {
        return Err(Error::Client(
            "--instance conflicts with the instance in the share URI".into(),
        ));
    }
    Ok(resolved)
}

fn invalid_target() -> Error {
    Error::Client(
        "share target must be a ShareID with --instance or exactly adrive://instance/share-id"
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_instance_selectors_decode_once_and_keep_legacy_names() {
        for target in ["adrive://My%20Instance", "ADrive://My%20Instance/"] {
            assert_eq!(
                resolve_share_instance(Some(target), Some("My Instance")).unwrap(),
                "My Instance"
            );
        }
        assert_eq!(
            resolve_share_instance(Some("adrive://a%252Fb"), None).unwrap(),
            "a%2Fb"
        );
        assert_eq!(
            resolve_share_instance(None, Some("instance_with_underscore")).unwrap(),
            "instance_with_underscore"
        );
        assert!(resolve_share_instance(None, None).is_err());
        assert!(resolve_share_instance(None, Some(" ")).is_err());
        assert!(resolve_share_instance(Some("adrive://inst"), Some("other")).is_err());
    }

    #[test]
    fn malformed_list_instance_uris_reject_paths_and_redact_errors() {
        for target in [
            "inst",
            "adrive://",
            "adrive:///",
            "adrive://inst//",
            "adrive://inst/s",
            "adrive://inst/space/file",
            "adrive://inst?pwd=secret",
            "adrive://inst/#secret",
            "adrive://user:secret@inst",
            "adrive://inst:8080",
            "adrive://inst%2Fs",
            "adrive://%2E%2E",
            "adrive://inst%00",
            "adrive://%ZZ",
            "adrive:/inst",
            "https://inst",
            " adrive://inst",
            "adrive://inst\\s",
        ] {
            let error = resolve_share_instance(Some(target), None).unwrap_err();
            assert!(!error.to_string().contains("secret"));
        }
    }

    #[test]
    fn uri_and_legacy_selectors_resolve_to_the_same_identity() {
        let expected = resolve_share_target("s", Some("inst")).unwrap();
        assert_eq!(
            resolve_share_target("adrive://inst/s", None).unwrap(),
            expected
        );
        assert_eq!(
            resolve_share_target("ADrive://inst/s", Some("inst")).unwrap(),
            expected
        );
        assert!(resolve_share_target("s", None).is_err());
        assert!(resolve_share_target("adrive://inst/s", Some("other")).is_err());
    }

    #[test]
    fn uri_components_decode_once_and_preserve_instance_case() {
        let selected = resolve_share_target("adrive://My%20Instance/a%252Fb", None).unwrap();
        assert_eq!(selected.instance, "My Instance");
        assert_eq!(selected.share_id, "a%2Fb");
        let selected = resolve_share_target("adrive://inst/a%2Fb", None).unwrap();
        assert_eq!(selected.share_id, "a/b");
        assert_eq!(
            resolve_share_target("s", Some("instance_with_underscore"))
                .unwrap()
                .instance,
            "instance_with_underscore"
        );
    }

    #[test]
    fn malformed_share_uris_are_rejected_without_echoing_input() {
        for target in [
            "adrive://inst",
            "adrive://inst/",
            "adrive:///s",
            "adrive://inst/space/file",
            "adrive://inst/s/",
            "adrive://inst/..",
            "adrive://inst/s?pwd=secret",
            "adrive://inst/s#secret",
            "adrive://user:secret@inst/s",
            "adrive://inst:8080/s",
            "adrive://inst%2Fs/s",
            "adrive://inst/%ZZ",
            "adrive://inst/%00",
            "adrive:/inst/s",
            "adrive://inst/s\\file",
            " adr ive://inst/s",
            "https://pages.test/share/s",
        ] {
            let error = resolve_share_target(target, Some("inst")).unwrap_err();
            assert!(!error.to_string().contains("secret"));
        }
    }
}
