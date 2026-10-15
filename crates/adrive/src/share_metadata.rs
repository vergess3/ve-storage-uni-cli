//! Typed public metadata for share commands.
use crate::registry::{find_capability, RegistryParameter};
use serde_json::{json, Value};

/// Describe a canonical share leaf without config, authentication or password I/O.
/// Returns None for paths outside the registered share command surface.
pub fn describe_share_command(command: &str) -> Option<Value> {
    if command == "ve-adrive share" {
        return Some(
            json!({"command":command,"kind":"command_group","description":"Manage and receive share links","supports_describe":true,"supports_help":true,"subcommands":crate::registry::capabilities().iter().filter(|row|row.group=="share").map(|row|json!({"name":row.command.rsplit(' ').next(),"description":row.description,"risk_level":row.risk_level})).collect::<Vec<_>>() }),
        );
    }
    if !command.starts_with("ve-adrive share ") {
        return None;
    }
    let row = find_capability(command)?;
    Some(
        json!({"command":command,"layer":row.layer,"description":row.description,
            "risk_level":row.risk_level,"supports_dry_run":true,"supports_pipe":true,
            // [Review Fix #Rebase5] Expose the cancellation gate consistently
            // with capabilities and doctor without changing critical deletes.
            "destructive":row.destructive,"supports_force":row.supports_force,
            "supports_yes":supports_yes(command),
            "parameters":row.parameters.iter().map(|item| parameter(command,item)).collect::<Vec<_>>(),
            "output_schema": output_schema(command),
            // [Review Fix #LiveDocs1] Describe/MCP guidance must expose the same connection and root rules as help.
            "semantics": share_semantics(command),
            "low_level_apis":row.api_actions,"wraps_apis":row.api_actions,
            "examples":row.examples,"side_effects":if matches!(command.rsplit(' ').next(),Some("create"|"update"|"cancel")) { vec!["Share settings are mutable; the shared target cannot change."] } else if command.ends_with(" view") {vec![crate::cli::share::SHARE_VIEW_HELP]} else if matches!(command.rsplit(' ').next(),Some("inspect"|"files"|"download")) { vec![crate::cli::share::SHARE_RECEIVER_HELP] } else {vec![]}
        }),
    )
}

fn share_semantics(command: &str) -> Vec<&'static str> {
    let mut semantics = vec![crate::cli::share::SHARE_CONNECTION_HELP];
    if matches!(
        command,
        "ve-adrive share get" | "ve-adrive share update" | "ve-adrive share cancel"
    ) {
        semantics.push(crate::cli::share::SHARE_SELECTOR_HELP);
    }
    if command == "ve-adrive share download" {
        semantics.push(crate::cli::share::SHARE_DOWNLOAD_HELP);
    }
    if command == "ve-adrive share view" {
        semantics.push(crate::cli::share::SHARE_VIEW_HELP);
    }
    if command == "ve-adrive share list" {
        semantics.push(crate::cli::share::SHARE_LIST_HELP);
    }
    if matches!(command, "ve-adrive share create" | "ve-adrive share update") {
        semantics.push(crate::cli::share::SHARE_SETTINGS_HELP);
    }
    // [Review Fix #ShareReceiveDocs2] Append new guidance without changing the established ordering of existing semantics.
    if matches!(
        command.rsplit(' ').next(),
        Some("inspect" | "files" | "download" | "view")
    ) {
        semantics.push(crate::cli::share::SHARE_RECEIVER_HELP);
    }
    if matches!(command.rsplit(' ').next(), Some("files" | "download")) {
        semantics.push(crate::cli::share::SHARE_FILES_HELP);
    }
    semantics
}

/// Return whether global `--yes` supplies this share command's confirmation.
/// The canonical `command` must be `ve-adrive share cancel`; other paths return
/// false. This metadata lookup performs no I/O and cannot fail.
pub fn supports_yes(command: &str) -> bool {
    // [Review Fix #Rebase5] Cancellation revokes a share and uses its approved
    // --yes gate; it does not bypass force/confirm guards for file deletion.
    command == "ve-adrive share cancel"
}

fn output_schema(command: &str) -> Value {
    if command == "ve-adrive share view" {
        return json!({"type":"object","required":["ResultType","FilePath","StatusCode"],"properties":{
            "ResultType":{"enum":["raw","document"]},"FilePath":{"type":"string"},
            "StatusCode":{"type":"integer"},"ShareUrl":{"type":"string","format":"uri"},
            "Headers":{"type":"object","description":"Safe content headers for raw metadata results"},
            "PreviewURL":{"type":"string","format":"uri","description":"Short-lived document capability URL"},
            "ExpiresAt":{"type":"string","format":"date-time"},"PreviewType":{"type":"string"}
        }});
    }
    // [Review Fix #ShareDelivery1] Delivery passwords belong only to authenticated create/get results, never receiver or mutation metadata.
    if matches!(command, "ve-adrive share create" | "ve-adrive share get") {
        return json!({"type":"object","properties":{
            "ShareID":{"type":"string"},
            "ShareUrl":{"type":"string","format":"uri"},
            "SharePwd":{"type":"string","description":"Share password returned for authorized delivery"}
        }});
    }
    if command == "ve-adrive share download" {
        return json!({"type":"object","required":["Complete","PlanComplete","Summary","Files"],"properties":{
            "Complete":{"type":"boolean"},"PlanComplete":{"type":"boolean"},
            "CheckpointAvailable":{"type":"boolean"},"IssuanceOutcomeUnknown":{"type":"boolean"},
            "Summary":{"type":"object","properties":{"Total":{"type":"integer"},"Completed":{"type":"integer"},"Failed":{"type":"integer"},"Pending":{"type":"integer"},"Skipped":{"type":"integer","description":"Validated completed files reused during resume; included in Completed"}}},
            "Files":{"type":"array","items":{"type":"object","properties":{"Path":{"type":"string"},"LocalPath":{"type":"string"},"Status":{"enum":["completed","failed","pending"]},"SHA256":{"type":["string","null"]}}}}
        }});
    }
    json!({"type":"object"})
}

fn parameter(command: &str, item: &RegistryParameter) -> Value {
    json!({"name":item.name,"required":item.required,"description":item.description,
        "location":if matches!(item.name,"target"|"share-id"|"share-url"|"destination-dir") { "positional" } else { "flag" },
        "schema":parameter_schema(command,item.name)})
}

/// Return the share-specific schema, preserving scalar types and explicit bool values.
pub fn parameter_schema(command: &str, name: &str) -> Value {
    match name {
        "creator" => json!({"type":"array","items":{"type":"string"}}),
        "preview-limit" | "download-limit" => json!({"type":"integer","minimum":0}),
        "offset" => json!({"type":"integer","minimum":0,"maximum":10000}),
        "created-at-start" | "created-at-end" => json!({"type":"string","format":"date-time"}),
        "updated-at" => json!({"type":"integer"}),
        "multipart-concurrency" => json!({"type":"integer","minimum":1,"default":4}),
        "checkpoint-threshold" => json!({"type":"string","default":"20MB"}),
        "limit" => json!({"type":"integer","minimum":1}),
        "require-login" | "password-stdin" | "by-name" | "recursive" | "overwrite" | "resume"
        | "progress" | "no-progress" | "raw" => {
            json!({"type":"boolean"})
        }
        "permission" if command.ends_with("create") => {
            json!({"type":"string","enum":["view","download"],"default":crate::cli::share::DEFAULT_CREATE_SHARE_PERMISSION})
        }
        "permission" => json!({"type":"string","enum":["view","download"]}),
        "status" => json!({"type":"string","enum":["active","expired","revoked"]}),
        "expires-in" if command.ends_with("create") => json!({"type":"string","default":"0"}),
        _ => json!({"type":"string"}),
    }
}

/// Whether this command supports a virtual MCP password passed only through stdin.
pub fn supports_password(command: &str) -> bool {
    matches!(
        command,
        "ve-adrive share create"
            | "ve-adrive share update"
            | "ve-adrive share files"
            | "ve-adrive share view"
            | "ve-adrive share download"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_describe_exposes_explicit_oauth_and_pagination_rules() {
        for action in ["inspect", "files", "download"] {
            let value = describe_share_command(&format!("ve-adrive share {action}")).unwrap();
            let semantics = value["semantics"].to_string();
            assert!(
                semantics.contains("default to anonymous")
                    && semantics.contains("--auth-mode oauth")
            );
            assert!(semantics.contains("selected profile API or --endpoint"));
            if action != "inspect" {
                assert!(
                    semantics.contains("--limit")
                        && semantics.contains("--marker")
                        && semantics.contains("EOF")
                );
            }
        }
    }

    #[test]
    fn create_permission_default_is_view_and_update_has_no_default() {
        assert_eq!(
            parameter_schema("ve-adrive share create", "permission")["default"],
            "view"
        );
        assert!(parameter_schema("ve-adrive share update", "permission")
            .get("default")
            .is_none());
    }

    #[test]
    fn management_instance_flag_is_optional_for_share_uri_selectors() {
        for action in ["get", "update", "cancel"] {
            let description = describe_share_command(&format!("ve-adrive share {action}")).unwrap();
            let instance = description["parameters"]
                .as_array()
                .unwrap()
                .iter()
                .find(|parameter| parameter["name"] == "instance")
                .unwrap();
            assert_eq!(instance["required"], false);
            assert!(instance["description"]
                .as_str()
                .unwrap()
                .contains("bare ShareID"));
            assert!(description["semantics"]
                .to_string()
                .contains("adrive://instance/share-id"));
        }
    }

    #[test]
    fn share_list_describes_optional_uri_and_instance_selectors() {
        let description = describe_share_command("ve-adrive share list").unwrap();
        let parameters = description["parameters"].as_array().unwrap();
        let target = parameters
            .iter()
            .find(|parameter| parameter["name"] == "target")
            .unwrap();
        assert_eq!(target["location"], "positional");
        assert_eq!(target["required"], false);
        let instance = parameters
            .iter()
            .find(|parameter| parameter["name"] == "instance")
            .unwrap();
        assert_eq!(instance["required"], false);
        assert!(description["semantics"]
            .to_string()
            .contains("list accepts adrive://instance"));
    }

    #[test]
    fn delivery_password_output_schema_is_limited_to_create_and_get() {
        for action in [
            "create", "get", "list", "update", "cancel", "inspect", "files", "view", "download",
        ] {
            let command = format!("ve-adrive share {action}");
            let description = describe_share_command(&command).unwrap();
            let property = &description["output_schema"]["properties"]["SharePwd"];
            if matches!(action, "create" | "get") {
                assert_eq!(property["type"], "string");
                assert_eq!(
                    property["description"],
                    "Share password returned for authorized delivery"
                );
                assert!(property.get("writeOnly").is_none());
            } else {
                assert!(property.is_null());
            }
        }
    }
}
