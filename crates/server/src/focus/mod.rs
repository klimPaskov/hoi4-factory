//! The S04 focus editor: projections, the shared focus operations and the guarded rewrite.
//!
//! People and the agent edit a focus tree through the same [`ops::FocusOp`] list. The host turns
//! it into one `hoi4.focus_rewrite` request ([`session::FocusSession::request`]) and runs it in the
//! task's worktree with hash guards ([`session::FocusSession::rewrite`]). Each applied batch is a
//! C-02 change proposal.

pub mod apply;
pub mod ops;
pub mod projection;
pub mod session;
pub mod store;
