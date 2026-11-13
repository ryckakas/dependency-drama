//! Reads npm's `package-lock.json` (lockfileVersion 2 and 3) into the neutral model.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use nodejs_semver::{Range, Version};
use serde::Deserialize;

use crate::lockfile::{self, ReadError};
use crate::model::{Members, Package, Peer, Project, Source};

pub(crate) const LOCKFILE: &str = "package-lock.json";
const PUBLIC_REGISTRY: &str = "https://registry.npmjs.org/";
const NODE_MODULES: &str = "node_modules/";

/// Names an npm user recognises on sight.
const HOUSEHOLD_NAMES: &[&str] = &[
    "@babel/core",
    "axios",
    "eslint",
    "express",
    "jquery",
    "lodash",
    "moment",
    "react",
    "react-dom",
    "rxjs",
    "typescript",
    "vue",
    "webpack",
];

pub(crate) fn is_household_name(name: &str) -> bool {
    HOUSEHOLD_NAMES.contains(&name)
}

#[derive(Deserialize)]
struct Lockfile {
    name: Option<String>,
    packages: Option<BTreeMap<String, Entry>>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Entry {
    name: Option<String>,
    version: Option<String>,
    resolved: Option<String>,
    link: bool,
    os: Option<serde_json::Value>,
    cpu: Option<serde_json::Value>,
    deprecated: Option<serde_json::Value>,
    funding: Option<serde_json::Value>,
    has_install_script: bool,
    dependencies: BTreeMap<String, String>,
    dev_dependencies: BTreeMap<String, String>,
    optional_dependencies: BTreeMap<String, String>,
    peer_dependencies: BTreeMap<String, String>,
}

pub(crate) fn read(path: &Path) -> Result<Project, ReadError> {
    let file = if path.is_dir() {
        path.join(LOCKFILE)
    } else {
        path.to_path_buf()
    };
    let text = lockfile::text(&file)?;
    let lockfile: Lockfile = serde_json::from_str(&text)
        .map_err(|error| ReadError::Malformed(file.clone(), error.to_string()))?;
    let entries = lockfile.packages.ok_or(ReadError::TooOld)?;
    let root = entries.get("");
    let name = root
        .and_then(|root| root.name.clone())
        .or(lockfile.name)
        .or_else(|| lockfile::directory_name(&file))
        .unwrap_or_else(|| "this project".to_owned());
    let direct = direct_paths(&entries);
    let packages = entries
        .iter()
        .filter_map(|(path, entry)| package(&entries, &direct, path, entry))
        .collect();
    let members = members(&entries);
    Ok(Project {
        name,
        packages,
        members,
    })
}

/// Where the dependencies declared by the root and by every workspace member are installed. In a
/// monorepo the root often declares nothing, and the members ask for everything.
fn direct_paths(entries: &BTreeMap<String, Entry>) -> BTreeSet<String> {
    entries
        .iter()
        .filter(|(folder, _)| !folder.contains(NODE_MODULES))
        .flat_map(|(folder, manifest)| {
            declared(manifest).map(move |name| installed_path(entries, folder, name))
        })
        .collect()
}

/// The root is the folder `""` in the lockfile and `.` in the model, as pnpm writes it.
fn members(entries: &BTreeMap<String, Entry>) -> Members {
    entries
        .iter()
        .filter(|(folder, _)| !folder.contains(NODE_MODULES))
        .map(|(folder, manifest)| {
            let dependencies = declared(manifest)
                .filter_map(|name| resolved(entries, folder, name))
                .collect();
            let member = if folder.is_empty() { "." } else { folder };
            (member.to_owned(), dependencies)
        })
        .collect()
}

/// The version a member's dependency resolved to. A link to another member has none.
fn resolved(
    entries: &BTreeMap<String, Entry>,
    folder: &str,
    name: &str,
) -> Option<(String, String)> {
    let entry = entries.get(&installed_path(entries, folder, name))?;
    let version = entry.version.clone().filter(|_| !entry.link)?;
    Some((name.to_owned(), version))
}

fn declared(manifest: &Entry) -> impl Iterator<Item = &str> {
    [
        &manifest.dependencies,
        &manifest.dev_dependencies,
        &manifest.optional_dependencies,
    ]
    .into_iter()
    .flat_map(|names| names.keys().map(String::as_str))
}

/// A member's own copy when it has one, the shared copy at the root otherwise.
fn installed_path(entries: &BTreeMap<String, Entry>, folder: &str, name: &str) -> String {
    let shared = format!("{NODE_MODULES}{name}");
    let own = format!("{folder}/{shared}");
    if !folder.is_empty() && entries.contains_key(&own) {
        own
    } else {
        shared
    }
}

fn package(
    entries: &BTreeMap<String, Entry>,
    direct: &BTreeSet<String>,
    path: &str,
    entry: &Entry,
) -> Option<Package> {
    let installed_as = installed_name(path)?;
    if entry.link || is_platform_specific(entry) {
        return None;
    }
    let version = entry.version.clone()?;
    let name = entry
        .name
        .clone()
        .unwrap_or_else(|| installed_as.to_owned());
    Some(Package {
        household_name: is_household_name(&name),
        name,
        version,
        direct: direct.contains(path),
        deprecation: deprecation(entry),
        asks_for_funding: entry.funding.is_some(),
        runs_install_script: entry.has_install_script,
        peers: peers(entries, path, entry),
        source: source(entry),
    })
}

/// The lockfile lists every platform's optional binaries, but one machine installs only its own,
/// so none of them can be claimed as installed.
fn is_platform_specific(entry: &Entry) -> bool {
    entry.os.is_some() || entry.cpu.is_some()
}

fn installed_name(path: &str) -> Option<&str> {
    let start = path.rfind(NODE_MODULES)? + NODE_MODULES.len();
    Some(&path[start..])
}

fn deprecation(entry: &Entry) -> Option<String> {
    match entry.deprecated.as_ref()? {
        serde_json::Value::String(message) => Some(message.trim().to_owned()),
        serde_json::Value::Bool(false) | serde_json::Value::Null => None,
        _ => Some(String::new()),
    }
}

fn source(entry: &Entry) -> Source {
    match &entry.resolved {
        Some(url) if url.starts_with(PUBLIC_REGISTRY) => Source::PublicRegistry,
        _ => Source::Other,
    }
}

fn peers(entries: &BTreeMap<String, Entry>, path: &str, entry: &Entry) -> Vec<Peer> {
    entry
        .peer_dependencies
        .iter()
        .filter_map(|(name, range)| {
            let installed = resolve(entries, path, name)?.version.clone()?;
            let satisfied = satisfies(&installed, range)?;
            Some(Peer {
                name: name.clone(),
                range: range.clone(),
                installed,
                satisfied,
            })
        })
        .collect()
}

/// Finds what `name` means to the package at `path`, the way Node resolves it.
///
/// The package's own `node_modules` comes first: a package that lists a peer as a dependency too
/// gets a private copy there, and a feud claimed against the shared copy would be false.
fn resolve<'a>(entries: &'a BTreeMap<String, Entry>, path: &str, name: &str) -> Option<&'a Entry> {
    let mut base = path;
    loop {
        let separator = if base.is_empty() { "" } else { "/" };
        if let Some(entry) = entries.get(&format!("{base}{separator}{NODE_MODULES}{name}")) {
            return Some(entry);
        }
        if base.is_empty() {
            return None;
        }
        base = base
            .rfind(NODE_MODULES)
            .map_or("", |index| base[..index].trim_end_matches('/'));
    }
}

/// `None` when the version or the range is not semver (a git URL, a tag), so nothing can be claimed.
pub(crate) fn satisfies(version: &str, range: &str) -> Option<bool> {
    // npm accepts anything for these without consulting semver, `0.0.0-experimental` included.
    if matches!(range.trim(), "" | "*") {
        return Some(true);
    }
    let version = Version::parse(version).ok()?;
    let range = Range::parse(range).ok()?;
    Some(version.satisfies_with_prerelease(&range, true))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::{read, satisfies};
    use crate::lockfile::ReadError;
    use crate::model::{Project, Source};

    fn project_with(lockfile: &str) -> TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        fs::write(directory.path().join("package-lock.json"), lockfile).expect("write lockfile");
        directory
    }

    fn read_text(lockfile: &str) -> Result<Project, ReadError> {
        read(project_with(lockfile).path())
    }

    fn shop_web() -> Project {
        read(Path::new("tests/fixtures/shop-web")).expect("fixture reads")
    }

    #[test]
    fn a_real_lockfile_reads_into_packages() {
        let project = shop_web();

        assert_eq!(project.name, "shop-web");
        assert_eq!(project.packages.len(), 461);
        assert_eq!(project.direct().count(), 9);
    }

    #[test]
    fn a_real_lockfile_keeps_the_facts_the_plots_need() {
        let project = shop_web();
        let find = |name: &str| {
            project
                .packages
                .iter()
                .find(|package| package.name == name)
                .expect("installed")
        };

        assert!(
            find("request")
                .deprecation
                .as_deref()
                .is_some_and(|m| m.contains("deprecated"))
        );
        assert!(find("core-js").asks_for_funding);
        assert!(find("esbuild").runs_install_script);
        assert_eq!(find("axios").version, "0.21.4");
        assert_eq!(find("express").source, Source::PublicRegistry);
        assert!(!find("accepts").direct);
    }

    #[test]
    fn a_project_without_dependencies_has_no_packages() {
        let project =
            read_text(r#"{"name":"empty","lockfileVersion":3,"packages":{"":{"name":"empty"}}}"#)
                .expect("reads");

        assert_eq!(project.name, "empty");
        assert_eq!(project.packages, []);
    }

    #[test]
    fn one_dependency_is_one_direct_package() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"name":"solo","dependencies":{"ms":"^2.1.3"}},
                "node_modules/ms":{"version":"2.1.3","resolved":"https://registry.npmjs.org/ms/-/ms-2.1.3.tgz"}
            }}"#,
        )
        .expect("reads");

        assert_eq!(project.packages.len(), 1);
        assert!(project.packages[0].direct);
    }

    #[test]
    fn a_nested_copy_of_a_direct_dependency_is_not_direct() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"dependencies":{"ms":"^2.1.3","debug":"^2.6.9"}},
                "node_modules/ms":{"version":"2.1.3"},
                "node_modules/debug":{"version":"2.6.9"},
                "node_modules/debug/node_modules/ms":{"version":"2.0.0"}
            }}"#,
        )
        .expect("reads");
        let direct: Vec<_> = project.direct().map(|p| p.version.as_str()).collect();

        assert_eq!(direct, ["2.6.9", "2.1.3"]);
    }

    #[test]
    fn a_package_from_elsewhere_is_marked_so_it_is_never_looked_up() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"dependencies":{"secret":"1.0.0","forked":"github:me/forked"}},
                "node_modules/secret":{"version":"1.0.0","resolved":"https://npm.example.com/secret/-/secret-1.0.0.tgz"},
                "node_modules/forked":{"version":"1.0.0","resolved":"git+ssh://git@github.com/me/forked.git#abc"}
            }}"#,
        )
        .expect("reads");

        assert!(
            project
                .packages
                .iter()
                .all(|package| package.source == Source::Other)
        );
    }

    #[test]
    fn workspace_links_and_workspace_folders_are_not_packages() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"workspaces":["packages/*"]},
                "node_modules/ui":{"resolved":"packages/ui","link":true},
                "packages/ui":{"version":"1.0.0"}
            }}"#,
        )
        .expect("reads");

        assert_eq!(project.packages, []);
    }

    #[test]
    fn what_a_workspace_member_asks_for_is_direct_too() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"workspaces":["packages/*"],"devDependencies":{"typescript":"^5"}},
                "node_modules/typescript":{"version":"5.6.3"},
                "node_modules/ui":{"resolved":"packages/ui","link":true},
                "node_modules/api":{"resolved":"packages/api","link":true},
                "node_modules/react":{"version":"18.3.1"},
                "node_modules/loose-envify":{"version":"1.4.0"},
                "node_modules/lodash":{"version":"4.17.21"},
                "packages/ui":{"version":"1.0.0","dependencies":{"react":"^18","lodash":"^3","api":"*"}},
                "packages/ui/node_modules/lodash":{"version":"3.10.1"},
                "packages/api":{"version":"1.0.0","dependencies":{"lodash":"^4"}}
            }}"#,
        )
        .expect("reads");
        let direct: Vec<_> = project
            .direct()
            .map(|package| format!("{} {}", package.name, package.version))
            .collect();

        assert_eq!(
            direct,
            [
                "lodash 4.17.21",
                "react 18.3.1",
                "typescript 5.6.3",
                "lodash 3.10.1"
            ]
        );
        assert_eq!(project.packages.len(), 5);
        assert_eq!(
            format!("{:?}", project.members),
            r#"{".": {"typescript": "5.6.3"}, "packages/api": {"lodash": "4.17.21"}, "packages/ui": {"lodash": "3.10.1", "react": "18.3.1"}}"#
        );
    }

    #[test]
    fn a_peer_is_resolved_from_the_nearest_node_modules() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{},
                "node_modules/react":{"version":"18.2.0"},
                "node_modules/app/node_modules/react":{"version":"16.14.0"},
                "node_modules/app/node_modules/widget":{"version":"1.0.0","peerDependencies":{"react":"^18.0.0","absent":"*"}}
            }}"#,
        )
        .expect("reads");
        let widget = project
            .packages
            .iter()
            .find(|p| p.name == "widget")
            .expect("widget");

        assert_eq!(widget.peers.len(), 1);
        assert_eq!(widget.peers[0].installed, "16.14.0");
        assert!(!widget.peers[0].satisfied);
    }

    #[test]
    fn a_private_copy_of_a_peer_wins_over_the_shared_one() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{},
                "node_modules/ajv":{"version":"6.12.6"},
                "node_modules/ajv-formats":{"version":"2.1.1","peerDependencies":{"ajv":"^8.0.0"}},
                "node_modules/ajv-formats/node_modules/ajv":{"version":"8.12.0"}
            }}"#,
        )
        .expect("reads");
        let formats = project
            .packages
            .iter()
            .find(|p| p.name == "ajv-formats")
            .expect("installed");

        assert_eq!(formats.peers[0].installed, "8.12.0");
        assert!(formats.peers[0].satisfied);
    }

    #[test]
    fn a_peer_of_a_workspace_dependency_falls_back_to_the_root() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{},
                "node_modules/react":{"version":"18.2.0"},
                "packages/ui/node_modules/widget":{"version":"1.0.0","peerDependencies":{"react":"^17.0.0"}}
            }}"#,
        )
        .expect("reads");
        let widget = project
            .packages
            .iter()
            .find(|p| p.name == "widget")
            .expect("installed");

        assert_eq!(widget.peers[0].installed, "18.2.0");
        assert!(!widget.peers[0].satisfied);
    }

    #[test]
    fn peer_ranges_follow_npm_rules_at_the_boundaries() {
        assert_eq!(satisfies("1.2.3", "^1.2.3"), Some(true));
        assert_eq!(satisfies("1.2.2", "^1.2.3"), Some(false));
        assert_eq!(satisfies("2.0.0", "^1.2.3"), Some(false));
        assert_eq!(satisfies("0.2.5", "^0.2.3"), Some(true));
        assert_eq!(satisfies("0.3.0", "^0.2.3"), Some(false));
        assert_eq!(satisfies("1.2.9", "~1.2.3"), Some(true));
        assert_eq!(satisfies("1.3.0", "~1.2.3"), Some(false));
        assert_eq!(satisfies("17.0.2", "^16.8.0 || ^17.0.0"), Some(true));
        assert_eq!(satisfies("18.0.0", ">=16 <18"), Some(false));
        assert_eq!(satisfies("3.1.0", "1.x || 3.x"), Some(true));
        assert_eq!(satisfies("5.0.0", "*"), Some(true));
    }

    #[test]
    fn a_prerelease_is_given_the_benefit_of_the_doubt() {
        assert_eq!(
            satisfies("19.0.0-rc.1", "^18.0.0 || ^19.0.0-rc"),
            Some(true)
        );
        assert_eq!(satisfies("19.0.0-rc.1", ">=18"), Some(true));
    }

    #[test]
    fn any_version_at_all_satisfies_a_star_or_an_empty_range() {
        assert_eq!(satisfies("0.0.0-experimental-abc123", "*"), Some(true));
        assert_eq!(satisfies("0.0.0-experimental-abc123", ""), Some(true));
        assert_eq!(satisfies("github:me/fork", "*"), Some(true));
    }

    #[test]
    fn a_package_for_another_platform_is_not_counted_as_installed() {
        let project = read_text(
            r#"{"lockfileVersion":3,"packages":{
                "":{"devDependencies":{"esbuild":"^0.19.0"}},
                "node_modules/esbuild":{"version":"0.19.12","hasInstallScript":true},
                "node_modules/@esbuild/darwin-arm64":{"version":"0.19.12","optional":true,"os":["darwin"],"cpu":["arm64"]},
                "node_modules/fsevents":{"version":"2.3.3","optional":true,"hasInstallScript":true,"os":["darwin"]}
            }}"#,
        )
        .expect("reads");
        let names: Vec<_> = project
            .packages
            .iter()
            .map(|package| package.name.as_str())
            .collect();

        assert_eq!(names, ["esbuild"]);
    }

    #[test]
    fn household_names_are_marked_by_the_reader() {
        let project = shop_web();
        let household = |name: &str| {
            project
                .packages
                .iter()
                .any(|package| package.name == name && package.household_name)
        };

        assert!(household("express"));
        assert!(!household("accepts"));
    }

    #[test]
    fn a_range_that_is_not_semver_claims_nothing() {
        assert_eq!(satisfies("1.0.0", "github:me/fork"), None);
        assert_eq!(satisfies("not-a-version", "^1.0.0"), None);
    }

    #[test]
    fn a_version_one_lockfile_is_too_old() {
        let error = read_text(r#"{"lockfileVersion":1,"dependencies":{}}"#).expect_err("v1");

        assert!(matches!(error, ReadError::TooOld));
        assert!(error.to_string().contains("Too old to gossip about"));
    }

    #[test]
    fn a_missing_lockfile_says_where_it_looked() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = read(directory.path()).expect_err("missing");

        assert!(matches!(error, ReadError::Missing(_)));
        assert!(error.to_string().contains("package-lock.json"));
    }

    #[test]
    fn malformed_json_is_reported_not_panicked_on() {
        let error = read_text("{ not json").expect_err("malformed");

        assert!(matches!(error, ReadError::Malformed(..)));
    }
}
