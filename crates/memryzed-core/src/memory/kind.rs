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

//! Kind of a memory: the semantic flavor.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// What kind of fact this memory represents.
///
/// Kind is metadata for the agent and the user; it does not affect
/// retrieval ranking.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// How the user prefers to do things.
    Preference,
    /// An objective statement.
    #[default]
    Fact,
    /// A choice made during work, often with reasoning.
    Decision,
    /// A pending task or follow-up.
    Todo,
}

impl Kind {
    /// Database string.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Kind::Preference => "preference",
            Kind::Fact => "fact",
            Kind::Decision => "decision",
            Kind::Todo => "todo",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_db_str())
    }
}

impl FromStr for Kind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "preference" => Ok(Kind::Preference),
            "fact" => Ok(Kind::Fact),
            "decision" => Ok(Kind::Decision),
            "todo" => Ok(Kind::Todo),
            other => Err(Error::Validation(format!(
                "unknown kind {other:?}; expected one of: preference, fact, decision, todo"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_strings() {
        for kind in [Kind::Preference, Kind::Fact, Kind::Decision, Kind::Todo] {
            let parsed: Kind = kind.as_db_str().parse().unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn default_is_fact() {
        assert_eq!(Kind::default(), Kind::Fact);
    }
}
