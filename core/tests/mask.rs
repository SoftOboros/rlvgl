//! LPAR-08 alpha mask primitive and composition tests.

use rlvgl_core::mask::{AlphaMask, FadeDirection, FadeMask, IntersectMask, RectMask, UnionMask};
use rlvgl_core::widget::Rect;

#[test]
fn rect_mask_covers_inside_and_zeros_outside() {
    let mask = RectMask::new(Rect {
        x: 2,
        y: 3,
        width: 4,
        height: 2,
    });

    let mut row = [99u8; 8];
    mask.row(0, 3, &mut row);
    assert_eq!(row, [0, 0, 255, 255, 255, 255, 0, 0]);

    row.fill(99);
    mask.row(0, 2, &mut row);
    assert_eq!(row, [0; 8]);

    row.fill(99);
    mask.row(0, 5, &mut row);
    assert_eq!(row, [0; 8]);

    assert_eq!(mask.alpha_at(2, 3), 255);
    assert_eq!(mask.alpha_at(5, 4), 255);
    assert_eq!(mask.alpha_at(6, 4), 0);
}

#[test]
fn fade_mask_ramps_in_all_directions() {
    let rect = Rect {
        x: 10,
        y: 20,
        width: 5,
        height: 5,
    };

    let left_to_right = FadeMask::new(rect, FadeDirection::LeftToRight, 0, 200);
    let mut row = [99u8; 5];
    left_to_right.row(10, 22, &mut row);
    assert_eq!(row, [0, 50, 100, 150, 200]);

    let right_to_left = FadeMask::new(rect, FadeDirection::RightToLeft, 0, 200);
    right_to_left.row(10, 22, &mut row);
    assert_eq!(row, [200, 150, 100, 50, 0]);

    let top_to_bottom = FadeMask::new(rect, FadeDirection::TopToBottom, 0, 200);
    let top_to_bottom_rows = [
        top_to_bottom.alpha_at(10, 20),
        top_to_bottom.alpha_at(10, 21),
        top_to_bottom.alpha_at(10, 22),
        top_to_bottom.alpha_at(10, 23),
        top_to_bottom.alpha_at(10, 24),
    ];
    assert_eq!(top_to_bottom_rows, [0, 50, 100, 150, 200]);

    let bottom_to_top = FadeMask::new(rect, FadeDirection::BottomToTop, 0, 200);
    let bottom_to_top_rows = [
        bottom_to_top.alpha_at(10, 20),
        bottom_to_top.alpha_at(10, 21),
        bottom_to_top.alpha_at(10, 22),
        bottom_to_top.alpha_at(10, 23),
        bottom_to_top.alpha_at(10, 24),
    ];
    assert_eq!(bottom_to_top_rows, [200, 150, 100, 50, 0]);

    row.fill(99);
    left_to_right.row(10, 19, &mut row);
    assert_eq!(row, [0; 5]);
}

#[test]
fn intersect_mask_takes_minimum_coverage() {
    let rect = RectMask::new(Rect {
        x: 1,
        y: 0,
        width: 3,
        height: 1,
    });
    let fade = FadeMask::new(
        Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 1,
        },
        FadeDirection::LeftToRight,
        0,
        200,
    );
    let mask = IntersectMask::new(&rect, &fade);

    let mut row = [99u8; 5];
    mask.row(0, 0, &mut row);
    assert_eq!(row, [0, 50, 100, 150, 0]);
}

#[test]
fn union_mask_takes_maximum_coverage() {
    let rect = RectMask::new(Rect {
        x: 1,
        y: 0,
        width: 3,
        height: 1,
    });
    let fade = FadeMask::new(
        Rect {
            x: 0,
            y: 0,
            width: 5,
            height: 1,
        },
        FadeDirection::LeftToRight,
        0,
        200,
    );
    let mask = UnionMask::new(&rect, &fade);

    let mut row = [99u8; 5];
    mask.row(0, 0, &mut row);
    assert_eq!(row, [0, 255, 255, 255, 200]);
}

#[test]
fn empty_coverage_does_not_panic() {
    let rect = RectMask::new(Rect {
        x: 0,
        y: 0,
        width: 2,
        height: 2,
    });
    let fade = FadeMask::new(rect.rect(), FadeDirection::TopToBottom, 255, 0);
    let intersect = IntersectMask::new(&rect, &fade);
    let union = UnionMask::new(&rect, &fade);

    let mut empty = [];
    rect.row(0, 0, &mut empty);
    fade.row(0, 0, &mut empty);
    intersect.row(0, 0, &mut empty);
    union.row(0, 0, &mut empty);

    let mask: &dyn AlphaMask = &rect;
    mask.row(0, 0, &mut empty);
}
