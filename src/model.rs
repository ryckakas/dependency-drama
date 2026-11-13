//! The installed packages of a project, in a shape no package manager owns.
//!
//! Readers fill it in; detectors and lines only ever see this. Anything that needs a manager's own
//! rules (where a peer is looked up, how a range is matched) is answered by the reader.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Project {
    pub(crate) name: String,
    pub(crate) packages: Vec<Package>,
    pub(crate) members: Members,
}

/// For each workspace member, by its folder (`.` for the root), the version each of its direct
/// dependencies resolved to. A project without workspaces has only `.`.
pub(crate) type Members = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts about a package, not the states of one thing"
)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) direct: bool,
    /// A name most developers of this ecosystem know, so a second copy is already a story.
    pub(crate) household_name: bool,
    pub(crate) deprecation: Option<String>,
    pub(crate) asks_for_funding: bool,
    pub(crate) runs_install_script: bool,
    pub(crate) peers: Vec<Peer>,
    pub(crate) source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Peer {
    pub(crate) name: String,
    pub(crate) range: String,
    pub(crate) installed: String,
    pub(crate) satisfied: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    /// The ecosystem's public registry, whose package names are public knowledge.
    PublicRegistry,
    /// A private registry, a git URL, a tarball or a local path. Never looked up.
    Other,
}

impl Project {
    pub(crate) fn direct(&self) -> impl Iterator<Item = &Package> {
        self.packages.iter().filter(|package| package.direct)
    }
}

pub(crate) fn major(version: &str) -> Option<u64> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
pub(crate) fn package(name: &str, version: &str) -> Package {
    Package {
        name: name.to_owned(),
        version: version.to_owned(),
        direct: false,
        household_name: false,
        deprecation: None,
        asks_for_funding: false,
        runs_install_script: false,
        peers: Vec::new(),
        source: Source::PublicRegistry,
    }
}

#[cfg(test)]
mod tests {
    use super::major;

    #[test]
    fn major_reads_the_leading_number() {
        assert_eq!(major("4.17.21"), Some(4));
        assert_eq!(major("0.3.0-beta.1"), Some(0));
    }

    #[test]
    fn major_of_something_that_is_not_a_version_is_unknown() {
        assert_eq!(major("latest"), None);
        assert_eq!(major(""), None);
    }
}
