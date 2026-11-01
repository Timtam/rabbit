//! The named helpers a plan's `post` list runs once a package's files are
//! gone. There is one: setting REAPER back to no language pack when the
//! pack it uses was just removed.

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::reaper_ini::{clear_lang_pack_if_selected, lang_pack_setting};
use crate::rollback::BackupSetWriter;

use super::paths::same_ignoring_case;

/// Set `langpack=` in `reaper.ini` back to `<>` when it names a language
/// pack file among `removed`, the files this run actually moved away. A pack
/// that was kept, because it changed since RABBIT installed it, isn't among
/// them, so REAPER is never switched to English while its pack is still
/// there. Returns the value it replaced.
///
/// The old value goes into the backup set's record for `package_id` before
/// `reaper.ini` changes, so a restore can put that one line back.
pub(crate) fn reset_reaper_langpack_if_selected(
    resource: &Path,
    removed: &[PathBuf],
    ignore_case: bool,
    backups: &mut BackupSetWriter,
    package_id: &str,
) -> Result<Option<String>> {
    let lang_pack_dir = resource.join("LangPack");
    let names: Vec<String> = removed
        .iter()
        .filter(|path| {
            path.parent()
                .is_some_and(|folder| same_ignoring_case(folder, &lang_pack_dir))
        })
        .filter_map(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    if names.is_empty() {
        return Ok(None);
    }
    let Some(current) = lang_pack_setting(resource)? else {
        return Ok(None);
    };
    let names_it = names.iter().any(|name| {
        if ignore_case {
            name.to_lowercase() == current.to_lowercase()
        } else {
            *name == current
        }
    });
    if !names_it {
        return Ok(None);
    }
    backups.set_langpack_before(package_id, Some(current))?;
    match clear_lang_pack_if_selected(resource, &names, ignore_case) {
        Ok(Some(previous)) => Ok(Some(previous)),
        Ok(None) => {
            backups.set_langpack_before(package_id, None)?;
            Ok(None)
        }
        Err(error) => {
            // The line didn't change, so there is nothing for a restore to
            // put back.
            let _ = backups.set_langpack_before(package_id, None);
            Err(error)
        }
    }
}
