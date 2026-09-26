//! Record owners (ADR-0015/0019): a closure obligation is owned by a verified
//! judgment record, checked by a verifier compiled into xtask. A record owner
//! is exactly as strong as its verifier; nothing here runs a shell command.
use super::*;

/// A compiled-in record verifier, registered by the record's schema.
pub(super) struct Verifier {
    name: &'static str,
    schema: &'static str,
    record: &'static str,
    /// The record's own approval subject computation (no `sha256:` prefix).
    #[cfg(test)]
    subject: fn(&Value) -> Result<String, String>,
    /// Full in-process verification of the repository's record.
    verify: fn() -> Result<(), String>,
}

const VERIFIERS: &[Verifier] = &[
    Verifier {
        name: "pdf-judgments",
        schema: pdf_judgments::SCHEMA,
        record: pdf_judgments::RECORD,
        #[cfg(test)]
        subject: pdf_judgments::record_subject,
        verify: pdf_judgments::verify_owned_record,
    },
    Verifier {
        name: "layers",
        schema: layers::SCHEMA,
        record: layers::RECORD,
        #[cfg(test)]
        subject: layers::record_subject,
        verify: layers::verify_owned_record,
    },
];

fn verifier(name: &str) -> Result<&'static Verifier, String> {
    VERIFIERS
        .iter()
        .find(|v| v.name == name)
        .ok_or_else(|| format!("unknown record verifier {name:?}"))
}

fn unbound(obligation: &str, problem: impl std::fmt::Display) -> String {
    format!("stale or unbound record owner for {obligation}: {problem}")
}

/// Every ADR-0015 record-owner check. `read` supplies the record bytes (the
/// repository in production); the registered verifier always runs against the
/// repository record, after every cheaper check has passed.
fn check(
    obligation: &str,
    record: &Descriptor,
    _approval_subject: &str,
    verifier_name: &str,
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let fail = |problem: String| unbound(obligation, problem);
    if !obligation.starts_with("closure:") {
        return Err(fail(
            "record owners are allowed only for closure obligations".into(),
        ));
    }
    let verifier = verifier(verifier_name).map_err(fail)?;
    if record.path != verifier.record {
        return Err(fail(format!(
            "verifier {} checks {}, not {}",
            verifier.name, verifier.record, record.path
        )));
    }
    let bytes = read(&record.path).map_err(fail)?;
    if sha256(&bytes) != record.sha256 {
        return Err(fail(format!("record digest mismatch for {}", record.path)));
    }
    let value: Value = parse(&bytes).map_err(fail)?;
    if value["schema"] != verifier.schema {
        return Err(fail(format!(
            "record schema {} is not registered for verifier {}",
            value["schema"], verifier.name
        )));
    }
    if value["compatibility_revision"] != 1 {
        return Err(fail("unsupported compatibility revision".into()));
    }
    if value["obligation"] != obligation {
        return Err(fail(format!(
            "record names obligation {}, not {obligation}",
            value["obligation"]
        )));
    }
    (verifier.verify)().map_err(fail)
}

pub(super) fn verify(
    obligation: &str,
    record: &Descriptor,
    approval_subject: &str,
    verifier: &str,
) -> Result<(), String> {
    check(obligation, record, approval_subject, verifier, &read)
}

/// The fully verified record owner of `obligation` for the registered
/// verifier's current repository record.
pub(super) fn current_owner(obligation: &str, verifier_name: &str) -> Result<Owner, String> {
    let verifier = verifier(verifier_name).map_err(|e| unbound(obligation, e))?;
    let record = descriptor(verifier.record)?;
    let approval_subject_sha256 = String::new();
    verify(obligation, &record, &approval_subject_sha256, verifier_name)?;
    Ok(Owner::Record {
        record,
        approval_subject_sha256,
        verifier: verifier_name.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = pdf_judgments::RECORD;
    const OBLIGATION: &str = "closure:pdf-judgments";

    fn real() -> Value {
        parse(&read(PATH).unwrap()).unwrap()
    }

    fn subject_of(record: &Value) -> String {
        record["approval"]["approval_subject_sha256"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// Checks `record` as if it were the repository record, with a matching
    /// digest. Every case here fails before the verifier runs.
    fn check_value(obligation: &str, record: &Value, subject: &str, verifier: &str) -> String {
        let bytes = serde_json::to_vec(record).unwrap();
        let descriptor = Descriptor {
            path: PATH.into(),
            sha256: sha256(&bytes),
        };
        check(obligation, &descriptor, subject, verifier, &|_| {
            Ok(bytes.clone())
        })
        .unwrap_err()
    }

    #[test]
    fn rejects_every_unbound_record_before_running_the_verifier() {
        let record = real();
        let subject = subject_of(&record);
        let changed = |pointer: &str, value: Value| {
            let mut record = record.clone();
            *record.pointer_mut(pointer).unwrap() = value;
            record
        };
        for (obligation, value, subject, verifier, expected) in [
            (
                "codec:png:alpha",
                record.clone(),
                &subject,
                "pdf-judgments",
                "only for closure obligations",
            ),
            (
                "closure:all-five-layers",
                record.clone(),
                &subject,
                "pdf-judgments",
                "not closure:all-five-layers",
            ),
            (
                OBLIGATION,
                changed("/obligation", json!("closure:all-five-layers")),
                &subject,
                "pdf-judgments",
                "not closure:pdf-judgments",
            ),
            (
                OBLIGATION,
                changed("/compatibility_revision", json!(2)),
                &subject,
                "pdf-judgments",
                "unsupported compatibility revision",
            ),
            (
                OBLIGATION,
                record.clone(),
                &subject,
                "sh -c true",
                "unknown record verifier",
            ),
            (
                OBLIGATION,
                changed("/schema", json!("other/v1")),
                &subject,
                "pdf-judgments",
                "not registered",
            ),
        ] {
            let error = check_value(obligation, &value, subject, verifier);
            assert!(error.contains(expected), "{expected}: {error}");
            assert!(error.contains("stale or unbound record owner"), "{error}");
        }
    }

    #[test]
    fn rejects_a_stale_digest_or_foreign_record_path() {
        let bytes = read(PATH).unwrap();
        let subject = subject_of(&real());
        let reader = |_: &str| Ok(bytes.clone());
        let stale = Descriptor {
            path: PATH.into(),
            sha256: "0".repeat(64),
        };
        let error = check(OBLIGATION, &stale, &subject, "pdf-judgments", &reader).unwrap_err();
        assert!(error.contains("digest mismatch"), "{error}");
        let foreign = Descriptor {
            path: "tests/conformance/coverage/requirements.json".into(),
            sha256: sha256(&bytes),
        };
        let error = check(OBLIGATION, &foreign, &subject, "pdf-judgments", &reader).unwrap_err();
        assert!(error.contains("verifier pdf-judgments checks"), "{error}");
    }

    #[test]
    fn the_layers_verifier_is_registered_for_the_five_layer_record() {
        let layers = verifier("layers").unwrap();
        assert_eq!(
            (layers.schema, layers.record),
            (layers::SCHEMA, layers::RECORD)
        );
        let bytes = read(layers::RECORD).unwrap();
        let value: Value = parse(&bytes).unwrap();
        assert_eq!(value["obligation"], "closure:all-five-layers");
        let descriptor = Descriptor {
            path: layers::RECORD.into(),
            sha256: sha256(&bytes),
        };
        let subject = format!("sha256:{}", (layers.subject)(&value).unwrap());
        let reader = |_: &str| Ok(bytes.clone());
        for (obligation, verifier, expected) in [
            (
                "closure:pdf-judgments",
                "layers",
                "not closure:pdf-judgments",
            ),
            (
                "closure:all-five-layers",
                "pdf-judgments",
                "verifier pdf-judgments checks",
            ),
            (
                "layer:vector:color",
                "layers",
                "only for closure obligations",
            ),
        ] {
            let error = check(obligation, &descriptor, &subject, verifier, &reader).unwrap_err();
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }

    #[test]
    fn the_verified_pdf_judgments_record_owns_its_closure_row() {
        let Owner::Record {
            record,
            approval_subject_sha256,
            verifier,
        } = current_owner(OBLIGATION, "pdf-judgments").unwrap()
        else {
            panic!("expected a record owner")
        };
        assert_eq!(record, descriptor(PATH).unwrap());
        assert!(approval_subject_sha256.is_empty());
        assert_eq!(verifier, "pdf-judgments");
        assert!(current_owner("closure:all-five-layers", "pdf-judgments").is_err());
        assert!(current_owner(OBLIGATION, "unknown").is_err());
    }
}
