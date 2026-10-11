//! Native profile paths. Isolated profiles never migrate preferences from a user's old profile.

use std::ffi::OsString;
use std::path::PathBuf;

/// The settings file `ui.json` of the app whose folder is `name` (macOS, Windows) or `lower`
/// (Linux and BSD). `legacy` marks the profile of another app that is only read once, to migrate
/// from it (StitchFoundry reads VectorCraft's).
pub fn prefs_path(name: &str, lower: &str, legacy: bool) -> Option<PathBuf> {
    prefs_path_with(std::env::consts::OS, name, lower, legacy, |key| std::env::var_os(key))
}

fn prefs_path_with(os: &str, name: &str, lower: &str, legacy: bool, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(root) = env("VECTORCRAFT_CONFIG_DIR") {
        // Never fall back to another app's real profile when the isolated file does not exist.
        return (!legacy && !root.is_empty()).then(|| PathBuf::from(root).join("ui.json"));
    }
    let base = match os {
        "macos" => env("HOME").map(|p| PathBuf::from(p).join("Library/Application Support").join(name)),
        "windows" => env("APPDATA").map(|p| PathBuf::from(p).join(name)),
        _ => env("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| env("HOME").map(|p| PathBuf::from(p).join(".config"))).map(|p| p.join(lower)),
    };
    base.map(|p| p.join("ui.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: (&str, &str) = ("StitchFoundry", "stitchfoundry");
    const OLD: (&str, &str) = ("VectorCraft", "vectorcraft");

    fn environment(key: &str) -> Option<OsString> {
        match key {
            "HOME" => Some("/user".into()),
            "APPDATA" => Some("/roaming".into()),
            "XDG_CONFIG_HOME" => Some("/xdg-config".into()),
            _ => None,
        }
    }

    #[test]
    fn isolated_profile_has_no_legacy_fallback_on_any_platform() {
        for os in ["macos", "windows", "linux"] {
            let env = |key: &str| {
                if key == "VECTORCRAFT_CONFIG_DIR" { Some("/isolated".into()) } else { environment(key) }
            };
            assert_eq!(prefs_path_with(os, APP.0, APP.1, false, env), Some("/isolated/ui.json".into()));
            assert_eq!(prefs_path_with(os, OLD.0, OLD.1, true, env), None);
        }
    }

    #[test]
    fn empty_override_does_not_fall_back_to_user_files() {
        let env = |key: &str| if key == "VECTORCRAFT_CONFIG_DIR" { Some(OsString::new()) } else { environment(key) };
        assert_eq!(prefs_path_with("macos", APP.0, APP.1, false, env), None);
        assert_eq!(prefs_path_with("macos", OLD.0, OLD.1, true, env), None);
    }

    #[test]
    fn unset_override_keeps_platform_paths_and_legacy_migration() {
        assert_eq!(
            prefs_path_with("macos", APP.0, APP.1, false, environment),
            Some("/user/Library/Application Support/StitchFoundry/ui.json".into())
        );
        assert_eq!(prefs_path_with("macos", OLD.0, OLD.1, true, environment), Some("/user/Library/Application Support/VectorCraft/ui.json".into()));
        assert_eq!(prefs_path_with("windows", APP.0, APP.1, false, environment), Some("/roaming/StitchFoundry/ui.json".into()));
        assert_eq!(prefs_path_with("linux", APP.0, APP.1, false, environment), Some("/xdg-config/stitchfoundry/ui.json".into()));
        assert_eq!(
            prefs_path_with("linux", OLD.0, OLD.1, true, |key| if key == "HOME" { environment(key) } else { None }),
            Some("/user/.config/vectorcraft/ui.json".into())
        );
    }
}
