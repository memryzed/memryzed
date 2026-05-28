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

//! Status of a memory in its lifecycle.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Where a memory sits in its lifecycle.
///
/// `Pending` memories are not used by retrieval; `Approved` and
/// `Pinned` are. `Archived` memories are excluded from retrieval but
/// preserved for audit.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Awaiting user review in the pending queue.
    Pending,
    /// Active and used in retrieval.
    Approved,
    /// Active and never expires.
    Pinned,
    /// Excluded from retrieval; kept for audit.
    Archived,
}

impl Status {
    /// Database string.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Approved => "approved",
            Status::Pinned => "pinned",
            Status::Archived => "archived",
        }
    }

    /// `true` when retrieval should consider memories with this status.
    pub fn is_retrievable(self) -> bool {
        matches!(self, Status::Approved | Status::Pinned)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl FromStr for Status {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Status::Pending),
            "approved" => Ok(Status::Approved),
            "pinned" => Ok(Status::Pinned),
            "archived" => Ok(Status::Archived),
            other => Err(Error::Validation(format!(
                "unknown status {other:?}; expected one of: pending, approved, pinned, archived"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_strings() {
        for status in [
            Status::Pending,
            Status::Approved,
            Status::Pinned,
            Status::Archived,
        ] {
            let parsed: Status = status.as_db_str().parse().unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn retrievable_statuses() {
        assert!(Status::Approved.is_retrievable());
        assert!(Status::Pinned.is_retrievable());
        assert!(!Status::Pending.is_retrievable());
        assert!(!Status::Archived.is_retrievable());
    }
}
