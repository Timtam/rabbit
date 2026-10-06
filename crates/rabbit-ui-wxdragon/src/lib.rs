//! The RABBIT wizard: `wizard` holds the pages' data and decisions, `wx_app`
//! renders them with wxWidgets.

#[cfg(feature = "gui")]
mod wx_app;

mod wizard;

pub use wizard::*;

/// Run the wxDragon wizard. Wraps the internal `wx_app::run` so the merged
/// `rabbit` binary can spawn the GUI without needing to know the module's
/// internals.
#[cfg(feature = "gui")]
pub fn run_gui() {
    wx_app::run();
}
