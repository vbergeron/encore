mod simpl_01_dead_code;
mod simpl_02_copy_propagation;
mod simpl_03_constant_fold;
mod simpl_04_beta_contraction;
mod simpl_05_eta_reduction;

pub(crate) use crate::pass::cps_census::{Census, census_expr, count, is_pure};
pub(crate) use crate::pass::cps_subst::subst_expr;

// ── Re-exports ──────────────────────────────────────────────────────────────

pub use simpl_01_dead_code::dead_code;
pub use simpl_02_copy_propagation::copy_propagation;
pub use simpl_03_constant_fold::constant_fold;
pub use simpl_04_beta_contraction::beta_contraction;
pub use simpl_05_eta_reduction::eta_reduction;
