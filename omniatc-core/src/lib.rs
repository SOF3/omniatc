#![warn(clippy::pedantic)]
#![cfg_attr(feature = "precommit-checks", deny(warnings, clippy::pedantic, clippy::dbg_macro))]
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)] // too many false positives from system params.
#![allow(clippy::collapsible_else_if)] // this is usually intentional
#![cfg_attr(not(feature = "precommit-checks"), allow(dead_code, unused_variables, unused_imports))]
#![cfg_attr(feature = "precommit-checks", allow(dead_code))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", warn(warnings, clippy::pedantic, clippy::dbg_macro))] // TODO remove this in the future
#![cfg_attr(feature = "rust-analyzer", allow(unused_imports))] // TODO remove this in the future

pub mod level;
pub mod pid;
pub mod store;
pub mod try_log;
pub use try_log::{
    EntityRefExt as EntityTryLog, EntityWorldMutExt as EntityMutTryLog, QueryExt as QueryTryLog,
    WorldExt as WorldTryLog,
};
pub mod util;
