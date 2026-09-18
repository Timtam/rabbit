//! The wizard's data layer: everything the pages show, decide, and report,
//! with no wxWidgets dependency. `wx_app` renders these types; the tests
//! drive them directly.

mod bootstrap;
mod configuration;
mod install;
mod labels;
mod model;
mod outcome;
mod packages;
mod request;
mod review;
mod self_update;
mod summary;
mod target;
mod text;

#[cfg(test)]
mod tests;

pub use bootstrap::*;
pub use configuration::*;
pub use install::*;
pub use model::*;
pub use outcome::*;
pub use packages::*;
pub use request::*;
pub use review::*;
pub use self_update::*;
pub use summary::*;
pub use target::*;
