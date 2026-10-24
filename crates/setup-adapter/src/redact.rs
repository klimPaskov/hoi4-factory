//! Host paths never leave the server. Everything the adapter returns or stores for the browser
//! has them replaced by logical locators such as `workspace:<project>/source`.

use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct Redactor {
    pairs: Vec<(String, String)>,
}

impl Redactor {
    /// `pairs` are `(host path, logical locator)`. Each host path is also matched with the other
    /// separator style and in the JSON-escaped form.
    pub fn new(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut out: Vec<(String, String)> = Vec::new();
        for (host, logical) in pairs {
            if host.trim().is_empty() {
                continue;
            }
            let variants = [
                host.clone(),
                host.replace('\\', "/"),
                host.replace('/', "\\"),
                host.trim_start_matches(r"\\?\").to_owned(),
                host.trim_start_matches(r"\\?\").replace('\\', "/"),
            ];
            for v in variants {
                if !v.is_empty() && !out.iter().any(|(h, _)| *h == v) {
                    out.push((v, logical.clone()));
                }
            }
        }
        // Longest first so a parent never masks a child.
        out.sort_by_key(|(h, _)| std::cmp::Reverse(h.len()));
        Self { pairs: out }
    }

    /// The expanded `(host, logical)` pairs, to build a wider redactor.
    pub fn pairs(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.pairs.iter().cloned()
    }

    pub fn text(&self, input: &str) -> String {
        let mut out = input.to_owned();
        for (host, logical) in &self.pairs {
            if out.contains(host.as_str()) {
                out = out.replace(host.as_str(), logical);
            }
        }
        out
    }

    pub fn value(&self, input: &Value) -> Value {
        match input {
            Value::String(s) => Value::String(self.text(s)),
            Value::Array(items) => Value::Array(items.iter().map(|v| self.value(v)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), self.value(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Whether `input` still contains a host path. Used by tests and a debug assertion.
    pub fn leaks(&self, input: &str) -> bool {
        self.pairs.iter().any(|(h, _)| input.contains(h.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replaces_both_separator_styles_and_nested_values() {
        let r = Redactor::new([(
            r"C:\data\workspaces\a\p\source".to_owned(),
            "workspace:p/source".to_owned(),
        )]);
        assert_eq!(
            r.text(r"at C:\data\workspaces\a\p\source\x and C:/data/workspaces/a/p/source/y"),
            r"at workspace:p/source\x and workspace:p/source/y"
        );
        let v = json!({"a": [r"C:\data\workspaces\a\p\source"], "n": 1});
        assert_eq!(r.value(&v), json!({"a": ["workspace:p/source"], "n": 1}));
    }

    #[test]
    fn longer_paths_win() {
        let r = Redactor::new([
            ("/d/p".to_owned(), "P".to_owned()),
            ("/d/p/source".to_owned(), "S".to_owned()),
        ]);
        assert_eq!(r.text("/d/p/source/x /d/p/y"), "S/x P/y");
    }
}
