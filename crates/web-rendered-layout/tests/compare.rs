//! Scoring a render against an original, over small hand-built pages.

mod support;

use support::{BLUE, Card, RED, facts, page_over};
use web_rendered_layout::{
    Facts, ItemKind, PASS_MARK, Viewport, Width, compare, compare_at, items,
};

#[test]
fn identical_pages_pass() {
    let original = facts(&[RED, BLUE]);
    let comparison = compare(&original, &original).expect("the ranges overlap");

    assert!(comparison.pass);
    assert!((comparison.lowest() - 1.0).abs() < f64::EPSILON);
    assert_eq!(comparison.widths["lo"], 320);
    assert_eq!(comparison.widths["hi"], 1440);
    assert_eq!(comparison.by_width.len(), 1121);
    assert_eq!(comparison.worst_original_items.len(), 0);
}

#[test]
fn moved_card_fails() {
    let original = facts(&[RED, BLUE]);
    let moved = facts(&[RED, Card { y: 300.0, ..BLUE }]);
    let comparison = compare(&original, &moved).expect("the ranges overlap");

    assert!(!comparison.pass);
    assert!(comparison.lowest() < PASS_MARK);
    let worst = &comparison.worst_original_items[0];
    assert!(worst.label.contains("div:nth-of-type(2)"), "{worst:?}");
    assert_eq!(worst.first, "320");
    assert_eq!(worst.last, "1440");
}

#[test]
fn recoloured_card_names_the_colour() {
    let original = facts(&[RED]);
    let recoloured = facts(&[Card {
        colour: "rgb(0, 200, 0)",
        ..RED
    }]);
    let found = compare_at(&original, &recoloured, Viewport::at(Width(400)));

    let (item, why) = &found.failed[0];
    assert_eq!(item.kind, ItemKind::Paint);
    assert_eq!(why, "background-color rgb(200, 0, 0)->rgb(0, 200, 0)");
    assert_eq!(found.unpaired.len(), 1);
}

#[test]
fn widths_scale_boxes() {
    let at = |width| items(&facts(&[RED]), Viewport::at(Width(width)));
    let paint = |shown: Vec<web_rendered_layout::Item>| {
        shown.into_iter().find(|item| item.kind == ItemKind::Paint).expect("the card paints").bounds
    };

    assert_eq!(paint(at(400)), [16.0, 16.0, 200.0, 60.0]);
    assert_eq!(paint(at(1000)), [16.0, 16.0, 500.0, 60.0]);
}

#[test]
fn disjoint_widths_refused() {
    let original = facts(&[RED]);
    let elsewhere = Facts::read(&page_over([1600, 1920], &[RED])).expect("hand-built facts read");
    let refused = compare(&original, &elsewhere).expect_err("the ranges do not overlap");

    assert!(refused.to_string().contains("do not overlap"), "{refused}");
}
