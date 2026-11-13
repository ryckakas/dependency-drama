//! Which registry a package name belongs to, according to the configuration pnpm itself reads.
//!
//! `pnpm-lock.yaml` does not say which registry a package came from, and a name from a private
//! registry must never be sent to the public one. So a name counts as public only when nothing
//! that was read points its scope, or the default registry, anywhere else.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_REGISTRY: &str = "registry.npmjs.org";
const ENVIRONMENT_PREFIX: &str = "npm_config_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registries {
    default_is_public: bool,
    /// By scope, `@` included. A scope that is absent uses the default registry.
    scope_is_public: BTreeMap<String, bool>,
}

impl Default for Registries {
    fn default() -> Self {
        Self {
            default_is_public: true,
            scope_is_public: BTreeMap::new(),
        }
    }
}

impl Registries {
    /// Reads `.npmrc` and `pnpm-workspace.yaml` beside the lockfile, the user's `.npmrc`, the file
    /// `NPM_CONFIG_USERCONFIG` names (where `actions/setup-node` writes a registry), and the
    /// `npm_config_*` environment variables.
    pub(crate) fn discover(project: &Path) -> Self {
        let files = [
            Some(project.join(".npmrc")),
            Some(project.join("pnpm-workspace.yaml")),
            home().map(|home| home.join(".npmrc")),
            env::var_os("NPM_CONFIG_USERCONFIG").map(PathBuf::from),
        ];
        let from_files = files
            .into_iter()
            .flatten()
            .filter_map(|file| fs::read_to_string(file).ok())
            .flat_map(|text| settings_in(&text));
        // `vars_os`, because `vars` panics on a variable that is not Unicode.
        let from_environment = env::vars_os().filter_map(|(name, value)| {
            let name = name.to_str()?.to_lowercase();
            let setting = name.strip_prefix(ENVIRONMENT_PREFIX)?.to_owned();
            Some((setting, value.to_str()?.to_lowercase()))
        });
        Self::from_settings(from_files.chain(from_environment))
    }

    /// Takes `registry` and `@scope:registry` settings and ignores the rest. Sources are not
    /// ranked: one that points somewhere private wins over any that says public.
    pub(crate) fn from_settings(settings: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut registries = Self::default();
        for (key, url) in settings {
            registries.note(&key, host(&url) == PUBLIC_REGISTRY);
        }
        registries
    }

    fn note(&mut self, key: &str, public: bool) {
        if key == "registry" || key == "default" {
            self.default_is_public &= public;
        } else if let Some(scope) = scope_of_setting(key) {
            *self.scope_is_public.entry(scope.to_owned()).or_insert(true) &= public;
        }
    }

    pub(crate) fn is_public(&self, package: &str) -> bool {
        let scope = package
            .strip_prefix('@')
            .and_then(|_| package.split('/').next());
        scope
            .and_then(|scope| self.scope_is_public.get(scope))
            .copied()
            .unwrap_or(self.default_is_public)
    }
}

/// `@scope:registry` in `.npmrc`; a bare `@scope` under `registries:` in `pnpm-workspace.yaml`.
fn scope_of_setting(key: &str) -> Option<&str> {
    let scope = key.strip_suffix(":registry").unwrap_or(key);
    (scope.starts_with('@') && !scope.contains(':')).then_some(scope)
}

/// Every `key=value` or `key: value` line whose value is a URL, with quotes dropped. Both files
/// are scanned the same way, so a setting is found whichever of them a pnpm version keeps it in.
fn settings_in(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let (key, value) = line.split_once('=').or_else(|| line.split_once(": "))?;
            let clean = |part: &str| part.trim().trim_matches(['\'', '"']).to_lowercase();
            let (key, value) = (clean(key), clean(value));
            (value.starts_with("http://") || value.starts_with("https://")).then_some((key, value))
        })
        .collect()
}

fn host(url: &str) -> &str {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    after_scheme.split(['/', ':']).next().unwrap_or_default()
}

fn home() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::{Registries, settings_in};

    fn registries(text: &str) -> Registries {
        Registries::from_settings(settings_in(text))
    }

    #[test]
    fn with_nothing_configured_every_name_is_public() {
        let registries = registries("");

        assert!(registries.is_public("left-pad"));
        assert!(registries.is_public("@babel/core"));
    }

    #[test]
    fn a_scope_pointed_at_another_registry_is_private_and_the_rest_stays_public() {
        let registries = registries(
            "# company packages\n@acme:registry=https://npm.acme.example/\n//npm.acme.example/:_authToken=${TOKEN}\n",
        );

        assert!(!registries.is_public("@acme/ui"));
        assert!(registries.is_public("@acmetools/ui"));
        assert!(registries.is_public("acme"));
        assert!(registries.is_public("left-pad"));
    }

    #[test]
    fn a_default_registry_that_is_not_the_public_one_makes_every_unscoped_name_private() {
        let registries = registries(
            "registry=https://artifactory.acme.example/api/npm/all/\n@types:registry=https://registry.npmjs.org/\n",
        );

        assert!(!registries.is_public("left-pad"));
        assert!(!registries.is_public("@acme/ui"));
        assert!(registries.is_public("@types/node"));
    }

    #[test]
    fn the_public_registry_named_explicitly_changes_nothing() {
        let registries = registries("registry = \"https://registry.npmjs.org/\"\n");

        assert!(registries.is_public("left-pad"));
    }

    #[test]
    fn a_host_that_only_contains_the_name_of_the_public_registry_is_not_it() {
        let registries = registries("registry=https://registry.npmjs.org.acme.example/\n");

        assert!(!registries.is_public("left-pad"));
    }

    #[test]
    fn one_private_source_wins_over_another_that_says_public() {
        let registries = Registries::from_settings([
            (
                "@acme:registry".to_owned(),
                "https://registry.npmjs.org/".to_owned(),
            ),
            (
                "@acme:registry".to_owned(),
                "https://npm.acme.example/".to_owned(),
            ),
            (
                "registry".to_owned(),
                "https://npm.acme.example/".to_owned(),
            ),
            (
                "registry".to_owned(),
                "https://registry.npmjs.org/".to_owned(),
            ),
        ]);

        assert!(!registries.is_public("@acme/ui"));
        assert!(!registries.is_public("left-pad"));
    }

    #[test]
    fn registries_in_a_workspace_file_are_read_in_either_spelling() {
        let registries = registries(
            "packages:\n  - 'packages/*'\nregistries:\n  default: https://registry.npmjs.org/\n  '@acme': https://npm.acme.example/\n'@corp:registry': https://npm.corp.example/\n",
        );

        assert!(!registries.is_public("@acme/ui"));
        assert!(!registries.is_public("@corp/ui"));
        assert!(registries.is_public("left-pad"));
    }

    #[test]
    fn settings_that_are_not_registries_are_ignored() {
        let registries = registries(
            "proxy=http://proxy.acme.example:8080\nstrict-ssl=false\n//npm.acme.example/:_authToken=abc\n",
        );

        assert_eq!(registries, Registries::default());
    }
}
