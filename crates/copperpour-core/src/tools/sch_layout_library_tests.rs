use super::{assert_verdict, compare, handler, Sheet};
use serde_json::json;

const U1: &str = "symbol cfd52ef9-c5e5-42c8-8e6e-96fd2851ffb8 (U1)";
const U1_B: &str = "symbol eeaaa6d0-0c8a-4106-9515-482a6ea7cab2 (U1)";

fn fixture(name: &str) -> Sheet {
    Sheet::input(&format!("layout_library_kicad10/{name}/metadata.kicad_sch"))
}

async fn expect_changed(candidate: &str, explanations: &[&str]) {
    expect_pair("baseline", candidate, explanations).await;
}

async fn expect_pair(baseline: &str, candidate: &str, explanations: &[&str]) {
    expect_symbol_changes(baseline, candidate, U1, explanations).await;
}

async fn expect_symbol_changes(
    baseline: &str,
    candidate: &str,
    subject: &str,
    explanations: &[&str],
) {
    let before = fixture(baseline);
    let after = fixture(candidate);
    let result = compare(&handler().await, &before, &after, &[]).await;
    let expected: Vec<_> = explanations
        .iter()
        .map(|why| format!("{subject} {why}"))
        .collect();
    assert_eq!(result["reasons"], json!(expected), "{candidate}: {result}");
    assert_verdict(&result, "blocked");
}

macro_rules! case {
    ($test:ident, $fixture:literal, [$($reason:literal),+ $(,)?]) => {
        #[tokio::test]
        async fn $test() {
            expect_changed($fixture, &[$($reason),+]).await;
        }
    };
}

case!(
    duplicate_jumper_flag,
    "duplicate-jumper",
    ["changed duplicate_pin_numbers_are_jumpers"]
);
case!(jumper_group, "jumper-group", ["changed jumper pin groups"]);
case!(
    unplaced_unit_name,
    "unit-name",
    ["changed embedded unit 2 name"]
);
case!(
    unplaced_unit_pin_number,
    "unit-pin-number",
    [
        "changed embedded unit 2 pin list",
        "changed library pin \"17\" metadata",
        "changed library pin \"7\" metadata",
    ]
);
case!(
    unit_pin_x_order,
    "pin-unit-x-order",
    ["changed embedded unit 3 pin list"]
);
case!(
    unit_pin_y_order,
    "pin-unit-y-order",
    ["changed embedded unit 3 pin list"]
);
case!(
    extra_library_field,
    "libpart-field",
    ["changed library field \"AuditVendor\""]
);
case!(
    library_datasheet,
    "libpart-datasheet",
    ["changed library field \"Datasheet\""]
);
case!(
    base_pin_name,
    "libpart-pin-name",
    ["changed library pin \"8\" metadata"]
);
case!(
    last_embedded_definition_survives_kicad_resave,
    "duplicate-library-key",
    ["changed library pin \"8\" metadata"]
);
case!(
    base_pin_type,
    "libpart-pin-type",
    ["changed library pin \"8\" metadata"]
);
case!(
    lib_name_override,
    "lib-name",
    ["changed embedded library identity (lib_name)"]
);
case!(
    unit_pin_membership,
    "pin-unit-membership",
    [
        "changed embedded unit count",
        "changed embedded unit 2 pin list",
        "changed embedded unit 4 pin list",
    ]
);

#[tokio::test]
async fn drawing_and_pin_text_styling_leave_metadata_comparable() {
    let handler = handler().await;
    let before = fixture("baseline");
    for candidate in [
        "drawing",
        "pin-text-style",
        "pin-style",
        "pin-unit-order",
        "unit-default-name",
    ] {
        let after = fixture(candidate);
        let result = compare(&handler, &before, &after, &[]).await;
        assert_eq!(result["reasons"], json!([]), "{candidate}: {result}");
        assert_ne!(result["verdict"], "blocked", "{candidate}: {result}");
    }
}

#[tokio::test]
async fn duplicate_groups_remain_visible() {
    expect_pair(
        "jumper-group",
        "jumper-group-repeated",
        &["changed jumper pin groups"],
    )
    .await;
}

#[tokio::test]
async fn duplicate_pin_placement_can_change_the_exported_base_name() {
    expect_pair(
        "duplicate-pin-before",
        "duplicate-pin-after",
        &["changed library pin \"8\" metadata"],
    )
    .await;
}

#[tokio::test]
async fn adding_one_pin_copy_can_change_a_different_duplicate_winner() {
    expect_pair(
        "duplicate-pin-roster-before",
        "duplicate-pin-roster-after",
        &["cannot establish unchanged library metadata for duplicate pin numbers"],
    )
    .await;
}

#[tokio::test]
async fn ordering_an_empty_number_can_change_the_exported_unit_pins() {
    expect_pair(
        "empty-pin-before",
        "empty-pin-after",
        &["changed embedded unit 3 pin list"],
    )
    .await;
}

#[tokio::test]
async fn common_pins_are_compared_by_the_units_that_really_export_them() {
    let handler = handler().await;
    for (baseline, candidate) in [
        ("common-parity-before", "common-parity-after"),
        ("common-parity-after", "unit-zero-name"),
    ] {
        let before = fixture(baseline);
        let after = fixture(candidate);
        let result = compare(&handler, &before, &after, &[]).await;
        assert_eq!(result["reasons"], json!([]), "{candidate}: {result}");
        assert_ne!(result["verdict"], "blocked", "{candidate}: {result}");
    }
}

#[tokio::test]
async fn numbers_that_sort_equal_can_change_libparts_pin_count() {
    expect_pair(
        "zero-before",
        "zero-after",
        &[
            "changed embedded unit 3 pin list",
            "cannot establish unchanged library metadata for duplicate pin numbers",
        ],
    )
    .await;
}

#[tokio::test]
async fn saved_order_of_equal_sort_keys_can_change_libparts_pin_count() {
    expect_pair(
        "tie-before",
        "tie-swap",
        &["cannot establish unchanged library metadata for duplicate pin numbers"],
    )
    .await;
}

#[tokio::test]
async fn moving_a_tied_number_without_changing_either_order_is_comparable() {
    let result = compare(
        &handler().await,
        &fixture("tie-before"),
        &fixture("tie-after"),
        &[],
    )
    .await;
    assert_eq!(result["reasons"], json!([]), "{result}");
    assert_verdict(&result, "unchanged");
}

#[tokio::test]
async fn moving_a_coincident_pin_can_change_the_exported_unit_order() {
    expect_pair(
        "stack-sort-before",
        "stack-sort-after",
        &["changed embedded unit 3 pin list"],
    )
    .await;
}

#[tokio::test]
async fn identical_coincident_pin_numbers_do_not_make_order_ambiguous() {
    let result = compare(
        &handler().await,
        &fixture("same-number-before"),
        &fixture("same-number-after"),
        &[],
    )
    .await;
    assert_eq!(result["reasons"], json!([]), "{result}");
    assert_verdict(&result, "unchanged");
}

#[tokio::test]
async fn equal_stacked_pin_numbers_keep_the_comparison_open() {
    let before = fixture("stack-before");
    let after = fixture("stack-after");
    let result = compare(&handler().await, &before, &after, &[]).await;
    assert_eq!(result["reasons"], json!([]), "{result}");
    assert_verdict(&result, "unchanged");
}

#[tokio::test]
async fn equivalent_stack_spelling_can_reorder_library_pin_records() {
    expect_pair(
        "overlap-stacks-before",
        "overlap-stacks-after",
        &["changed library pin record order"],
    )
    .await;
}

#[tokio::test]
async fn other_units_can_change_a_coincident_pin_order_in_a_secondary_alias() {
    expect_symbol_changes(
        "alias-tie-before",
        "alias-tie-after",
        U1_B,
        &["changed embedded unit 2 pin list"],
    )
    .await;
}

#[tokio::test]
async fn a_secondary_alias_supplies_its_own_unit_only() {
    let handler = handler().await;
    let before = fixture("secondary-base");
    for candidate in ["secondary-c", "secondary-key"] {
        let result = compare(&handler, &before, &fixture(candidate), &[]).await;
        assert_eq!(result["reasons"], json!([]), "{candidate}: {result}");
        assert_verdict(&result, "unchanged");
    }
    let result = compare(&handler, &before, &fixture("secondary-b"), &[]).await;
    assert_eq!(
        result["reasons"],
        json!(["symbol eeaaa6d0-0c8a-4106-9515-482a6ea7cab2 (U1) changed embedded unit 2 name"]),
        "{result}"
    );
    assert_verdict(&result, "blocked");
}

#[tokio::test]
async fn a_short_alias_cannot_supply_a_unit_it_does_not_define() {
    expect_pair(
        "short-alias-before",
        "short-alias-after",
        &["changed embedded unit 2 name"],
    )
    .await;
}

#[tokio::test]
async fn placed_unit_zero_cannot_hide_an_exported_common_pin_list() {
    expect_pair(
        "phantom-0-before",
        "phantom-0-after",
        &["changed common unit pin list"],
    )
    .await;
}

#[tokio::test]
async fn a_slot_beyond_the_primary_count_cannot_hide_common_pins() {
    expect_pair(
        "phantom-4-before",
        "phantom-4-after",
        &["changed common unit pin list"],
    )
    .await;
}

#[tokio::test]
async fn a_different_saved_reference_can_export_the_whole_alias() {
    expect_symbol_changes(
        "path-ref-before",
        "path-ref-after",
        U1_B,
        &["changed embedded unit 3 name"],
    )
    .await;
}

#[tokio::test]
async fn a_different_saved_unit_can_export_another_alias_slot() {
    expect_symbol_changes(
        "path-unit-before",
        "path-unit-after",
        U1_B,
        &["changed embedded unit 1 name"],
    )
    .await;
}

#[tokio::test]
async fn an_unmatched_saved_path_cannot_narrow_an_alias_to_one_unit() {
    expect_symbol_changes(
        "path-set-before",
        "path-set-after",
        U1_B,
        &["changed embedded unit 3 name"],
    )
    .await;
}

#[tokio::test]
async fn a_retained_alias_resolves_its_own_library_metadata() {
    expect_pair(
        "lib-name",
        "alias-pin-type",
        &["changed library pin \"8\" metadata"],
    )
    .await;
}

#[test]
fn cleanup_gate_does_not_adopt_the_layout_export_guards() {
    let before = fixture("baseline").tree();
    let after = fixture("unit-name").tree();
    assert!(super::super::cleanup_gate(&before, &after, &[]).is_empty());
}
