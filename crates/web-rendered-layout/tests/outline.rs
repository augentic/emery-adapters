//! The page as regions, containers, and leaves, over small hand-built pages.

mod support;

use support::{BLUE, RED, facts};
use web_rendered_layout::{Role, outline};

#[test]
fn cards_are_leaves_with_their_boxes() {
    let placed = outline(&facts(&[RED, BLUE]));

    assert_eq!(placed.len(), 2);
    assert!(placed.iter().all(|placement| placement.role == Role::Leaf));
    assert_eq!(placed[0].file, "parts/n0.json");
    assert_eq!(
        placed[0].synopsis(),
        "`div` n1: \"Fresh <eggs> & milk\"; at 360px 16,16 180x60; at 768px 16,16 384x60; at 1440px 16,16 720x60"
    );
}
