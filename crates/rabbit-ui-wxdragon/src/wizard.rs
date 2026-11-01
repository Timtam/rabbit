//! The wizard's data layer: everything the pages show, decide, and report,
//! with no wxWidgets dependency. `wx_app` renders these types; the tests
//! drive them directly.

mod bootstrap;
mod configuration;
mod expert_mode;
mod install;
mod labels;
mod model;
mod outcome;
mod packages;
mod progress;
mod request;
mod review;
mod self_update;
mod summary;
mod target;
mod text;
mod uninstall;
mod uninstall_run;

#[cfg(test)]
mod tests;

pub use bootstrap::*;
pub use configuration::*;
pub use expert_mode::*;
pub use install::*;
pub use model::*;
pub use outcome::*;
pub use packages::*;
pub use progress::*;
pub use request::*;
pub use review::*;
pub use self_update::*;
pub use summary::*;
pub use target::*;
pub use uninstall::*;
pub use uninstall_run::*;
