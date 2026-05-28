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

//! Scope of a memory: the bucket that determines visibility.
//!
//! See `docs/concepts.md` for the user-level explanation.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Where a memory applies.
///
/// Memories are filtered by scope at retrieval time. A memory in
/// `Project` scope is not implicitly available in `Global` scope;
/// scopes do not inherit.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Applies to the user globally, across every project.
    Global,
    /// Applies to a specific project (repository).
    Project,
    /// Applies to a single session within a project.
    Session,
}

impl Scope {
    /// Database string for the `scope_kind` column.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Project => "project",
            Scope::Session => "session",
        }
    }

    /// All scopes in display order.
    pub fn all() -> &'static [Scope] {
        &[Scope::Global, Scope::Project, Scope::Session]
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl FromStr for Scope {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "global" => Ok(Scope::Global),
            "project" => Ok(Scope::Project),
            "session" => Ok(Scope::Session),
            other => Err(Error::Validation(format!(
                "unknown scope {other:?}; expected one of: global, project, session"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_strings() {
        for scope in Scope::all() {
            let s = scope.as_db_str();
            let parsed: Scope = s.parse().unwrap();
            assert_eq!(parsed, *scope);
        }
    }

    #[test]
    fn unknown_string_is_a_validation_error() {
        let err = "wing".parse::<Scope>().unwrap_err();
        assert!(matches!(err, Error::Validation(_)));
    }

    #[test]
    fn db_strings_match_schema_check_constraint() {
        assert_eq!(Scope::Global.as_db_str(), "global");
        assert_eq!(Scope::Project.as_db_str(), "project");
        assert_eq!(Scope::Session.as_db_str(), "session");
    }
}
