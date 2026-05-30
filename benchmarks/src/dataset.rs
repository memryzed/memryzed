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

//! Dataset model and loader.
//!
//! A benchmark dataset is a normalized set of documents (the haystack
//! to remember) plus questions, each with the id of the document that
//! should be retrieved. The harness loads a dataset from a JSON file
//! in this normalized shape. Adapters that convert LongMemEval,
//! LoCoMo, ConvoMem, and MemBench into this shape live alongside the
//! datasets themselves and are not part of this scaffold, since the
//! datasets are license-gated and not redistributed here.
//!
//! Normalized file shape:
//!
//! ```json
//! {
//!   "name": "longmemeval-s",
//!   "documents": [{ "id": "d1", "text": "..." }],
//!   "questions": [{ "id": "q1", "query": "...", "answer_doc_ids": ["d1"] }]
//! }
//! ```

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A document to be stored and later retrieved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Stable id used to score retrieval hits.
    pub id: String,
    /// The document text that gets remembered.
    pub text: String,
}

/// A question with the set of documents that count as correct hits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// Stable id for reporting.
    pub id: String,
    /// The natural-language query passed to retrieval.
    pub query: String,
    /// Document ids that count as a correct retrieval.
    pub answer_doc_ids: Vec<String>,
}

/// A normalized benchmark dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    /// Dataset name, used in result output.
    pub name: String,
    /// The documents to remember.
    pub documents: Vec<Document>,
    /// The questions to evaluate.
    pub questions: Vec<Question>,
}

impl Dataset {
    /// Load a dataset from a normalized JSON file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let ds: Dataset = serde_json::from_str(&raw)?;
        if ds.documents.is_empty() {
            anyhow::bail!("dataset has no documents");
        }
        if ds.questions.is_empty() {
            anyhow::bail!("dataset has no questions");
        }
        Ok(ds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_valid_dataset() {
        let dir = std::env::temp_dir().join(format!("mzbench-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ds.json");
        std::fs::write(
            &path,
            r#"{"name":"t","documents":[{"id":"d1","text":"x"}],
                "questions":[{"id":"q1","query":"x","answer_doc_ids":["d1"]}]}"#,
        )
        .unwrap();
        let ds = Dataset::load(&path).unwrap();
        assert_eq!(ds.name, "t");
        assert_eq!(ds.documents.len(), 1);
        assert_eq!(ds.questions[0].answer_doc_ids, vec!["d1".to_string()]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn rejects_empty_documents() {
        let dir = std::env::temp_dir().join(format!("mzbench2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.json");
        std::fs::write(
            &path,
            r#"{"name":"t","documents":[],"questions":[{"id":"q","query":"x","answer_doc_ids":[]}]}"#,
        )
        .unwrap();
        assert!(Dataset::load(&path).is_err());
        std::fs::remove_file(&path).ok();
    }
}
