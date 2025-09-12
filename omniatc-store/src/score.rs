use serde::{Deserialize, Serialize};

/// Unit for game score.
///
/// This type is signed to allow representing negative score deltas.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    PartialOrd,
    Serialize,
    Deserialize,
    derive_more::Add,
    derive_more::AddAssign,
    derive_more::Sub,
    derive_more::SubAssign,
)]
pub struct Score(pub i32);
