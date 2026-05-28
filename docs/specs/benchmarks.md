# Quality benchmarks plan

This document defines how Memryzed measures and publishes the
quality of its memory and retrieval. It exists so that:

- Users have credible numbers to evaluate Memryzed before adopting
  it.
- Contributors have a stable target to optimize against.
- Regressions are caught quickly and visibly.
- Comparisons against other memory tools rest on shared, public
  methodology rather than marketing claims.

The benchmarks are run against the v1.0 implementation and are
published as part of the v1.x release cycle, not gated as v1.0
acceptance criteria. Performance latency targets are separate and
defined in `v1.md` section 18.

## Why benchmarks matter

Memory products are easy to claim and hard to evaluate. The
existing field publishes inconsistent numbers on inconsistent
splits, often for end-to-end QA accuracy mixed with retrieval
recall, and sometimes after fine-tuning the pipeline against the
test set. We will not contribute to that pattern.

Public, reproducible benchmarks let any user run the same harness
against Memryzed and against any other tool that exposes a similar
interface. That is the only fair comparison.

## Datasets

Four datasets cover the relevant axes. All are public.

### LongMemEval

A retrieval-recall benchmark for long-term memory in
conversational agents. 500 questions covering five task types
(single-session-user, single-session-assistant,
multi-session-information-update, knowledge-update, temporal-
reasoning).

We use it to measure: can the system recall the right past session
when asked a question that depends on it.

### LoCoMo

A long-context conversational memory benchmark. 1,986 questions
across hundreds of long, multi-turn conversations.

We use it to measure: per-session recall at scale. LoCoMo is
deliberately harder than LongMemEval and tests robustness on
denser corpora.

### ConvoMem

250 items in five categories of fifty each. Smaller and faster to
iterate on. We use it to track per-category quality so we can see
where regressions are concentrated.

### MemBench

8,500 items across many categories. Used to measure breadth: does
the retrieval hold up across diverse query patterns.

### Why these four

These four are the de facto standard for memory-product evaluation
in 2026 and are the same datasets MemPalace and others publish
against. Using the same datasets makes our numbers directly
comparable to theirs without us asserting equivalence — readers
can decide.

We do not include synthetic benchmarks we author ourselves in the
headline numbers. Self-authored benchmarks are useful for internal
regression testing but do not belong in marketing claims.

## Metrics

We report the following, in order of importance:

### Recall at K (R@K)

The fraction of queries for which the right document appears in
the top K retrieved results. Reported as R@5 and R@10.

This is the metric that most cleanly measures the retrieval
layer's quality without conflating it with the LLM that consumes
the results.

### Recall by category

For each dataset that defines categories, report R@K per category.
This surfaces per-category strengths and weaknesses rather than
hiding them in an average.

### Retrieval latency at percentiles

p50, p95, p99 latency for retrieval-only queries. Already covered
by the performance budget in the v1 spec, repeated here so quality
and speed appear together.

### Index size and rebuild time

How large the index gets per N memories, and how long it takes to
rebuild from scratch. Memory-product viability degrades quickly
with size; we publish these numbers so users can plan.

## What we do not report as headline numbers

- End-to-end QA accuracy with an LLM on top. The LLM dominates
  the result and obscures the retrieval. We report it separately
  if at all, never as a headline.
- Numbers after LLM-rerank. LLM-rerank improves any reasonable
  retrieval; comparing rerank-on against rerank-off is not a
  comparison of memory systems. If we report rerank numbers, they
  are clearly labeled and accompany the un-reranked numbers from
  the same run.
- Numbers after iterating against a held-out test split. Any
  hyperparameter that has been tuned against the dataset is
  declared. The headline number is the held-out, never-tuned-on
  number.

## Methodology

### Reproducibility

Every published number is reproducible from the repository:

- The harness lives at `benchmarks/`.
- A single command per benchmark runs the full evaluation.
- Per-question results files are committed alongside the harness so
  anyone can audit which questions we got right and wrong.
- Datasets are not redistributed by us; the harness includes
  scripts to download them from their original sources.

### Train/test discipline

For each dataset:

- A small dev split (no more than 10 percent) is allowed for
  development of heuristics, with the dev split documented and
  declared.
- The held-out test split is the published number. It is run after
  changes are frozen for a release and reported as-is.
- If the dev split is exhausted and we want to tune further, we
  rotate to a new dev split and document the change in
  `CHANGELOG.md`. Old test numbers stay published with their
  original split for honesty.

### Pipeline disclosure

Every published number lists the exact pipeline that produced it:

- Embedding model and version.
- Vector backend and version.
- Whether full-text search is on or off.
- Recency boost weight if not the default.
- Whether the LLM rerank stage was used and which model.
- Whether the candidate set was capped before rerank.

A user reading our numbers should be able to reconstruct the
pipeline that produced them without inferring anything.

### What is fair to compare

Memryzed numbers are comparable to other tools' numbers when:

- The same dataset, split, and metric are used.
- The pipeline is disclosed for both.
- No tool has been tuned on the test split.

If any of those is not true, the comparison is unfair and we will
say so. We will not publish side-by-side comparison tables with
other tools whose pipelines we cannot inspect.

### Frequency

Benchmarks run:

- On every tagged release as part of CI, with results attached to
  the GitHub Release.
- On every pull request labeled `benchmark`, against a small
  subset for fast feedback.
- Manually on demand for research or contributor exploration.

## Honesty principles

Five principles guide every published number.

1. We publish losses too. If a competitor beats us on a category
   or a dataset, we report that fact and explain why we think it
   happens.
2. We do not optimize for headlines. The headline number is the
   honest, held-out number. Better numbers from non-comparable
   pipelines go in a footnote at most.
3. We do not retroactively change benchmark numbers. Once
   published with a version, the number stays linked to that
   version. New runs produce new numbers tied to new versions.
4. We do not make qualitative claims that are not measured. If we
   want to claim "better recall on long conversations," we
   measure it. If we cannot measure it, we do not claim it.
5. We name our weaknesses. If retrieval quality drops on
   non-English corpora because v1 is English-tuned, we say so in
   the published numbers and link to the multilingual roadmap
   item.

## Versioning of published numbers

Every published number is tagged with:

- The Memryzed version that produced it.
- The dataset version.
- The harness commit hash.

A number from Memryzed v0.2.0 is not directly comparable to a
number from v0.3.0; the version is part of the result.

Historical published numbers are kept on the project website at a
stable URL so a user reading old marketing material can still find
the numbers it cited.

## When the first numbers ship

The first published numbers ride with v1.1 or v1.2, after v1.0
ships and stabilizes. We do not gate v1.0 on hitting any
particular number. We do gate v1.1 publication on having numbers
that we are willing to defend in public.

If the v1.0 implementation produces numbers we are not willing to
publish, we do not publish them. We publish nothing rather than
publishing weak numbers we will be tempted to spin.

## Tools we will use

The benchmark harness uses:

- Standard Rust test infrastructure to drive evaluation runs.
- A small Python helper for dataset preparation, since most of the
  benchmarks ship with Python loaders. The Python helper is
  optional; a Rust-only path is preferred where the dataset format
  permits it.
- The same `criterion` crate we use for performance benchmarks,
  for any latency measurement that runs in the harness.
- Plain JSON output for results, so anyone can post-process them.

The harness is part of the open-source repository under
`benchmarks/`. There is no proprietary measurement code.

## Future work in benchmarking

Items that may be added to this plan as the project matures:

- Real-world query corpora collected from telemetry (opt-in only).
- Multi-turn agent simulations that test memory across long
  workflows, not single-query retrieval.
- Adversarial benchmarks that test memory contamination
  resistance.
- Cross-tool benchmarks where we run several memory products
  through the same harness and publish the results, after asking
  each project for permission and sharing the methodology.

These are not v1.x commitments; they are open questions for the
research direction.
