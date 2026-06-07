// SPDX-License-Identifier: MIT
//! Cardinal direction enum shared by motion widgets.
//!
//! Motion components such as [`crate::motion::crawl::TextCrawl`] scroll
//! along one of four cardinal directions. The enum lives alongside the
//! other motion infrastructure so all variants consume the same
//! vocabulary.

/// One of four cardinal scroll directions.
///
/// Combined with a [`crate::motion::MotionRate`], a [`Direction`]
/// fully specifies how the visible window slides over a jumbo
/// background buffer: the rate supplies the **speed**, the direction
/// supplies the **axis** and **sign**.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Content scrolls upward (Star Wars crawl default).
    Up,
    /// Content scrolls downward.
    Down,
    /// Content scrolls to the left (news ticker default).
    Left,
    /// Content scrolls to the right.
    Right,
}

impl Direction {
    /// Returns `true` if this direction is vertical (`Up` or `Down`).
    #[inline]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Direction::Up | Direction::Down)
    }

    /// Returns `true` if this direction is horizontal (`Left` or `Right`).
    #[inline]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Direction::Left | Direction::Right)
    }

    /// Sign of the scroll offset along the scroll axis.
    ///
    /// `+1` when the scroll accumulator should *grow* per frame
    /// ([`Direction::Down`] / [`Direction::Right`]) and `-1` when it
    /// should shrink ([`Direction::Up`] / [`Direction::Left`]).
    #[inline]
    pub const fn sign(self) -> i32 {
        match self {
            Direction::Down | Direction::Right => 1,
            Direction::Up | Direction::Left => -1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_directions() {
        assert!(Direction::Up.is_vertical());
        assert!(Direction::Down.is_vertical());
        assert!(!Direction::Left.is_vertical());
        assert!(!Direction::Right.is_vertical());
    }

    #[test]
    fn horizontal_directions() {
        assert!(Direction::Left.is_horizontal());
        assert!(Direction::Right.is_horizontal());
        assert!(!Direction::Up.is_horizontal());
        assert!(!Direction::Down.is_horizontal());
    }

    #[test]
    fn signs() {
        assert_eq!(Direction::Up.sign(), -1);
        assert_eq!(Direction::Down.sign(), 1);
        assert_eq!(Direction::Left.sign(), -1);
        assert_eq!(Direction::Right.sign(), 1);
    }
}
