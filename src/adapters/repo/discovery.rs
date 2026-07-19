//! Walks a Neovim plugin repo and classifies each `.lua` file by role.
//!
//! Classification follows Neovim's own `require()` semantics: `lua/foo.lua`
//! and `lua/foo/init.lua` are both module entrypoints for `require("foo")`,
//! and `lua/foo/health.lua` is the corresponding `:checkhealth` entrypoint.
//! Everything else under `lua/foo/` is general module code.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Serialize;

/// A `.lua` file found on disk, with its classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LuaFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Path relative to the discovery root — this is what
    /// classification is derived from and what tests and reports use.
    pub relative_path: PathBuf,
    pub role: LuaFileRole,
}

/// The role a Lua file plays in a Neovim plugin repo. Rules use this
/// to decide which files they apply to (e.g. `nvim/augroup-clear`
/// scans [`LuaFileRole::Plugin`] and [`LuaFileRole::After`] but not
/// [`LuaFileRole::Test`]).
///
/// Serde wire format uses `type` as the tag and lowercase kebab-case
/// variants — snapshot-tested so downstream tooling can rely on the
/// exact JSON shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LuaFileRole {
    /// `plugin/**/*.lua` — files sourced at Neovim startup.
    Plugin,
    /// `lua/<name>.lua` or `lua/<name>/init.lua` — module entrypoint
    /// for `require("<name>")`.
    LuaInit { module: String },
    /// `lua/<name>/health.lua` — `:checkhealth <name>` entrypoint.
    LuaHealth { module: String },
    /// Any other file under `lua/<name>/...` — general module code.
    Lua { module: String },
    /// `after/**/*.lua` — patch layer sourced after user config.
    After,
    /// `tests/**`, `test/**`, `spec/**` — test suite.
    Test,
    /// Anything else (top-level `.lua`, unusual layouts, etc.).
    Other,
}

/// Errors surfaced by [`discover`]. Wrapped so callers do not depend
/// on `ignore` types directly.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("walk error at `{path}`: {source}")]
    Walk {
        path: PathBuf,
        #[source]
        source: ignore::Error,
    },
}

/// Walk `root`, return every `.lua` file with its classification.
///
/// - Respects `.gitignore` via the `ignore` crate.
/// - Skips hidden directories (`.git`, `.direnv`, etc.).
/// - Deterministic order — sorted by relative path.
pub fn discover(root: &Path) -> Result<Vec<LuaFile>, DiscoveryError> {
    let mut files = Vec::new();

    for entry in WalkBuilder::new(root).hidden(true).build() {
        let entry = entry.map_err(|source| DiscoveryError::Walk {
            path: root.to_path_buf(),
            source,
        })?;

        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("lua") {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("walker paths are always under the walk root")
            .to_path_buf();

        let role = classify(&relative);
        files.push(LuaFile {
            path: path.to_path_buf(),
            relative_path: relative,
            role,
        });
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

/// Classify a repo-relative path into a [`LuaFileRole`].
///
/// This is a pure function so classification can be unit-tested
/// without touching the filesystem.
pub fn classify(relative: &Path) -> LuaFileRole {
    let comps: Vec<&str> = relative.iter().filter_map(|c| c.to_str()).collect();

    let Some(top) = comps.first() else {
        return LuaFileRole::Other;
    };

    match *top {
        "plugin" => return LuaFileRole::Plugin,
        "after" => return LuaFileRole::After,
        "tests" | "test" | "spec" => return LuaFileRole::Test,
        "lua" => {} // fall through
        _ => return LuaFileRole::Other,
    }

    // Everything below is under `lua/`.
    let tail = &comps[1..];
    match tail {
        // lua/foo.lua — top-level module entrypoint. Only `.lua` files
        // reach this function (discover filters by extension), so any
        // single component is a module entrypoint.
        [file] => LuaFileRole::LuaInit {
            module: strip_lua_extension(file),
        },
        [module, "init.lua"] => LuaFileRole::LuaInit {
            module: (*module).to_string(),
        },
        [module, "health.lua"] => LuaFileRole::LuaHealth {
            module: (*module).to_string(),
        },
        [module, ..] => LuaFileRole::Lua {
            module: (*module).to_string(),
        },
        [] => LuaFileRole::Other,
    }
}

fn strip_lua_extension(name: &str) -> String {
    name.strip_suffix(".lua").unwrap_or(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(rel: &str) -> LuaFileRole {
        classify(Path::new(rel))
    }

    #[test]
    fn classifies_plugin_dir() {
        assert_eq!(role("plugin/foo.lua"), LuaFileRole::Plugin);
        assert_eq!(role("plugin/nested/foo.lua"), LuaFileRole::Plugin);
    }

    #[test]
    fn classifies_after_dir() {
        assert_eq!(role("after/plugin/foo.lua"), LuaFileRole::After);
        assert_eq!(role("after/ftplugin/lua.lua"), LuaFileRole::After);
    }

    #[test]
    fn classifies_test_dirs() {
        assert_eq!(role("tests/foo.lua"), LuaFileRole::Test);
        assert_eq!(role("test/foo.lua"), LuaFileRole::Test);
        assert_eq!(role("spec/foo_spec.lua"), LuaFileRole::Test);
    }

    #[test]
    fn classifies_lua_top_level_as_init() {
        assert_eq!(
            role("lua/foo.lua"),
            LuaFileRole::LuaInit {
                module: "foo".to_string()
            }
        );
    }

    #[test]
    fn classifies_lua_init() {
        assert_eq!(
            role("lua/foo/init.lua"),
            LuaFileRole::LuaInit {
                module: "foo".to_string()
            }
        );
    }

    #[test]
    fn classifies_lua_health() {
        assert_eq!(
            role("lua/foo/health.lua"),
            LuaFileRole::LuaHealth {
                module: "foo".to_string()
            }
        );
    }

    #[test]
    fn classifies_general_lua() {
        assert_eq!(
            role("lua/foo/util.lua"),
            LuaFileRole::Lua {
                module: "foo".to_string()
            }
        );
        assert_eq!(
            role("lua/foo/sub/deep.lua"),
            LuaFileRole::Lua {
                module: "foo".to_string()
            }
        );
    }

    #[test]
    fn classifies_unknown_layout_as_other() {
        assert_eq!(role("colors/dark.lua"), LuaFileRole::Other);
        assert_eq!(role("init.lua"), LuaFileRole::Other);
    }

    #[test]
    fn empty_path_is_other() {
        assert_eq!(classify(Path::new("")), LuaFileRole::Other);
    }

    #[test]
    fn discovers_sample_repo() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample-repo");
        let files = discover(&root).unwrap();

        let stable: Vec<(String, &LuaFileRole)> = files
            .iter()
            .map(|f| {
                (
                    f.relative_path.to_string_lossy().replace('\\', "/"),
                    &f.role,
                )
            })
            .collect();

        insta::assert_json_snapshot!("sample_repo_discovery", stable);
    }

    #[test]
    fn discovery_skips_non_lua_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("sample-repo");
        let files = discover(&root).unwrap();

        for f in &files {
            assert_eq!(
                f.path.extension().and_then(|s| s.to_str()),
                Some("lua"),
                "non-lua file leaked into discovery: {}",
                f.path.display()
            );
        }
        assert!(
            !files
                .iter()
                .any(|f| f.relative_path.to_string_lossy().contains("README")),
            "README.md must not appear in discovery output"
        );
    }
}
