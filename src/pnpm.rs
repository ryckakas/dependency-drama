//! Reads pnpm's `pnpm-lock.yaml` (lockfileVersion 9) into the neutral model.
//!
//! The lockfile does not record funding requests or install scripts, so a pnpm project never gets
//! those two scenes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::de::IgnoredAny;

use crate::lockfile::{self, ReadError};
use crate::model::{Package, Peer, Project, Source};
use crate::npm;
use crate::npmrc::Registries;

pub(crate) const LOCKFILE: &str = "pnpm-lock.yaml";
/// pnpm 9 split the lockfile into `packages` and `snapshots`; earlier layouts are not read.
const SUPPORTED_MAJOR: &str = "9";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Lockfile {
    /// One per workspace member, keyed by its folder; `.` is the root.
    importers: BTreeMap<String, Importer>,
    /// What each `name@version` is, whoever depends on it.
    packages: BTreeMap<String, Entry>,
    /// What each `name@version(peers)` was installed with.
    snapshots: BTreeMap<String, Snapshot>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Importer {
    dependencies: BTreeMap<String, Requested>,
    dev_dependencies: BTreeMap<String, Requested>,
    optional_dependencies: BTreeMap<String, Requested>,
}

#[derive(Debug, Deserialize)]
struct Requested {
    version: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
#[expect(
    clippy::zero_sized_map_values,
    reason = "only the field names of a resolution matter, and YAML has no set to read them from"
)]
struct Entry {
    resolution: BTreeMap<String, IgnoredAny>,
    deprecated: Option<String>,
    peer_dependencies: BTreeMap<String, String>,
    os: Option<IgnoredAny>,
    cpu: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct Manifest {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Snapshot {
    dependencies: BTreeMap<String, String>,
}

pub(crate) fn read(file: &Path, registries: &Registries) -> Result<Project, ReadError> {
    let text = lockfile::text(file)?;
    let version = lockfile_version(&text);
    if version.split('.').next() != Some(SUPPORTED_MAJOR) {
        return Err(ReadError::UnsupportedPnpm(version.to_owned()));
    }
    let lockfile: Lockfile = serde_saphyr::from_str(&text)
        .map_err(|error| ReadError::Malformed(file.to_path_buf(), error.to_string()))?;
    let direct = direct(&lockfile);
    let packages = lockfile
        .packages
        .iter()
        .filter_map(|(key, entry)| package(&lockfile, registries, &direct, key, entry))
        .collect();
    Ok(Project {
        name: manifest_name(file)
            .or_else(|| lockfile::directory_name(file))
            .unwrap_or_else(|| "this project".to_owned()),
        packages,
    })
}

/// The `name` in the `package.json` beside the lockfile, which npm copies into its own lockfile.
/// The directory is only a fallback: a CI checkout is named after the repository, not the package.
fn manifest_name(lockfile: &Path) -> Option<String> {
    let text = fs::read_to_string(lockfile.with_file_name("package.json")).ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    manifest.name.filter(|name| !name.trim().is_empty())
}

/// Read from the text, not the parsed document: older lockfiles write the version as a number and
/// differ in everything else, and they should be turned away by version, not by a parse error.
fn lockfile_version(text: &str) -> &str {
    text.lines()
        .find_map(|line| line.strip_prefix("lockfileVersion:"))
        .map_or("", |version| version.trim().trim_matches(['\'', '"']))
}

/// The `name@version` keys every workspace member asks for. A link to another member is not a
/// package.
fn direct(lockfile: &Lockfile) -> BTreeSet<String> {
    lockfile
        .importers
        .values()
        .flat_map(|importer| {
            [
                &importer.dependencies,
                &importer.dev_dependencies,
                &importer.optional_dependencies,
            ]
        })
        .flatten()
        .filter(|(_, requested)| !requested.version.starts_with("link:"))
        .map(|(name, requested)| format!("{name}@{}", without_peers(&requested.version)))
        .collect()
}

fn package(
    lockfile: &Lockfile,
    registries: &Registries,
    direct: &BTreeSet<String>,
    key: &str,
    entry: &Entry,
) -> Option<Package> {
    // Every platform's optional binaries are listed, but one machine installs only its own.
    if entry.os.is_some() || entry.cpu.is_some() {
        return None;
    }
    let (name, version) = name_and_version(key)?;
    Some(Package {
        name: name.to_owned(),
        version: version.to_owned(),
        direct: direct.contains(key),
        household_name: npm::is_household_name(name),
        deprecation: entry
            .deprecated
            .as_ref()
            .map(|message| message.trim().to_owned()),
        asks_for_funding: false,
        runs_install_script: false,
        peers: peers(lockfile, key, entry),
        source: source(registries, name, entry),
    })
}

/// `@scope/name@1.2.3` and `name@1.2.3`: the separator is the first `@` after the scope's own.
fn name_and_version(key: &str) -> Option<(&str, &str)> {
    let separator = key.get(1..)?.find('@')? + 1;
    Some((&key[..separator], &key[separator + 1..]))
}

/// `1.2.3(react@18.3.1)` is version 1.2.3 installed beside react 18.3.1.
fn without_peers(version: &str) -> &str {
    version.split('(').next().unwrap_or(version)
}

/// A snapshot lists the peer it was really installed with among its dependencies, so nothing is
/// resolved here. A package installed in several peer contexts is checked in each.
fn peers(lockfile: &Lockfile, key: &str, entry: &Entry) -> Vec<Peer> {
    let mut seen = BTreeSet::new();
    lockfile
        .snapshots
        .iter()
        .filter(|(snapshot, _)| without_peers(snapshot) == key)
        .flat_map(|(_, snapshot)| {
            entry.peer_dependencies.iter().filter_map(|(name, range)| {
                let installed = without_peers(snapshot.dependencies.get(name)?);
                Some(Peer {
                    name: name.clone(),
                    range: range.clone(),
                    installed: installed.to_owned(),
                    satisfied: npm::satisfies(installed, range)?,
                })
            })
        })
        .filter(|peer| seen.insert((peer.name.clone(), peer.installed.clone())))
        .collect()
}

/// A package from a registry has only an integrity hash; a tarball URL, a git repository or a
/// directory says it came from somewhere else. Which registry is not recorded, so that is asked
/// of the configuration pnpm itself would read.
fn source(registries: &Registries, name: &str, entry: &Entry) -> Source {
    let from_a_registry = entry.resolution.keys().all(|field| field == "integrity");
    if from_a_registry && registries.is_public(name) {
        Source::PublicRegistry
    } else {
        Source::Other
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{name_and_version, read};
    use crate::lockfile::ReadError;
    use crate::model::{Package, Project, Source};
    use crate::npmrc::Registries;

    fn read_text(lockfile: &str) -> Result<Project, ReadError> {
        read_with(lockfile, &Registries::default())
    }

    fn read_with(lockfile: &str, registries: &Registries) -> Result<Project, ReadError> {
        let directory = project_with(lockfile);
        read(&directory.path().join("pnpm-lock.yaml"), registries)
    }

    fn project_with(lockfile: &str) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temp dir");
        std::fs::write(directory.path().join("pnpm-lock.yaml"), lockfile).expect("write lockfile");
        directory
    }

    fn monorepo() -> Project {
        read(
            Path::new("tests/fixtures/pnpm-monorepo/pnpm-lock.yaml"),
            &Registries::default(),
        )
        .expect("fixture reads")
    }

    fn described(project: &Project) -> Vec<String> {
        project
            .packages
            .iter()
            .map(|package| format!("{} {}{}", package.name, package.version, role(package)))
            .collect()
    }

    fn role(package: &Package) -> &'static str {
        if package.direct { " direct" } else { "" }
    }

    #[test]
    fn a_lockfile_without_dependencies_is_an_empty_project() {
        let project =
            read_text("lockfileVersion: '9.0'\n\nimporters:\n\n  .: {}\n").expect("reads");

        assert_eq!(project.packages, []);
    }

    /// The project's name with this `package.json` beside the lockfile, and its directory's name.
    fn named(manifest: Option<&str>) -> (String, String) {
        let directory = project_with(
            "lockfileVersion: '9.0'
importers:
  .: {}
",
        );
        if let Some(manifest) = manifest {
            std::fs::write(directory.path().join("package.json"), manifest)
                .expect("write manifest");
        }
        let project = read(
            &directory.path().join("pnpm-lock.yaml"),
            &Registries::default(),
        );
        let directory_name = directory.path().canonicalize().expect("temp dir exists");
        let directory_name = directory_name
            .file_name()
            .expect("has a name")
            .to_string_lossy();
        (project.expect("reads").name, directory_name.into_owned())
    }

    #[test]
    fn the_project_is_named_by_its_package_json_and_else_by_its_directory() {
        assert_eq!(named(Some(r#"{"name": "acme"}"#)).0, "acme");
        for manifest in [None, Some("{}"), Some(r#"{"name": " "}"#), Some("not json")] {
            let (name, directory_name) = named(manifest);
            assert_eq!(name, directory_name, "{manifest:?}");
        }
    }

    #[test]
    fn one_dependency_is_one_direct_package() {
        let project = read_text(
            "lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      left-pad:
        specifier: ^1.3.0
        version: 1.3.0
packages:
  left-pad@1.3.0:
    resolution: {integrity: sha512-x==}
    deprecated: use String.prototype.padStart()
snapshots:
  left-pad@1.3.0: {}
",
        )
        .expect("reads");

        assert_eq!(described(&project), ["left-pad 1.3.0 direct"]);
        assert_eq!(
            project.packages[0].deprecation.as_deref(),
            Some("use String.prototype.padStart()")
        );
        assert_eq!(project.packages[0].source, Source::PublicRegistry);
    }

    #[test]
    fn what_every_workspace_member_asks_for_is_direct_and_links_between_members_are_not_packages() {
        let project = monorepo();

        assert_eq!(
            described(&project),
            [
                "@scope/kit 2.0.0 direct",
                "js-tokens 4.0.0",
                "lodash 3.10.1 direct",
                "lodash 4.17.21 direct",
                "loose-envify 1.4.0",
                "react 18.3.1 direct",
                "typescript 5.6.3 direct",
                "widget 1.0.0 direct",
            ]
        );
    }

    #[test]
    fn a_package_for_another_platform_is_not_counted_as_installed() {
        let project = monorepo();

        assert!(!project.packages.iter().any(|p| p.name == "fsevents"));
    }

    #[test]
    fn a_peer_is_judged_against_what_the_snapshot_was_installed_with() {
        let project = monorepo();
        let peers = |name: &str| {
            let package = project.packages.iter().find(|p| p.name == name);
            package.expect("installed").peers.clone()
        };

        let widget = peers("widget");
        assert_eq!(widget.len(), 1);
        assert_eq!(
            (widget[0].name.as_str(), widget[0].range.as_str()),
            ("react", "^17.0.0")
        );
        assert_eq!(widget[0].installed, "18.3.1");
        assert!(!widget[0].satisfied);

        let kit = peers("@scope/kit");
        assert_eq!(
            kit.len(),
            1,
            "the optional peer that is not installed is skipped"
        );
        assert!(kit[0].satisfied);
    }

    #[test]
    fn a_tarball_a_git_repository_or_a_private_registry_is_never_the_public_registry() {
        let lockfile = "lockfileVersion: '9.0'
packages:
  '@acme/ui@1.0.0':
    resolution: {integrity: sha512-x==}
  from-tarball@1.0.0:
    resolution: {integrity: sha512-x==, tarball: https://npm.pkg.github.com/download/x/1.0.0/abc}
  from-git@https://codeload.github.com/me/fork/tar.gz/abc123:
    resolution: {tarball: https://codeload.github.com/me/fork/tar.gz/abc123}
  public@1.0.0:
    resolution: {integrity: sha512-x==}
";
        let registries = Registries::from_settings([(
            "@acme:registry".to_owned(),
            "https://npm.acme.example/".to_owned(),
        )]);

        let project = read_with(lockfile, &registries).expect("reads");
        let sources: Vec<_> = project
            .packages
            .iter()
            .map(|package| (package.name.as_str(), package.source))
            .collect();

        assert_eq!(
            sources,
            [
                ("@acme/ui", Source::Other),
                ("from-git", Source::Other),
                ("from-tarball", Source::Other),
                ("public", Source::PublicRegistry),
            ]
        );
    }

    #[test]
    fn keys_split_at_the_version_even_for_scoped_names_and_urls() {
        assert_eq!(name_and_version("ms@2.1.3"), Some(("ms", "2.1.3")));
        assert_eq!(
            name_and_version("@babel/core@7.29.7"),
            Some(("@babel/core", "7.29.7"))
        );
        assert_eq!(
            name_and_version("fork@https://host/me@x/tar.gz/abc"),
            Some(("fork", "https://host/me@x/tar.gz/abc"))
        );
        assert_eq!(name_and_version("no-version"), None);
        assert_eq!(name_and_version(""), None);
    }

    #[test]
    fn an_older_lockfile_layout_is_turned_away_by_its_version() {
        for lockfile in [
            "lockfileVersion: '6.0'\n",
            "lockfileVersion: 5.4\n",
            "packages: {}\n",
        ] {
            let error = read_text(lockfile).expect_err("not read");

            assert!(matches!(error, ReadError::UnsupportedPnpm(_)), "{lockfile}");
        }
    }

    #[test]
    fn yaml_that_is_not_a_lockfile_is_reported_not_panicked_on() {
        let error = read_text("lockfileVersion: '9.0'\npackages: [1, 2\n").expect_err("malformed");

        assert!(matches!(error, ReadError::Malformed(..)));
    }

    #[test]
    fn a_missing_lockfile_is_reported() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = read(
            &directory.path().join("pnpm-lock.yaml"),
            &Registries::default(),
        )
        .expect_err("missing");

        assert!(matches!(error, ReadError::Missing(_)));
    }
}
