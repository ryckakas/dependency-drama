//! Finds a project's lockfile and hands it to the reader for its package manager.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::model::Project;
use crate::npm;
use crate::npmrc::Registries;
use crate::pnpm;

#[derive(Debug)]
pub(crate) enum ReadError {
    Missing(PathBuf),
    Unreadable(PathBuf, io::Error),
    Malformed(PathBuf, String),
    /// npm 5 and 6 wrote lockfileVersion 1, which has no `packages` map.
    TooOld,
    /// A `pnpm-lock.yaml` in a layout from before pnpm 9, with the version it declares.
    UnsupportedPnpm(String),
}

impl ReadError {
    /// The lockfile is fine, only too old to read: said once, without failing anyone's build.
    pub(crate) fn is_an_old_lockfile(&self) -> bool {
        matches!(self, Self::TooOld | Self::UnsupportedPnpm(_))
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(
                f,
                "no lockfile at {}. I read {} and {}: install there first, or point me at a \
                 project that has one.",
                path.display(),
                npm::LOCKFILE,
                pnpm::LOCKFILE
            ),
            Self::Unreadable(path, error) => write!(f, "cannot read {}: {error}", path.display()),
            Self::Malformed(path, error) => {
                write!(
                    f,
                    "{} is not a lockfile I can read: {error}",
                    path.display()
                )
            }
            Self::TooOld => write!(
                f,
                "This lockfile was written by npm 6 or older. Too old to gossip about: \
                 run `npm install` with a current npm and the drama can begin."
            ),
            Self::UnsupportedPnpm(version) => write!(
                f,
                "This pnpm-lock.yaml says lockfileVersion {}. Too old to gossip about: \
                 run `pnpm install` with pnpm 9 or newer and the drama can begin.",
                if version.is_empty() {
                    "nothing"
                } else {
                    version
                }
            ),
        }
    }
}

/// `path` is a project directory or a lockfile. In a directory with both lockfiles, npm's is
/// read.
pub(crate) fn read(path: &Path) -> Result<Project, ReadError> {
    let file = locate(path);
    if file.file_name().is_some_and(|name| name == pnpm::LOCKFILE) {
        let project = file.parent().unwrap_or_else(|| Path::new("."));
        pnpm::read(&file, &Registries::discover(project))
    } else {
        npm::read(&file)
    }
}

fn locate(path: &Path) -> PathBuf {
    if !path.is_dir() {
        return path.to_path_buf();
    }
    let npm = path.join(npm::LOCKFILE);
    let pnpm = path.join(pnpm::LOCKFILE);
    if pnpm.exists() && !npm.exists() {
        pnpm
    } else {
        npm
    }
}

pub(crate) fn text(file: &Path) -> Result<String, ReadError> {
    fs::read_to_string(file).map_err(|error| match error.kind() {
        io::ErrorKind::NotFound => ReadError::Missing(file.to_path_buf()),
        _ => ReadError::Unreadable(file.to_path_buf(), error),
    })
}

/// The name of the directory a lockfile sits in, for a project that does not name itself.
pub(crate) fn directory_name(lockfile: &Path) -> Option<String> {
    let directory = lockfile
        .canonicalize()
        .ok()?
        .parent()?
        .file_name()?
        .to_str()?
        .to_owned();
    Some(directory)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{ReadError, read};

    const NPM: &str = r#"{"lockfileVersion":3,"packages":{"":{"dependencies":{"from-npm":"^1"}},"node_modules/from-npm":{"version":"1.0.0"}}}"#;
    const PNPM: &str = "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n      from-pnpm:\n        specifier: ^1\n        version: 1.0.0\npackages:\n  from-pnpm@1.0.0:\n    resolution: {integrity: sha512-x==}\nsnapshots:\n  from-pnpm@1.0.0: {}\n";

    fn names(files: &[(&str, &str)]) -> Vec<String> {
        let directory = tempfile::tempdir().expect("temp dir");
        for (name, text) in files {
            fs::write(directory.path().join(name), text).expect("write");
        }
        let project = read(directory.path()).expect("reads");
        project.packages.into_iter().map(|p| p.name).collect()
    }

    #[test]
    fn a_directory_is_read_from_whichever_lockfile_it_has() {
        assert_eq!(names(&[("package-lock.json", NPM)]), ["from-npm"]);
        assert_eq!(names(&[("pnpm-lock.yaml", PNPM)]), ["from-pnpm"]);
    }

    #[test]
    fn with_both_lockfiles_npm_is_read() {
        let both = [("package-lock.json", NPM), ("pnpm-lock.yaml", PNPM)];

        assert_eq!(names(&both), ["from-npm"]);
    }

    #[test]
    fn a_lockfile_can_be_named_directly() {
        let directory = tempfile::tempdir().expect("temp dir");
        let file = directory.path().join("pnpm-lock.yaml");
        fs::write(&file, PNPM).expect("write");
        fs::write(directory.path().join("package-lock.json"), NPM).expect("write");

        let project = read(&file).expect("reads");

        assert_eq!(project.packages[0].name, "from-pnpm");
    }

    #[test]
    fn a_directory_without_a_lockfile_names_both_kinds_it_looked_for() {
        let directory = tempfile::tempdir().expect("temp dir");
        let error = read(directory.path()).expect_err("missing");

        assert!(matches!(error, ReadError::Missing(_)));
        let message = error.to_string();
        assert!(message.contains("package-lock.json") && message.contains("pnpm-lock.yaml"));
    }

    #[test]
    fn only_an_old_lockfile_is_forgiven() {
        assert!(ReadError::TooOld.is_an_old_lockfile());
        assert!(ReadError::UnsupportedPnpm("6.0".to_owned()).is_an_old_lockfile());
        assert!(!ReadError::Malformed("x".into(), "bad".to_owned()).is_an_old_lockfile());
    }
}
