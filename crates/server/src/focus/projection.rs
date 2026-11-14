//! The focus editor's projection (SPEC-009, contract "Editor projection").
//!
//! The editor shows exactly what `hoi4.focus_inspect` returned, the call the agent makes: the
//! tool's plan of one tree, the solver's resolved positions, the localised titles and the
//! diagnostics for this mod. The browser receives logical paths (relative to the mod), never
//! server paths, and the projection carries the tool version, the hashes of every mod file the
//! tool read and a hash of the projection itself.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::ops::not_editable_reason;

pub const INSPECT_TOOL: &str = "hoi4.focus_inspect";
pub const SUPPORTED_OPERATIONS: [&str; 5] = ["create", "link", "move", "set_cost", "set_text"];

/// A projection with what the host needs to guard a later rewrite.
#[derive(Debug, Clone)]
pub struct Built {
    pub tree_id: String,
    /// The tree's source file, relative to the mod.
    pub relative_path: String,
    pub projection: Value,
    pub projection_hash: String,
    /// The tool's plan of the tree, which focus operations change.
    pub plan: Value,
    /// Mod file (relative path) -> SHA-256, for every mod file the inspection read.
    pub source_hashes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    TreeNotFound,
    /// The tree is defined outside this mod (the game or a dependency).
    NotInMod(String),
    Malformed(&'static str),
}

fn mod_path(path: &str) -> Option<&str> {
    path.strip_prefix("mod:")
}

fn source_ref(loc: &Value) -> Option<Value> {
    let path = mod_path(loc["path"].as_str()?)?;
    Some(json!({
        "path": path,
        "line": loc["start"]["line"].as_u64().unwrap_or(0),
        "column": loc["start"]["column"].as_u64().unwrap_or(0),
    }))
}

fn layout_verdict(raw: Option<&str>) -> &'static str {
    match raw {
        Some("pass") | Some("good") | Some("ok") => "pass",
        Some("revise") => "revise",
        Some("blocked") => "blocked",
        _ => "unknown",
    }
}

fn severity(raw: Option<&str>) -> &'static str {
    match raw {
        Some("error") => "error",
        Some("warning") => "warning",
        _ => "info",
    }
}

/// Builds the projection of `tree_id` from the inspect result and its linked artifact.
pub fn build(
    tree_id: &str,
    inspect: &Value,
    artifact: &Value,
    tool_version: &str,
) -> Result<Built, BuildError> {
    let plan = artifact["plans"]
        .as_array()
        .ok_or(BuildError::Malformed("the artifact has no plans"))?
        .iter()
        .find(|p| p["id"] == tree_id)
        .cloned()
        .ok_or(BuildError::TreeNotFound)?;
    let source_path = plan["sourceLocation"]["path"].as_str().unwrap_or("");
    let relative_path = match mod_path(source_path) {
        Some(p) => p.to_owned(),
        None => return Err(BuildError::NotInMod(source_path.to_owned())),
    };

    let positions: BTreeMap<String, (i64, i64)> = artifact["layouts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l["treeId"] == tree_id || l["layout"]["treeId"] == tree_id)
        .flat_map(|l| l["layout"]["nodes"].as_array().cloned().unwrap_or_default())
        .filter_map(|n| {
            Some((
                n["id"].as_str()?.to_owned(),
                (n["x"].as_i64()?, n["y"].as_i64()?),
            ))
        })
        .collect();
    let entries = &artifact["presentation"]["entries"];
    let focus_ids: BTreeSet<String> = plan["focuses"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f["id"].as_str().map(str::to_owned))
        .collect();

    // Diagnostics for this mod: from the result, the artifact and its presentation, deduplicated.
    let mut diagnostics = Vec::new();
    let mut seen = BTreeSet::new();
    let mut icon_missing = BTreeSet::new();
    let mut title_missing = BTreeSet::new();
    let sources = [
        inspect["diagnostics"].as_array(),
        artifact["diagnostics"].as_array(),
        artifact["presentation"]["diagnostics"].as_array(),
    ];
    for d in sources.into_iter().flatten().flatten() {
        let focus_id = d["details"]["focusId"]
            .as_str()
            .or_else(|| d["location"]["symbol"].as_str().filter(|s| focus_ids.contains(*s)))
            .map(str::to_owned);
        let source = source_ref(&d["location"]);
        let in_mod = source.is_some() || focus_id.as_ref().is_some_and(|f| focus_ids.contains(f));
        if !in_mod {
            continue;
        }
        let code = d["code"].as_str().unwrap_or("UNKNOWN").to_owned();
        let key = format!("{code}|{focus_id:?}|{source:?}");
        if !seen.insert(key) {
            continue;
        }
        if let Some(f) = &focus_id {
            if code.contains("ICON") {
                icon_missing.insert(f.clone());
            }
            if code.contains("LOCALISATION") && d["details"]["key"].as_str() == Some(f.as_str()) {
                title_missing.insert(f.clone());
            }
        }
        let mut item = Map::new();
        item.insert("code".into(), json!(code));
        item.insert("severity".into(), json!(severity(d["severity"].as_str())));
        item.insert("message".into(), d["message"].clone());
        if let Some(f) = focus_id {
            item.insert("focusId".into(), json!(f));
        }
        if let Some(s) = source {
            item.insert("source".into(), s);
        }
        diagnostics.push(Value::Object(item));
    }

    let mut nodes = Vec::new();
    for f in plan["focuses"].as_array().into_iter().flatten() {
        let id = f["id"].as_str().unwrap_or_default().to_owned();
        let entry = &entries[&id];
        let title = entry["title"].as_str().filter(|t| !t.is_empty() && !title_missing.contains(&id));
        let (x, y) = positions.get(&id).copied().unwrap_or((
            f["position"]["x"].as_i64().unwrap_or(0),
            f["position"]["y"].as_i64().unwrap_or(0),
        ));
        let declared = match f["position"]["mode"].as_str() {
            Some("fixed") => json!({"mode": "absolute", "x": f["position"]["x"], "y": f["position"]["y"]}),
            Some("relative") => json!({"mode": "relative", "x": f["position"]["x"], "y": f["position"]["y"], "relativeTo": f["position"]["relativeTo"]}),
            _ => json!({"mode": "auto"}),
        };
        let groups: Vec<Value> = f["prerequisites"]["groups"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|g| g["focusIds"].clone())
            .collect();
        let icon = f["icons"].as_array().and_then(|i| i.first()).and_then(|i| {
            Some(json!({"sprite": i["sprite"].as_str()?, "resolved": !icon_missing.contains(&id)}))
        });
        let mut node = Map::new();
        node.insert("id".into(), json!(id));
        node.insert("label".into(), json!(title.unwrap_or(&id)));
        if let Some(description) = entry["description"].as_str() {
            node.insert("description".into(), json!(description));
        }
        node.insert(
            "titleKey".into(),
            json!(f["localisation"]["titleKey"].as_str().unwrap_or(&id)),
        );
        node.insert(
            "descriptionKey".into(),
            json!(f["localisation"]["descriptionKey"].as_str().map(str::to_owned).unwrap_or(format!("{id}_desc"))),
        );
        node.insert("cost".into(), if f["cost"].is_number() { f["cost"].clone() } else { Value::Null });
        node.insert("icon".into(), icon.unwrap_or(Value::Null));
        node.insert("x".into(), json!(x));
        node.insert("y".into(), json!(y));
        node.insert("declared".into(), declared);
        node.insert("prerequisiteGroups".into(), json!(groups));
        node.insert("mutuallyExclusive".into(), f["mutuallyExclusive"].clone());
        node.insert(
            "source".into(),
            source_ref(&f["sourceLocation"]).unwrap_or(json!({"path": relative_path, "line": 0, "column": 0})),
        );
        match not_editable_reason(f) {
            None => {
                node.insert("editable".into(), json!(true));
            }
            Some(why) => {
                node.insert("editable".into(), json!(false));
                node.insert("notEditableReason".into(), json!(why));
            }
        }
        nodes.push(Value::Object(node));
    }

    let source_hashes: BTreeMap<String, String> = artifact["sourceHashes"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((mod_path(k)?.to_owned(), v.as_str()?.to_owned())))
        .collect();
    let tree_summary = inspect["data"]["trees"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["id"] == tree_id);
    let verdict = layout_verdict(
        tree_summary
            .and_then(|t| t["layoutVerdict"].as_str())
            .or_else(|| inspect["data"]["layoutVerdict"].as_str()),
    );
    let mut projection = json!({
        "tool": {"name": INSPECT_TOOL, "version": tool_version},
        "treeId": tree_id,
        "sourcePath": relative_path,
        "sourceHashes": source_hashes,
        "layoutVerdict": verdict,
        "nodes": nodes,
        "diagnostics": diagnostics,
        "supportedOperations": SUPPORTED_OPERATIONS,
    });
    let projection_hash = factory_hoi4_tools::mcp::canonical_sha256(&projection);
    projection["projectionHash"] = json!(projection_hash);
    Ok(Built {
        tree_id: tree_id.to_owned(),
        relative_path,
        projection,
        projection_hash,
        plan,
        source_hashes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> Value {
        json!({
            "plans": [{
                "id": "t",
                "sourceLocation": {"path": "mod:common/national_focus/t.txt"},
                "focuses": [
                    {"id": "a", "cost": 10, "position": {"mode": "fixed", "x": 4, "y": 0},
                     "prerequisites": {"groups": []}, "mutuallyExclusive": [],
                     "icons": [{"sprite": "GFX_a"}], "localisation": {"titleKey": "a", "descriptionKey": "a_desc"},
                     "sourceLocation": {"path": "mod:common/national_focus/t.txt", "start": {"line": 3, "column": 2}}},
                    {"id": "r", "cost": 5, "position": {"mode": "relative", "relativeTo": "a", "x": 1, "y": 1},
                     "prerequisites": {"groups": [{"focusIds": ["a"]}]}, "mutuallyExclusive": [],
                     "icons": [], "localisation": {"titleKey": "r", "descriptionKey": "r_desc"},
                     "sourceLocation": {"path": "mod:common/national_focus/t.txt", "start": {"line": 9, "column": 2}}}
                ]
            }],
            "layouts": [{"treeId": "t", "layout": {"nodes": [{"id": "a", "x": 4, "y": 0}, {"id": "r", "x": 5, "y": 1}]}}],
            "presentation": {
                "entries": {"a": {"title": "Alpha"}},
                "diagnostics": [{"code": "FOCUS_ICON_TEXTURE_MISSING", "severity": "error", "message": "no texture",
                                 "details": {"focusId": "a"}, "location": {"path": "mod:common/national_focus/t.txt", "start": {"line": 4, "column": 3}}}]
            },
            "diagnostics": [{"code": "SOURCE_UNEXPECTED_RIGHT_BRACE", "severity": "error", "message": "game file",
                             "location": {"path": "game:common/national_focus/x.txt", "start": {"line": 1, "column": 1}}}],
            "sourceHashes": {"mod:common/national_focus/t.txt": "aa", "game:common/x.txt": "bb"}
        })
    }

    #[test]
    fn projection_keeps_mod_facts_only_and_marks_unsupported_nodes() {
        let inspect = json!({"data": {"layoutVerdict": "pass", "trees": [{"id": "t", "layoutVerdict": "pass"}]}, "diagnostics": []});
        let built = build("t", &inspect, &artifact(), "4.11.0").unwrap();
        let p = &built.projection;
        assert_eq!(p["sourcePath"], "common/national_focus/t.txt");
        assert_eq!(built.source_hashes.len(), 1);
        assert_eq!(p["nodes"][0]["label"], "Alpha");
        assert_eq!(p["nodes"][0]["icon"], json!({"sprite": "GFX_a", "resolved": false}));
        assert_eq!(p["nodes"][0]["editable"], true);
        assert_eq!(p["nodes"][1]["label"], "r");
        assert_eq!(p["nodes"][1]["editable"], false);
        assert_eq!(p["nodes"][1]["declared"]["relativeTo"], "a");
        assert_eq!(p["nodes"][1]["prerequisiteGroups"], json!([["a"]]));
        assert_eq!(p["diagnostics"].as_array().unwrap().len(), 1, "the game diagnostic is left out");
        assert_eq!(p["layoutVerdict"], "pass");
        assert_eq!(p["projectionHash"], built.projection_hash);
        // The hash covers the content: the same inputs give the same hash.
        let again = build("t", &inspect, &artifact(), "4.11.0").unwrap();
        assert_eq!(again.projection_hash, built.projection_hash);
        assert!(!p.to_string().contains("game:"));
    }

    #[test]
    fn missing_or_foreign_trees_are_refused() {
        let inspect = json!({});
        assert_eq!(build("zz", &inspect, &artifact(), "v").unwrap_err(), BuildError::TreeNotFound);
        let mut a = artifact();
        a["plans"][0]["sourceLocation"]["path"] = json!("game:common/national_focus/t.txt");
        assert!(matches!(build("t", &inspect, &a, "v"), Err(BuildError::NotInMod(_))));
    }
}
