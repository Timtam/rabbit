//! Expert mode's wizard side: the environment switch that turns it on at
//! launch, and the channels a run installs from.

use rabbit_core::package::PACKAGE_OSARA;

/// Environment variable that turns the wizard's expert mode on at launch.
/// Expert mode is never saved - RABBIT keeps no settings file - so this is
/// how someone who always wants it gets it.
pub const EXPERT_MODE_ENV: &str = "RABBIT_EXPERT";

/// Whether `RABBIT_EXPERT` asks for expert mode (`1`, `true`, `yes`, `on`).
pub fn expert_mode_requested_by_env() -> bool {
    std::env::var(EXPERT_MODE_ENV).is_ok_and(|value| expert_mode_value_enabled(&value))
}

pub(crate) fn expert_mode_value_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// The channels a wizard run installs from. Outside expert mode that is no
/// channel at all, which is what takes any package that came from a
/// pre-release back to its regular release. In expert mode it is whatever
/// the build choices say.
pub fn wizard_channels(
    expert_mode: bool,
    reaper: Option<String>,
    osara: Option<String>,
) -> rabbit_core::package::PackageChannels {
    let mut channels = rabbit_core::package::PackageChannels::new();
    if expert_mode {
        if let Some(channel) = reaper {
            channels.insert(rabbit_core::package::PACKAGE_REAPER.to_string(), channel);
        }
        if let Some(channel) = osara {
            channels.insert(PACKAGE_OSARA.to_string(), channel);
        }
    }
    channels
}
