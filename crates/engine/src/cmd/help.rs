//! Help → project links (this app's GitHub repository: source code, issues and releases). The
//! commands return the URL; the frontend opens it.

use serde_json::{Value, json};

use super::*;

/// This app's source repository.
pub const GITHUB_URL: &str = "https://github.com/SiteSentrix/StitchFoundry";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "help.github", "StitchFoundry on GitHub", ["Help"], None, "{} → {url} source code, issues and releases", always, |_, _| url(GITHUB_URL.into())),
        cmd!(query "help.links", "Links", [], None, "{} → {github}", always, |_, _| Ok(json!({ "github": GITHUB_URL }))),
    ]
}

fn url(u: String) -> Result<Value> {
    Ok(json!({ "url": u }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        let mut s = Session::new();
        assert_eq!(s.execute("help.github", &json!({})).unwrap()["url"], GITHUB_URL);
        assert_eq!(s.execute("help.links", &json!({})).unwrap()["github"], GITHUB_URL);
        // VectorCraft's community links (its Discord and website) are not this app's
        assert!(s.execute("help.discord", &json!({})).is_err());
        assert!(s.execute("help.website", &json!({})).is_err());
    }
}
