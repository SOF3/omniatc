use std::marker::PhantomData;
use std::{cmp, ops};

use ordered_float::OrderedFloat;

use super::{Accel, Length, Quantity, Speed};
use crate::LengthBase;

/// A wrapper type for squared distance,
/// used to compare with other distances without the pow2 boilerplate.
pub struct AsSqrt<Dt, Pow> {
    pub(super) norm_squared: OrderedFloat<f32>,
    pub(super) _ph:          PhantomData<(Dt, Pow)>,
}

impl<Dt, Pow> Clone for AsSqrt<Dt, Pow> {
    fn clone(&self) -> Self { *self }
}

impl<Dt, Pow> Copy for AsSqrt<Dt, Pow> {}

impl<Dt, Pow> PartialEq for AsSqrt<Dt, Pow> {
    fn eq(&self, other: &Self) -> bool { self.norm_squared == other.norm_squared }
}
impl<Dt, Pow> Eq for AsSqrt<Dt, Pow> {}

impl<Dt, Pow> Ord for AsSqrt<Dt, Pow> {
    fn cmp(&self, other: &Self) -> cmp::Ordering { self.norm_squared.cmp(&other.norm_squared) }
}

impl<Dt, Pow> PartialOrd for AsSqrt<Dt, Pow> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { Some(self.cmp(other)) }
}

impl<Dt, Pow> PartialEq<Quantity<f32, LengthBase, Dt, Pow>> for AsSqrt<Dt, Pow> {
    fn eq(&self, other: &Quantity<f32, LengthBase, Dt, Pow>) -> bool {
        // Check other >= 0.0 to ensure consistency with PartialOrd
        OrderedFloat(other.0) >= OrderedFloat(0.0)
            && self.norm_squared == OrderedFloat(other.0.powi(2))
    }
}

impl<Dt, Pow> PartialOrd<Quantity<f32, LengthBase, Dt, Pow>> for AsSqrt<Dt, Pow> {
    fn partial_cmp(&self, other: &Quantity<f32, LengthBase, Dt, Pow>) -> Option<cmp::Ordering> {
        if other.0 < 0.0 {
            Some(cmp::Ordering::Greater)
        } else {
            Some(self.norm_squared.cmp(&OrderedFloat(other.0.powi(2))))
        }
    }
}
