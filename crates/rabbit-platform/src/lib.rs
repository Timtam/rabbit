//! Platform-specific OS API bindings for RABBIT.
//!
//! This crate isolates the Windows/macOS native-API code (file-version probes,
//! and in future slices: disk-image mounting, code-signing verification) from
//! the cross-platform package engine in `rabbit-core`. Functions here return
//! plain Rust types (no `RabbitError`, no `Version`) so the boundary stays one
//! way: `rabbit-core` depends on `rabbit-platform`, never the reverse.
//!
//! The first slice exports `read_file_version_parts`, which wraps the Windows
//! VersionInfo APIs. On macOS and other targets it is a no-op that returns
//! `None` so callers don't have to spread `cfg(windows)` everywhere.

pub mod arch;
pub mod ctrl_c;
pub mod defender;
pub mod disk_image;
pub mod elevation;
pub mod file_version;
pub mod fs_security;
pub mod jaws;
pub mod komplete_kontrol;
pub mod links;
pub mod local_time;
pub mod locale;
pub mod object_facts;
pub mod package_receipts;
pub mod paths;
pub mod pipe;
pub mod process;
pub mod registry;
pub mod signature;
pub mod timed_process;
pub mod trash;
pub mod volume;
#[cfg(windows)]
mod win_util;

pub use arch::is_running_under_rosetta;
pub use ctrl_c::{CtrlCGuard, catch_ctrl_c};
pub use defender::{DefenderExclusionOutcome, ensure_path_excluded};
pub use disk_image::{
    DiskImageError, MountedDiskImage, copy_directory_recursive, find_app_bundle_in_directory,
    install_app_bundle_from_disk_image, mount_disk_image, run_pkg_installer_from_disk_image,
};
pub use elevation::{
    ADMINISTRATOR_SHELL_MAX_STEPS, AdministratorShellReport, AdministratorShellRequest,
    ElevatedChild, ElevatedWindow, ElevationError, ShellCommand, ShellLinesOutcome, ShellStep,
    administrator_shell_text, is_current_process_elevated, is_nsis_destination_argument,
    is_rabbit_file_stem, run_administrator_shell, run_elevated_and_wait, running_as_rabbit,
    spawn_elevated,
};
pub use file_version::{
    read_file_version_parts, read_file_version_string, read_string_file_info_key,
};
pub use fs_security::{
    ADMINISTRATORS_ONLY_SDDL, LockedFile, TRUSTED_OWNER_SIDS, create_admin_only_directory,
    file_owner_is_trusted, file_owner_sid, is_reparse_point, is_trusted_owner_sid,
    machine_common_program_files_dir, machine_program_files_dirs, machine_program_files_x86_dir,
    program_data_dir,
};
pub use jaws::{JawsInstall, detect_jaws_install, detect_jaws_install_under, is_jaws_installed};
pub use komplete_kontrol::is_komplete_kontrol_installed;
pub use links::{remove_empty_dir, remove_junction, remove_regular_file};
pub use local_time::{CivilDateTime, civil_date, civil_date_time, local_utc_offset_seconds};
pub use locale::os_default_locale;
pub use object_facts::{
    ObjectFacts, TrashFolderFacts, effective_uid_is_root, object_facts, user_trash_folder,
    user_trash_folder_facts,
};
pub use package_receipts::{installed_package_ids, parse_pkgutil_pkgs, refresh_audio_units};
pub use paths::{
    user_appdata_dir, user_documents_dir, user_home_dir, user_local_appdata_dir, windows_clap_dir,
    windows_common_program_files_dir, windows_common_program_files_dirs, windows_program_data_dir,
    windows_program_files_dirs, windows_public_desktop_dir, windows_user_desktop_dir,
};
pub use pipe::{
    MAX_PIPE_LINE_BYTES, PipeChannel, PipeServer, UNINSTALL_PIPE_PREFIX, is_uninstall_pipe_name,
    new_uninstall_pipe_name, uninstall_pipe_name,
};
pub use registry::{
    HkcuUninstallRoot, PendingFileRename, RegistryValue, RegistryView, UninstallEntry,
    UninstallViews, ViewEntry, ViewValues, parse_pending_file_renames, pending_file_renames,
    read_uninstall_display_version, read_uninstall_install_location, read_uninstall_value,
};
pub use signature::{SignatureVerdict, verify_executable_signature};
pub use timed_process::{NsisInPlaceArgument, RunningCommand, TimedCommand, TimedRunOutcome};
pub use trash::{TrashError, TrashErrorKind, move_to_trash};
pub use volume::{available_space, same_volume, volume_id};
