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

//! Time helpers.
//!
//! Storage timestamps are Unix epoch seconds stored as `INTEGER`.
//! Functions inside the crate accept timestamps as parameters so
//! tests can pass deterministic values; callers at the edge use
//! [`now_epoch_seconds`] to get the wall-clock time.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current wall-clock time in Unix epoch seconds.
///
/// Returns `0` if the system clock is set before 1970, which would
/// be a configuration problem rather than a bug we should handle.
pub fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Format an epoch-seconds timestamp as ISO 8601 (UTC).
///
/// Returns the input value rendered as RFC 3339 / ISO 8601 in the
/// `Z` (UTC) timezone. Used for human-readable CLI output.
pub fn format_epoch_iso(secs: i64) -> String {
    use time::OffsetDateTime;
    OffsetDateTime::from_unix_timestamp(secs)
        .ok()
        .and_then(|dt| {
            dt.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_else(|| format!("epoch:{secs}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_returns_a_positive_recent_value() {
        let n = now_epoch_seconds();
        assert!(n > 1_700_000_000, "now must be after 2023-11-14");
    }

    #[test]
    fn format_epoch_returns_rfc3339_for_known_value() {
        // 1970-01-01T00:00:00Z
        assert_eq!(format_epoch_iso(0), "1970-01-01T00:00:00Z");
        // 2026-01-01T00:00:00Z
        assert_eq!(format_epoch_iso(1_767_225_600), "2026-01-01T00:00:00Z");
    }
}
