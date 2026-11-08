//! Focus operations: the one vocabulary for a person's gesture and the agent's edit (FR-5).
//!
//! A drag, a cost edit, a new prerequisite link and a new focus each become a [`FocusOp`]. The
//! same operations come from the browser editor and from the agent's `focus_edit` tool, and
//! [`apply_ops`] turns them into one change of the tool's own focus-tree plan, which the host then
//! sends to `hoi4.focus_rewrite`. No operation exists only for the editor.
//!
//! Only EXP-02's source-preserving subset is writable: moves and new links on focuses with a
//! literal position, scalar costs, and new focuses with literal positions. A relative or shared
//! focus is refused with a reason instead of being converted.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_OPS: usize = 20;
const MAX_ID: usize = 128;
const MAX_TEXT: usize = 400;
const MAX_REWARD: usize = 2000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum FocusOp {
    /// Moves a focus with a literal position to whole grid coordinates.
    Move { focus_id: String, x: i64, y: i64 },
    /// Sets the literal cost (in units of 7 days).
    SetCost { focus_id: String, cost: f64 },
    /// Adds one prerequisite group: any one of `prerequisites` must be complete.
    Link {
        focus_id: String,
        prerequisites: Vec<String>,
    },
    /// Adds a focus with a literal position, optional prerequisites (one group), an icon sprite,
    /// localisation text and a completion reward (an effect block body).
    Create {
        focus_id: String,
        x: i64,
        y: i64,
        cost: f64,
        #[serde(default)]
        prerequisites: Vec<String>,
        #[serde(default)]
        icon: Option<String>,
        title: String,
        description: String,
        #[serde(default)]
        reward: Option<String>,
    },
}

impl FocusOp {
    pub fn focus_id(&self) -> &str {
        match self {
            FocusOp::Move { focus_id, .. }
            | FocusOp::SetCost { focus_id, .. }
            | FocusOp::Link { focus_id, .. }
            | FocusOp::Create { focus_id, .. } => focus_id,
        }
    }
}

/// The closed JSON schema of one operation list, shared by the editor route and the agent tool.
pub fn ops_schema() -> Value {
    let id = json!({"type": "string", "pattern": "^[A-Za-z0-9_.-]{1,128}$"});
    let coord = json!({"type": "integer", "minimum": -200, "maximum": 400});
    let cost = json!({"type": "number", "minimum": 0, "maximum": 1000});
    json!({
        "type": "object",
        "properties": {
            "tree_id": {"type": "string", "pattern": "^[A-Za-z0-9_.-]{1,256}$"},
            "operations": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_OPS,
                "items": {
                    "type": "object",
                    "properties": {
                        "op": {"type": "string", "enum": ["move", "set_cost", "link", "create"]},
                        "focus_id": id,
                        "x": coord,
                        "y": coord,
                        "cost": cost,
                        "prerequisites": {"type": "array", "items": id, "maxItems": 8},
                        "icon": {"type": "string", "pattern": "^GFX_[A-Za-z0-9_]{1,120}$"},
                        "title": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT},
                        "description": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT},
                        "reward": {"type": "string", "minLength": 1, "maxLength": MAX_REWARD}
                    },
                    "required": ["op", "focus_id"],
                    "additionalProperties": false
                }
            },
            "expected_projection_hash": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
        },
        "required": ["tree_id", "operations"],
        "additionalProperties": false
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpError {
    /// `focus_not_found`, `focus_exists`, `not_editable`, `invalid_value`, `cycle`.
    pub code: &'static str,
    pub focus_id: String,
    pub message: String,
}

fn err(code: &'static str, focus_id: &str, message: impl Into<String>) -> OpError {
    OpError {
        code,
        focus_id: focus_id.to_owned(),
        message: message.into(),
    }
}

/// What the operations changed: focus IDs, and the localisation lines to add for new focuses.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PlanChange {
    pub changed_ids: Vec<String>,
    pub created_ids: Vec<String>,
    /// `(key, text)` pairs for the tree's English localisation file.
    pub localisation: Vec<(String, String)>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

fn focuses(plan: &mut Value) -> Result<&mut Vec<Value>, OpError> {
    plan.get_mut("focuses")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| err("invalid_value", "", "the plan has no focus list"))
}

fn index_of(list: &[Value], id: &str) -> Option<usize> {
    list.iter().position(|f| f["id"] == id)
}

/// Why a focus cannot take a structured edit, or `None` when it can.
pub fn not_editable_reason(focus: &Value) -> Option<String> {
    if focus["sharedSupport"] == true {
        return Some("a shared focus is edited in its own source".into());
    }
    match focus["position"]["mode"].as_str() {
        Some("fixed") => None,
        Some("relative") => Some(
            "its position is relative to another focus, so it is edited in the source to keep the relationship"
                .into(),
        ),
        Some(other) => Some(format!("its position is {other}, not a literal x and y")),
        None => Some("its position is not a literal x and y".into()),
    }
}

/// Every focus reachable from `start` through prerequisites (its ancestors).
fn ancestors(list: &[Value], start: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![start.to_owned()];
    while let Some(id) = stack.pop() {
        let Some(i) = index_of(list, &id) else { continue };
        for group in list[i]["prerequisites"]["groups"].as_array().into_iter().flatten() {
            for p in group["focusIds"].as_array().into_iter().flatten() {
                if let Some(p) = p.as_str()
                    && seen.insert(p.to_owned())
                {
                    stack.push(p.to_owned());
                }
            }
        }
    }
    seen
}

fn check_reward(id: &str, reward: &str) -> Result<(), OpError> {
    let mut depth = 0i32;
    let mut quoted = false;
    for c in reward.chars() {
        match c {
            '"' => quoted = !quoted,
            '{' if !quoted => depth += 1,
            '}' if !quoted => {
                depth -= 1;
                if depth < 0 {
                    return Err(err("invalid_value", id, "the reward closes a block it never opened"));
                }
            }
            '#' if !quoted => {
                return Err(err("invalid_value", id, "the reward may not contain comments"));
            }
            _ => {}
        }
    }
    if depth != 0 || quoted {
        return Err(err("invalid_value", id, "the reward has an unclosed block or quote"));
    }
    Ok(())
}

fn check_cost(id: &str, cost: f64) -> Result<(), OpError> {
    if !cost.is_finite() || !(0.0..=1000.0).contains(&cost) {
        return Err(err("invalid_value", id, "the cost is between 0 and 1000"));
    }
    Ok(())
}

fn number(cost: f64) -> Value {
    if cost.fract() == 0.0 {
        json!(cost as i64)
    } else {
        json!(cost)
    }
}

/// Applies the operations to a copy of the plan, in order. All or nothing.
pub fn apply_ops(plan: &Value, ops: &[FocusOp]) -> Result<(Value, PlanChange), OpError> {
    if ops.is_empty() || ops.len() > MAX_OPS {
        return Err(err("invalid_value", "", format!("send 1 to {MAX_OPS} operations")));
    }
    let mut plan = plan.clone();
    let mut change = PlanChange::default();
    for op in ops {
        let id = op.focus_id().to_owned();
        if !valid_id(&id) {
            return Err(err("invalid_value", &id, "a focus ID uses letters, digits, _ . and -"));
        }
        let list = focuses(&mut plan)?;
        match op {
            FocusOp::Move { x, y, .. } => {
                let i = index_of(list, &id).ok_or_else(|| err("focus_not_found", &id, "no such focus in this tree"))?;
                if let Some(why) = not_editable_reason(&list[i]) {
                    return Err(err("not_editable", &id, why));
                }
                let pinned = list[i]["position"]["pinned"].as_bool().unwrap_or(false);
                list[i]["position"] = json!({"mode": "fixed", "pinned": pinned, "x": x, "y": y});
            }
            FocusOp::SetCost { cost, .. } => {
                check_cost(&id, *cost)?;
                let i = index_of(list, &id).ok_or_else(|| err("focus_not_found", &id, "no such focus in this tree"))?;
                if list[i]["sharedSupport"] == true {
                    return Err(err("not_editable", &id, "a shared focus is edited in its own source"));
                }
                if !list[i]["cost"].is_number() {
                    return Err(err("not_editable", &id, "its cost is not a literal number"));
                }
                list[i]["cost"] = number(*cost);
            }
            FocusOp::Link { prerequisites, .. } => {
                let i = index_of(list, &id).ok_or_else(|| err("focus_not_found", &id, "no such focus in this tree"))?;
                if let Some(why) = not_editable_reason(&list[i]) {
                    return Err(err("not_editable", &id, why));
                }
                link_group(list, i, &id, prerequisites)?;
            }
            FocusOp::Create {
                x,
                y,
                cost,
                prerequisites,
                icon,
                title,
                description,
                reward,
                ..
            } => {
                check_cost(&id, *cost)?;
                if index_of(list, &id).is_some() {
                    return Err(err("focus_exists", &id, "a focus with this ID already exists in this tree"));
                }
                if title.trim().is_empty() || description.trim().is_empty() {
                    return Err(err("invalid_value", &id, "a new focus needs a title and a description"));
                }
                if let Some(r) = reward {
                    check_reward(&id, r)?;
                }
                // The template keeps the tree's lane and branch fields; every value that names the
                // source is replaced or removed.
                let template = prerequisites
                    .first()
                    .and_then(|p| index_of(list, p))
                    .or(if list.is_empty() { None } else { Some(0) })
                    .map(|i| list[i].clone());
                let mut focus = template.unwrap_or_else(|| json!({}));
                let obj = focus.as_object_mut().expect("focus object");
                for k in ["sourceLocation", "label"] {
                    obj.remove(k);
                }
                obj.insert("id".into(), json!(id));
                obj.insert("label".into(), json!(title));
                obj.insert("cost".into(), number(*cost));
                obj.insert("position".into(), json!({"mode": "fixed", "pinned": false, "x": x, "y": y}));
                obj.insert("prerequisites".into(), json!({"operator": "and", "groups": []}));
                obj.insert("mutuallyExclusive".into(), json!([]));
                obj.insert("links".into(), json!([]));
                obj.insert("routeLocks".into(), json!([]));
                obj.insert("rawPassthrough".into(), json!([]));
                obj.insert("filters".into(), json!([]));
                obj.insert("convergence".into(), json!(false));
                obj.insert("sharedSupport".into(), json!(false));
                obj.insert("visibility".into(), json!("normal"));
                obj.insert(
                    "ai".into(),
                    json!({"majorRoute": false, "strategyIds": [], "raw": {"referencedFocusIds": [], "text": "{ factor = 1 }"}}),
                );
                obj.insert(
                    "completionReward".into(),
                    json!({"referencedFocusIds": [], "text": format!("{{ {} }}", reward.as_deref().unwrap_or("").trim())}),
                );
                obj.insert(
                    "localisation".into(),
                    json!({"titleKey": id, "descriptionKey": format!("{id}_desc")}),
                );
                let icons = match icon {
                    Some(sprite) => json!([{"kind": "static", "sprite": sprite}]),
                    None => json!([]),
                };
                obj.insert("icons".into(), icons);
                list.push(focus);
                let i = list.len() - 1;
                if !prerequisites.is_empty() {
                    link_group(list, i, &id, prerequisites)?;
                }
                change.created_ids.push(id.clone());
                change.localisation.push((id.clone(), title.trim().to_owned()));
                change
                    .localisation
                    .push((format!("{id}_desc"), description.trim().to_owned()));
            }
        }
        if !change.changed_ids.contains(&id) {
            change.changed_ids.push(id);
        }
    }
    Ok((plan, change))
}

fn link_group(list: &mut [Value], i: usize, id: &str, prerequisites: &[String]) -> Result<(), OpError> {
    if prerequisites.is_empty() || prerequisites.len() > 8 {
        return Err(err("invalid_value", id, "a prerequisite group names 1 to 8 focuses"));
    }
    for p in prerequisites {
        if p == id {
            return Err(err("cycle", id, "a focus cannot require itself"));
        }
        if index_of(list, p).is_none() {
            return Err(err("focus_not_found", p, "the prerequisite is not in this tree"));
        }
        if ancestors(list, p).contains(id) {
            return Err(err("cycle", id, format!("{p} already requires {id}")));
        }
    }
    let groups = list[i]["prerequisites"]["groups"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let wanted: BTreeSet<&str> = prerequisites.iter().map(String::as_str).collect();
    for g in &groups {
        let have: BTreeSet<&str> = g["focusIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if have == wanted {
            return Err(err("invalid_value", id, "this prerequisite group already exists"));
        }
    }
    let mut groups = groups;
    groups.push(json!({"operator": "or", "focusIds": prerequisites, "rawPassthrough": []}));
    list[i]["prerequisites"] = json!({"operator": "and", "groups": groups});
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> Value {
        json!({
            "id": "t",
            "focuses": [
                {"id": "a", "cost": 10, "position": {"mode": "fixed", "pinned": false, "x": 4, "y": 0},
                 "prerequisites": {"operator": "and", "groups": []}, "sourceLocation": {"path": "mod:x"}, "laneId": "l1"},
                {"id": "b", "cost": 5, "position": {"mode": "fixed", "pinned": false, "x": 4, "y": 1},
                 "prerequisites": {"operator": "and", "groups": [{"operator": "or", "focusIds": ["a"], "rawPassthrough": []}]}},
                {"id": "r", "cost": 5, "position": {"mode": "relative", "pinned": true, "relativeTo": "a", "x": 1, "y": 1},
                 "prerequisites": {"operator": "and", "groups": []}}
            ]
        })
    }

    #[test]
    fn move_cost_link_and_create_change_only_their_focus() {
        let ops = vec![
            FocusOp::Move { focus_id: "b".into(), x: 6, y: 2 },
            FocusOp::SetCost { focus_id: "a".into(), cost: 7.0 },
            FocusOp::Create {
                focus_id: "c".into(),
                x: 5,
                y: 3,
                cost: 3.0,
                prerequisites: vec!["b".into()],
                icon: Some("GFX_x".into()),
                title: "New".into(),
                description: "A new focus.".into(),
                reward: Some("add_political_power = 15".into()),
            },
        ];
        let (out, change) = apply_ops(&plan(), &ops).unwrap();
        let f = out["focuses"].as_array().unwrap();
        assert_eq!(f[1]["position"], json!({"mode": "fixed", "pinned": false, "x": 6, "y": 2}));
        assert_eq!(f[0]["cost"], 7);
        assert_eq!(f[3]["id"], "c");
        assert!(f[3].get("sourceLocation").is_none());
        assert_eq!(f[3]["completionReward"]["text"], "{ add_political_power = 15 }");
        assert_eq!(f[3]["prerequisites"]["groups"][0]["focusIds"], json!(["b"]));
        assert_eq!(change.changed_ids, vec!["b", "a", "c"]);
        assert_eq!(change.localisation[1], ("c_desc".into(), "A new focus.".into()));
        // The input plan is untouched.
        assert_eq!(plan()["focuses"][1]["position"]["x"], 4);
    }

    #[test]
    fn unsupported_and_invalid_operations_are_refused_whole() {
        let refuse = |ops: Vec<FocusOp>, code: &str| {
            let e = apply_ops(&plan(), &ops).unwrap_err();
            assert_eq!(e.code, code, "{e:?}");
        };
        refuse(vec![FocusOp::Move { focus_id: "r".into(), x: 0, y: 0 }], "not_editable");
        refuse(vec![FocusOp::Move { focus_id: "zz".into(), x: 0, y: 0 }], "focus_not_found");
        refuse(vec![FocusOp::Link { focus_id: "a".into(), prerequisites: vec!["b".into()] }], "cycle");
        refuse(vec![FocusOp::Link { focus_id: "b".into(), prerequisites: vec!["a".into()] }], "invalid_value");
        refuse(vec![FocusOp::SetCost { focus_id: "a".into(), cost: f64::NAN }], "invalid_value");
        let create = |reward: &str| FocusOp::Create {
            focus_id: "n".into(),
            x: 0,
            y: 0,
            cost: 1.0,
            prerequisites: vec![],
            icon: None,
            title: "t".into(),
            description: "d".into(),
            reward: Some(reward.into()),
        };
        refuse(vec![create("add_stability = 0.1 }")], "invalid_value");
        refuse(vec![create("if = { limit = { a = b }")], "invalid_value");
        refuse(vec![create("x = 1 # hidden")], "invalid_value");
        // A later failure leaves no partial change behind.
        refuse(
            vec![
                FocusOp::SetCost { focus_id: "a".into(), cost: 1.0 },
                FocusOp::Move { focus_id: "r".into(), x: 0, y: 0 },
            ],
            "not_editable",
        );
    }

    #[test]
    fn the_schema_is_closed_and_matches_the_operations() {
        let schema = ops_schema();
        assert!(factory_hoi4_tools::profile::is_closed_schema(&schema));
        let v = jsonschema::validator_for(&schema).unwrap();
        let good = json!({"tree_id": "t", "operations": [{"op": "move", "focus_id": "a", "x": 1, "y": 2}]});
        assert!(v.is_valid(&good));
        let parsed: Vec<FocusOp> = serde_json::from_value(good["operations"].clone()).unwrap();
        assert_eq!(parsed[0], FocusOp::Move { focus_id: "a".into(), x: 1, y: 2 });
        assert!(!v.is_valid(&json!({"tree_id": "t", "operations": [{"op": "delete", "focus_id": "a"}]})));
        assert!(!v.is_valid(&json!({"tree_id": "t", "operations": [{"op": "move", "focus_id": "a", "extra": 1}]})));
        assert!(!v.is_valid(&json!({"tree_id": "../x", "operations": []})));
    }
}
