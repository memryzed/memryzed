// Copyright 2026 Memryzed contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Update checking.
//!
//! Queries the GitHub Releases API for the latest tag and compares
//! it to the running version. The actual binary swap is handled by
//! the CLI (it re-runs the install script), so this module is just
//! the version-comparison and network-check half.

use serde::Deserialize;

use crate::version::VERSION;

/// GitHub repository to check for releases.
pub const RELEASES_API: &str = "https://api.github.com/repos/memryzed/memryzed/releases/latest";

/// Outcome of an update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// The running version is the latest.
    UpToDate {
        /// The current (and latest) version.
        current: String,
    },
    /// A newer version is available.
    Available {
        /// The running version.
        current: String,
        /// The latest available version tag (without leading "v").
        latest: String,
    },
    /// The check could not complete (network failure, DNS, missing
    /// repository, parse error). Treated as a clean "we don't know"
    /// state by callers; never surfaced as an error.
    Unknown {
        /// The current running version.
        current: String,
        /// Human-readable reason for not knowing.
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

/// Check GitHub Releases for a newer version.
///
/// Never errors. Network failures, DNS issues, missing repositories,
/// and parse errors all collapse into [`UpdateStatus::Unknown`] with a
/// human-readable reason. Callers that need to act on the status
/// (such as `memryzed update --check`) treat `Unknown` as informative,
/// not fatal.
pub fn check() -> UpdateStatus {
    let resp = match ureq::get(RELEASES_API)
        .set("User-Agent", &format!("memryzed/{VERSION}"))
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            return UpdateStatus::Unknown {
                current: VERSION.to_string(),
                reason: format!("could not reach the release server: {e}"),
            };
        }
    };

    let release: LatestRelease = match resp.into_json() {
        Ok(r) => r,
        Err(e) => {
            return UpdateStatus::Unknown {
                current: VERSION.to_string(),
                reason: format!("could not parse releases response: {e}"),
            };
        }
    };

    let latest = release.tag_name.trim_start_matches('v').to_string();
    compare(VERSION, &latest)
}

/// Compare two version strings and produce a status. Public for
/// testing; the comparison is a simple semver-ish field compare that
/// treats any pre-release suffix as older than its release.
pub fn compare(current: &str, latest: &str) -> UpdateStatus {
    if is_newer(latest, current) {
        UpdateStatus::Available {
            current: current.to_string(),
            latest: latest.to_string(),
        }
    } else {
        UpdateStatus::UpToDate {
            current: current.to_string(),
        }
    }
}

/// `true` if `a` is a newer version than `b`.
fn is_newer(a: &str, b: &str) -> bool {
    let (a_core, a_pre) = split_pre(a);
    let (b_core, b_pre) = split_pre(b);
    match cmp_core(a_core, b_core) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => {
            // Same core: a release (no pre) beats a pre-release.
            match (a_pre, b_pre) {
                (None, Some(_)) => true,
                (Some(_), None) => false,
                // Two pre-releases or two releases: not "newer".
                _ => false,
            }
        }
    }
}

fn split_pre(v: &str) -> (&str, Option<&str>) {
    match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    }
}

fn cmp_core(a: &str, b: &str) -> std::cmp::Ordering {
    let pa = parse_triplet(a);
    let pb = parse_triplet(b);
    pa.cmp(&pb)
}

fn parse_triplet(core: &str) -> (u64, u64, u64) {
    let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_minor_is_available() {
        let s = compare("0.3.0", "0.4.0");
        assert_eq!(
            s,
            UpdateStatus::Available {
                current: "0.3.0".into(),
                latest: "0.4.0".into()
            }
        );
    }

    #[test]
    fn same_version_is_up_to_date() {
        assert_eq!(
            compare("0.3.0", "0.3.0"),
            UpdateStatus::UpToDate {
                current: "0.3.0".into()
            }
        );
    }

    #[test]
    fn older_latest_is_up_to_date() {
        assert!(matches!(
            compare("0.5.0", "0.4.0"),
            UpdateStatus::UpToDate { .. }
        ));
    }

    #[test]
    fn release_beats_prerelease_of_same_core() {
        // Running a pre-release; the released version of the same core
        // is newer.
        assert!(is_newer("0.4.0", "0.4.0-rc.1"));
        assert!(!is_newer("0.4.0-rc.1", "0.4.0"));
    }

    #[test]
    fn patch_comparison() {
        assert!(is_newer("0.4.1", "0.4.0"));
        assert!(!is_newer("0.4.0", "0.4.1"));
    }
}
