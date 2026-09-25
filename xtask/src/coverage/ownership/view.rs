//! A read-only view of the ownership ledger for the five-layer record
//! (ADR-0015). It exposes the catalog rows and checks the primary owner of one
//! row at a time with exactly the per-assignment rules of `validate`, so the
//! record's verifier never validates (and so never recurses into) the closure
//! rows that record owners answer.
use super::*;
use std::cell::RefCell;

/// The primary owner of a row, after its owner checks passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::coverage) enum PrimaryOwner {
    Case(String),
    Test(String),
    Representative(String),
    Record(String),
}

/// A catalog row as the layers record reads it.
pub(in crate::coverage) struct Row<'a> {
    pub(in crate::coverage) id: &'a str,
    pub(in crate::coverage) category: &'a str,
    pub(in crate::coverage) candidates: &'a [String],
}

pub(in crate::coverage) struct LedgerView {
    catalog: Catalog,
    ledger: Ledger,
    checked: RefCell<BTreeMap<String, Result<Option<PrimaryOwner>, String>>>,
    /// Test owner checks by (category, owner): one executable selection
    /// usually owns many rows, and its checks do not depend on the row id.
    tests: RefCell<BTreeMap<String, Result<bool, String>>>,
}

impl LedgerView {
    pub(in crate::coverage) fn load() -> Result<Self, String> {
        let (catalog, ledger) = super::load()?;
        Ok(Self {
            catalog,
            ledger,
            checked: RefCell::default(),
            tests: RefCell::default(),
        })
    }

    /// The digest of the current catalog, which the ledger must bind.
    pub(in crate::coverage) fn catalog_sha256(&self) -> Result<String, String> {
        let digest = digest(&self.catalog)?;
        if self.ledger.schema != SCHEMA
            || self.ledger.compatibility_revision != 1
            || self.ledger.catalog_sha256 != digest
        {
            return Err("stale ownership catalog binding".into());
        }
        Ok(digest)
    }

    pub(in crate::coverage) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.catalog.obligations.iter().map(|o| Row {
            id: &o.id,
            category: &o.category,
            candidates: &o.candidates,
        })
    }

    /// The checked primary owner of `id`: `Ok(None)` for a gap or exclusion,
    /// an error for an unknown row or an owner that fails its checks. A case
    /// owner is also verified as `verify-ownership` verifies it.
    pub(in crate::coverage) fn owner(&self, id: &str) -> Result<Option<PrimaryOwner>, String> {
        if let Some(result) = self.checked.borrow().get(id) {
            return result.clone();
        }
        let result = self.check(id);
        self.checked.borrow_mut().insert(id.into(), result.clone());
        result
    }

    fn check(&self, id: &str) -> Result<Option<PrimaryOwner>, String> {
        let row = self
            .catalog
            .obligations
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("unknown ownership identity {id}"))?;
        let assignment = self
            .ledger
            .assignments
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("missing ownership assignment {id}"))?;
        match &assignment.disposition {
            Disposition::Owned {
                owner: owner @ Owner::Test { .. },
                ..
            } if row.category != "tolerance-exception" && !row.id.starts_with("closure:") => {
                let key = format!(
                    "{}\n{}",
                    row.category,
                    serde_json::to_string(owner).map_err(|e| e.to_string())?
                );
                let cached = self.tests.borrow().get(&key).cloned();
                match cached {
                    Some(result) => result?,
                    None => {
                        let result = check_disposition(row, &assignment.disposition);
                        self.tests.borrow_mut().insert(key, result.clone());
                        result?
                    }
                }
            }
            disposition => check_disposition(row, disposition)?,
        };
        let Disposition::Owned { owner, .. } = &assignment.disposition else {
            return Ok(None);
        };
        Ok(Some(match owner {
            Owner::Case { case, .. } => {
                crate::conformance::verify(case, true)?;
                PrimaryOwner::Case(case.clone())
            }
            Owner::Test {
                package,
                source,
                function,
                binary,
                module,
                ..
            } => PrimaryOwner::Test(format!(
                "{} {binary} {}::{function} ({})",
                package.as_deref().unwrap_or(executable::RENDERER),
                module.as_deref().unwrap_or(""),
                source.path
            )),
            Owner::Representative { binding } => {
                PrimaryOwner::Representative(binding.case().into())
            }
            Owner::Record { verifier, .. } => PrimaryOwner::Record(verifier.clone()),
            Owner::TestGroup { group } => PrimaryOwner::Test(format!("group {}", group.path)),
        }))
    }
}

/// In-memory ledger edits for tests and dry runs; nothing here is written.
#[cfg(test)]
impl LedgerView {
    fn assign(&mut self, id: &str, owner: Owner, rationale: &str) -> Result<(), String> {
        self.checked.borrow_mut().clear();
        assign_gaps(&mut self.ledger, &owner, rationale, &[id.to_owned()])
    }

    /// Record a reviewed executable owner for a gap row, as `record-test-owner`.
    pub(in crate::coverage) fn assign_test(
        &mut self,
        package: &str,
        binary: &str,
        module: &str,
        function: &str,
        id: &str,
    ) -> Result<(), String> {
        let owner = test_owner(package, binary, module, function)?;
        self.assign(id, owner, "in-memory dry run")
    }

    /// Record a registered case as the owner of a gap row, as `record-case-owner`.
    pub(in crate::coverage) fn assign_case(&mut self, case: &str, id: &str) -> Result<(), String> {
        let registry: Value = parse(&read(REGISTRY)?)?;
        let entry = array(&registry, "cases")?
            .iter()
            .find(|e| e["id"] == case)
            .ok_or("owner case is not registered")?;
        let owner = Owner::Case {
            case: case.into(),
            manifest: descriptor(&format!("{}/manifest.json", string(entry, "path")?))?,
        };
        self.assign(id, owner, "in-memory dry run")
    }

    /// Turn any row back into an explicit gap.
    pub(in crate::coverage) fn clear(&mut self, id: &str) {
        self.checked.borrow_mut().clear();
        for assignment in &mut self.ledger.assignments {
            if assignment.id == id {
                assignment.disposition = Disposition::Gap {
                    issue: 85,
                    reason: "in-memory gap".into(),
                };
            }
        }
    }

    /// Add a catalog row as an explicit gap and rebind the catalog digest, as
    /// an applied `extend-ownership` would.
    pub(in crate::coverage) fn add_row(&mut self, id: &str, category: &str, candidates: &[&str]) {
        let mut row = self.catalog.obligations[0].clone();
        row.id = id.into();
        row.category = category.into();
        row.candidates = candidates.iter().map(|c| (*c).to_owned()).collect();
        self.catalog.obligations.push(row);
        self.catalog.obligations.sort_by(|a, b| a.id.cmp(&b.id));
        self.ledger.assignments.push(Assignment {
            id: id.into(),
            disposition: Disposition::Gap {
                issue: 85,
                reason: "in-memory extension".into(),
            },
        });
        self.ledger.catalog_sha256 = digest(&self.catalog).unwrap();
        self.checked.borrow_mut().clear();
    }
}
