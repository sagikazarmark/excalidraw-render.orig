//! Permanent confirmed failures use exact public outcomes, never generator state.
use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;

use super::{replay, Case, Renderer, Snapshot};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema: String,
    originating_issue: u32,
    cases: Vec<Regression>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Regression {
    discovery: String,
    failure: String,
    minimized: bool,
    reproducer: Case,
    #[serde(default)]
    history: Vec<Case>,
    expected: Vec<Expected>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Expected {
    Rejected { descriptor: Value },
    Artifact { sha256: String, report: Value },
    Failure { descriptor: Value, report: Value },
}

#[test]
fn confirmed_failures_are_manifested_and_replayed() {
    let corpus: Corpus = serde_json::from_slice(include_bytes!(
        "../qualification/render-fuzz-regressions.json"
    ))
    .unwrap();
    assert_eq!(
        corpus.schema,
        "excalidraw-render/render-fuzz-regressions/v1"
    );
    assert_eq!(corpus.originating_issue, 83);
    let mut ids = BTreeSet::new();
    for case in corpus.cases {
        assert!(ids.insert(case.reproducer.id.clone()));
        assert!(!case.discovery.is_empty() && !case.failure.is_empty() && case.minimized);
        for request in case.history.iter().chain(std::iter::once(&case.reproducer)) {
            assert!(
                request.limit_mode != 12 || request.output_ceilings.is_some(),
                "permanent boundary cases must freeze concrete limits"
            );
        }
        let renderer = Renderer::new();
        for prior in &case.history {
            replay(prior, &renderer, false);
        }
        let actual = replay(&case.reproducer, &renderer, false);
        assert_eq!(actual, replay(&case.reproducer, &Renderer::new(), true));
        assert_eq!(actual.len(), case.expected.len());
        for (actual, expected) in actual.iter().zip(case.expected) {
            match (actual, expected) {
                (Snapshot::Rejected(bytes), Expected::Rejected { descriptor }) => {
                    assert_eq!(serde_json::from_slice::<Value>(bytes).unwrap(), descriptor);
                }
                (
                    Snapshot::Failure(bytes, report),
                    Expected::Failure {
                        descriptor,
                        report: expected,
                    },
                ) => {
                    assert_eq!(serde_json::from_slice::<Value>(bytes).unwrap(), descriptor);
                    assert_eq!(serde_json::from_slice::<Value>(report).unwrap(), expected);
                }
                (
                    Snapshot::Artifact(hash, report),
                    Expected::Artifact {
                        sha256,
                        report: expected,
                    },
                ) => {
                    let actual: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
                    assert_eq!(actual, sha256);
                    assert_eq!(serde_json::from_slice::<Value>(report).unwrap(), expected);
                }
                _ => panic!("{} changed outcome kind", case.reproducer.id),
            }
        }
    }
}
