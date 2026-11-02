use crate::common::characterization::moved;

#[test]
fn a_moved_known_defect_names_its_issue_and_what_to_do() {
    assert_eq!(moved("#190", "row default", &553, &553), None);
    let message = moved("#190", "row default", &552, &553).expect("a moved pin fails");
    assert!(
        message.starts_with("known defect #190 moved (row default)"),
        "{message}"
    );
    assert!(message.contains("observed 552, pinned 553"), "{message}");
    assert!(
        message.contains("close the issue and drop the pin"),
        "{message}"
    );
}
