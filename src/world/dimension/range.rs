/// Represents the base range in any given 3D dimension- X, Y, and Z.
/// The concrete ranges delegate logic to the structure

use core::ops::AddAssign;
use serde::{Serialize, Deserialize};

macro_rules! create_range {
    ($x: ident) =>
    {
        /// Represents a range in a dimension
         #[derive(Copy, Clone, Debug, Serialize, Deserialize)]
        pub struct $x
        {
            pub min: f32,
            pub max: f32
        }

        impl $x
        {
            /// Creates a new range that represents the given space
            ///
            /// `min` - the starting point of the range
            /// `max` - the end point of the range. This variable must be bigger than min
            #[allow(dead_code)]
            pub fn new(min: f32, max: f32) -> $x { $x{ min, max } }

            /// Get the centre of the range
            #[allow(dead_code)]
            pub fn centre(&self) -> f32
            {
                (self.min + self.max) / 2.0
            }

            /// Combine two ranges such that the resulting range can hold both ranges
            ///
            /// `other_range` - the other range to combine with this range
            #[allow(dead_code)]
            pub fn combine(&self, other_range: &$x) -> $x
            {
                let epsilon = 0.01; // TODO: Epsilon should not be hard coded

                let min = if (self.min - epsilon) < other_range.min
                {
                    self.min
                }
                else
                {
                    other_range.min
                };

                let max = if (self.max + epsilon) > other_range.max
                {
                    self.max
                }
                else
                {
                    other_range.max
                };

                $x{ min, max }
            }

            /// Get the length of the range
            #[allow(dead_code)]
            pub fn length(&self) -> f32 { self.max - self.min }

            /// Check if the other range overlaps with this one
            ///
            /// `range` - the other range to check for an overlap
            #[allow(dead_code)]
            pub fn overlap_range(&self, range: &$x) -> bool { self.min <= range.max && self.max >= range.min }

            /// Checks if a point is within the range
            ///
            /// `point` - point to check if it is within this range
            #[allow(dead_code)]
            pub fn point_within(&self, point: f32) -> bool { self.min <= point && point <= self.max }

            /// Move the range by the given amount
            ///
            /// `amount` - the amount by which to move the range by
            #[allow(dead_code)]
            pub fn translate(&mut self, amount: f32){ self.min += amount; self.max += amount; }
        }

        impl AddAssign<f32> for $x
        {
            fn add_assign(&mut self, translation: f32)
            {
                self.min += translation;
                self.max += translation;
            }
        }
    };
}

create_range!(XRange);
create_range!(YRange);
create_range!(ZRange);