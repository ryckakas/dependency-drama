//! Asks the public npm registry about direct dependencies: the latest version and the last change.

use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use ureq::Agent;

use crate::model::{Project, Source};

pub(crate) const NPM_REGISTRY: &str = "https://registry.npmjs.org";

const TIMEOUT: Duration = Duration::from_secs(10);
const CONCURRENT_LOOKUPS: usize = 8;
/// The abbreviated document of a package with thousands of releases passes ureq's 10 MB default.
const BODY_LIMIT: u64 = 64 * 1024 * 1024;
const ABBREVIATED: &str = "application/vnd.npm.install-v1+json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistryFacts {
    pub(crate) latest: String,
    /// When the registry entry last changed, as an ISO 8601 timestamp.
    pub(crate) modified: String,
}

pub(crate) trait Registry: Sync {
    /// `None` for any failure: a plot that cannot be proven is dropped, never guessed.
    fn lookup(&self, name: &str) -> Option<RegistryFacts>;
}

#[derive(Debug)]
pub(crate) struct HttpRegistry {
    agent: Agent,
    base_url: String,
}

#[derive(Deserialize)]
struct Packument {
    modified: String,
    #[serde(rename = "dist-tags")]
    dist_tags: DistTags,
}

#[derive(Deserialize)]
struct DistTags {
    latest: String,
}

impl HttpRegistry {
    pub(crate) fn new(base_url: &str, timeout: Duration) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(timeout))
            .user_agent(concat!("dependency-drama/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self {
            agent,
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub(crate) fn npm() -> Self {
        Self::new(NPM_REGISTRY, TIMEOUT)
    }
}

impl Registry for HttpRegistry {
    fn lookup(&self, name: &str) -> Option<RegistryFacts> {
        let url = format!("{}/{}", self.base_url, name.replace('/', "%2F"));
        let mut response = self
            .agent
            .get(&url)
            .header("Accept", ABBREVIATED)
            .call()
            .ok()?;
        let packument: Packument = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_json()
            .ok()?;
        Some(RegistryFacts {
            latest: packument.dist_tags.latest,
            modified: packument.modified,
        })
    }
}

/// Looks up every direct dependency that came from the public registry, and nothing else.
pub(crate) fn lookup_direct(
    project: &Project,
    registry: &dyn Registry,
) -> HashMap<String, RegistryFacts> {
    let mut names: Vec<&str> = project
        .direct()
        .filter(|package| package.source == Source::PublicRegistry)
        .map(|package| package.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    let queue = Mutex::new(names.into_iter());
    let found = Mutex::new(HashMap::new());
    thread::scope(|scope| {
        for _ in 0..CONCURRENT_LOOKUPS {
            scope.spawn(|| work_through(&queue, &found, registry));
        }
    });
    found.into_inner().unwrap_or_default()
}

fn work_through<'a>(
    queue: &Mutex<impl Iterator<Item = &'a str>>,
    found: &Mutex<HashMap<String, RegistryFacts>>,
    registry: &dyn Registry,
) {
    loop {
        let Some(name) = queue.lock().ok().and_then(|mut queue| queue.next()) else {
            return;
        };
        let Some(facts) = registry.lookup(name) else {
            continue;
        };
        if let Ok(mut found) = found.lock() {
            found.insert(name.to_owned(), facts);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    use super::{HttpRegistry, Registry, RegistryFacts, lookup_direct};
    use crate::model::{Package, Project, Source, package};

    struct Recording {
        asked: Mutex<BTreeSet<String>>,
        unknown: &'static str,
    }

    impl Registry for Recording {
        fn lookup(&self, name: &str) -> Option<RegistryFacts> {
            self.asked.lock().expect("lock").insert(name.to_owned());
            (name != self.unknown).then(|| RegistryFacts {
                latest: "2.0.0".to_owned(),
                modified: "2026-01-01T00:00:00.000Z".to_owned(),
            })
        }
    }

    fn recording() -> Recording {
        Recording {
            asked: Mutex::new(BTreeSet::new()),
            unknown: "gone",
        }
    }

    fn direct(name: &str, source: Source) -> Package {
        Package {
            direct: true,
            source,
            ..package(name, "1.0.0")
        }
    }

    fn project(packages: Vec<Package>) -> Project {
        Project {
            name: "shop-web".to_owned(),
            packages,
            members: crate::model::Members::new(),
        }
    }

    #[test]
    fn a_project_without_dependencies_asks_nothing() {
        let registry = recording();

        assert!(lookup_direct(&project(Vec::new()), &registry).is_empty());
        assert!(registry.asked.lock().expect("lock").is_empty());
    }

    #[test]
    fn only_direct_packages_from_the_public_registry_are_asked_about() {
        let registry = recording();
        let project = project(vec![
            direct("express", Source::PublicRegistry),
            direct("@scope/public", Source::PublicRegistry),
            direct("company-secret", Source::Other),
            package("transitive", "1.0.0"),
        ]);

        let found = lookup_direct(&project, &registry);

        let asked: Vec<_> = registry
            .asked
            .lock()
            .expect("lock")
            .iter()
            .cloned()
            .collect();
        assert_eq!(asked, ["@scope/public", "express"]);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn a_failed_lookup_drops_that_package_and_keeps_the_rest() {
        let registry = recording();
        let names: Vec<String> = (0..40).map(|index| format!("package-{index}")).collect();
        let mut packages: Vec<_> = names
            .iter()
            .map(|name| direct(name, Source::PublicRegistry))
            .collect();
        packages.push(direct("gone", Source::PublicRegistry));

        let found = lookup_direct(&project(packages), &registry);

        assert_eq!(found.len(), 40);
        assert!(!found.contains_key("gone"));
    }

    /// Serves one canned response, and reports the request line it was asked.
    fn serve_once(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).expect("read");
            stream.write_all(response.as_bytes()).expect("write");
            String::from_utf8_lossy(&request[..read]).into_owned()
        });
        (url, server)
    }

    #[test]
    fn the_http_registry_reads_latest_and_modified() {
        let (url, server) = serve_once(concat!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 76\r\n",
            "Connection: close\r\n\r\n",
            r#"{"modified":"2022-05-01T12:00:00.000Z","dist-tags":{"latest":"5.1.0"},"x":1}"#,
        ));

        let facts = HttpRegistry::new(&url, Duration::from_secs(5)).lookup("@scope/name");

        assert_eq!(
            facts,
            Some(RegistryFacts {
                latest: "5.1.0".to_owned(),
                modified: "2022-05-01T12:00:00.000Z".to_owned(),
            })
        );
        let request = server.join().expect("server");
        assert!(
            request.starts_with("GET /@scope%2Fname HTTP/1.1"),
            "{request}"
        );
        assert!(
            request
                .to_lowercase()
                .contains("accept: application/vnd.npm.install-v1+json")
        );
    }

    #[test]
    fn a_package_the_registry_does_not_know_gives_nothing() {
        let (url, server) = serve_once(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
        );

        assert_eq!(
            HttpRegistry::new(&url, Duration::from_secs(5)).lookup("nope"),
            None
        );
        server.join().expect("server");
    }

    #[test]
    fn a_registry_that_never_answers_gives_nothing_after_the_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", listener.local_addr().expect("address"));

        assert_eq!(
            HttpRegistry::new(&url, Duration::from_millis(200)).lookup("slow"),
            None
        );
    }

    #[test]
    fn an_answer_that_is_not_a_package_document_gives_nothing() {
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot json!",
        );

        assert_eq!(
            HttpRegistry::new(&url, Duration::from_secs(5)).lookup("odd"),
            None
        );
        server.join().expect("server");
    }
}
