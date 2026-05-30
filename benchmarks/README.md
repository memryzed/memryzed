# Benchmarks

This directory holds the Memryzed quality benchmark harness. It
measures retrieval quality (recall at K) against the same hybrid
retrieval the product uses.

The methodology and the honesty principles that govern published
numbers are in `docs/specs/benchmarks.md`. This README covers how to
run the harness.

## What is and is not here

Here:

- A runnable harness (`memryzed-bench`) that loads a normalized
  dataset, stores every document in a fresh in-memory Memryzed
  store, runs each question through retrieval, and reports recall
  at K as JSON.
- A normalized dataset format the harness reads.

Not here:

- The datasets themselves. LongMemEval, LoCoMo, ConvoMem, and
  MemBench are license-gated and large. They are not redistributed
  in this repository. Download them from their original sources and
  convert them to the normalized format below.
- Per-dataset converters. Each public dataset has its own shape;
  writing a small converter to the normalized format is the price
  of admission and keeps this harness dataset-agnostic.

## Normalized dataset format

A dataset is a single JSON file:

```json
{
  "name": "example",
  "documents": [
    { "id": "d1", "text": "The deploy command is make ship." },
    { "id": "d2", "text": "The team uses Vitest for tests." }
  ],
  "questions": [
    { "id": "q1", "query": "how do we deploy", "answer_doc_ids": ["d1"] }
  ]
}
```

- `documents` are stored as global memories.
- Each `question` is run through retrieval; a hit is counted when
  any `answer_doc_ids` entry appears in the top-K results.

## Running

For a real quality number, run with the embedding model active:

```
cargo run --release -p memryzed-benchmarks -- \
  --dataset path/to/normalized.json \
  --k 5,10 \
  --out benchmarks/results/example.json
```

For a fast, embedding-free smoke run (full-text leg only; not a
headline number):

```
MEMRYZED_DISABLE_EMBEDDING=1 cargo run -p memryzed-benchmarks -- \
  --dataset path/to/normalized.json
```

## Output

The harness emits a JSON result object:

```json
{
  "dataset": "example",
  "memryzed_version": "0.5.0",
  "embedding_model": "bge-small-en-v1.5",
  "k_values": [5, 10],
  "recall_at_k": [0.93, 0.97],
  "questions": 500,
  "documents": 5000
}
```

Result files belong under `benchmarks/results/`, which is
gitignored. Published numbers are tagged with the Memryzed version
and the embedding model, per `docs/specs/benchmarks.md`.

## Honesty

The headline number is the un-reranked, never-tuned-on-test recall
with the embedding model active. Embedding-disabled runs and any
future rerank runs are clearly labeled and never presented as the
headline. See `docs/specs/benchmarks.md` for the full rules.
