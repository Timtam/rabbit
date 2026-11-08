//! What RABBIT reads about a macOS app before it decides how the app
//! leaves, and that decision.
//!
//! A `to_trash` plan's trees, REAPER.app in the shipped data, go to the
//! Trash. Who may move one decides how:
//! - one this account may move goes to the Trash as the user, through
//!   macOS's own Trash function, with no password;
//! - one only root may move goes into the user's Trash through the run's one
//!   administrator command, when the plan allows that (`elevation:
//!   if_needed`), the app sits directly in `/Applications`, is the app the
//!   manifest names, isn't Locked, nor is `/Applications`, and the user's
//!   Trash is a real folder of the user's on the same volume;
//! - anything else stays, and the user is told how to move it in Finder.
//!
//! The facts come from `rabbit-platform`, which reads them only on macOS.
//! The planner reaches them through [`ObjectProbe`], so its rules run on
//! every platform with stand-ins, and [`tree_route`] makes the decision
//! from them without reading anything itself.
//!
//! The run does a little more as the user, through [`MacosObjects`]: it
//! moves an app to the Trash, lists the installer's receipts before and
//! after the administrator command forgets some, asks hosts to read the
//! Audio Unit list again, and checks that RABBIT doesn't run as root.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use rabbit_platform::{ObjectFacts, TrashError, TrashErrorKind, TrashFolderFacts};

use super::ByHandReason;

/// What the planner asks about files and folders on this computer. Every
/// question only reads.
pub trait ObjectProbe {
    /// What this account may do with `path`, an absolute path, and the
    /// folder holding it. `path` itself is read with `lstat`, so a link is
    /// described as a link.
    fn facts(&self, path: &Path) -> io::Result<ObjectFacts>;

    /// This account's Trash folder, read with `lstat`. `NotFound` when it
    /// doesn't exist.
    fn user_trash_folder_facts(&self) -> io::Result<TrashFolderFacts>;
}

/// What a macOS uninstall run does as the user, beyond the planner's
/// questions. Never anything with administrator rights: that is the
/// administrator command's, see [`super::admin_batch`].
pub trait MacosObjects: ObjectProbe {
    /// Move `path` to the Trash as this user, the way Finder does, and say
    /// where it is now.
    fn move_to_trash(&self, path: &Path) -> Result<PathBuf, TrashError>;

    /// Every receipt macOS's installer lists (`pkgutil --pkgs`), read as
    /// this user.
    fn installed_package_ids(&self) -> io::Result<BTreeSet<String>>;

    /// Make this user's hosts read the Audio Unit list again, after
    /// plug-ins went.
    fn refresh_audio_units(&self) -> io::Result<()>;

    /// Whether RABBIT itself runs as root, as it does under `sudo`. A
    /// computer that can't tell, which is any but a Mac, answers `false`:
    /// the rule about root is macOS's alone.
    fn runs_as_root(&self) -> bool;
}

/// This computer, through `rabbit-platform`. Off macOS every question is
/// answered `Unsupported`, which leaves an app the user would have to move
/// by hand; no plan for another platform asks.
///
/// What changes something, the Trash and ending the Audio Unit registrar,
/// refuses in a test build and in any program not named like RABBIT, as
/// the elevated helper does; `rabbit-platform` refuses again.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemMacosObjects;

impl ObjectProbe for SystemMacosObjects {
    fn facts(&self, path: &Path) -> io::Result<ObjectFacts> {
        rabbit_platform::object_facts(path)
    }

    fn user_trash_folder_facts(&self) -> io::Result<TrashFolderFacts> {
        rabbit_platform::user_trash_folder_facts()
    }
}

/// Why this program may not change anything of the user's through
/// [`SystemMacosObjects`], or `None` when it may.
fn change_refusal() -> Option<&'static str> {
    if cfg!(test) {
        Some("a test build never changes anything of the user's")
    } else if !rabbit_platform::running_as_rabbit() {
        Some(
            "only RABBIT itself changes things of the user's, and this program isn't named like RABBIT",
        )
    } else {
        None
    }
}

impl MacosObjects for SystemMacosObjects {
    fn move_to_trash(&self, path: &Path) -> Result<PathBuf, TrashError> {
        if let Some(reason) = change_refusal() {
            return Err(TrashError {
                kind: TrashErrorKind::Refused,
                message: reason.to_string(),
            });
        }
        rabbit_platform::move_to_trash(path)
    }

    fn installed_package_ids(&self) -> io::Result<BTreeSet<String>> {
        rabbit_platform::installed_package_ids()
    }

    fn refresh_audio_units(&self) -> io::Result<()> {
        if let Some(reason) = change_refusal() {
            return Err(io::Error::other(reason));
        }
        rabbit_platform::refresh_audio_units()
    }

    fn runs_as_root(&self) -> bool {
        // getuid can't fail on macOS; elsewhere the answer is Unsupported,
        // and no rule about root applies there.
        rabbit_platform::effective_uid_is_root().unwrap_or(false)
    }
}

/// Nothing at all: every question is answered `Unsupported`. For runs that
/// never remove a macOS app, such as most tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoMacosObjects;

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "this run reads nothing about macOS apps",
    )
}

impl ObjectProbe for NoMacosObjects {
    fn facts(&self, _path: &Path) -> io::Result<ObjectFacts> {
        Err(unsupported())
    }

    fn user_trash_folder_facts(&self) -> io::Result<TrashFolderFacts> {
        Err(unsupported())
    }
}

impl MacosObjects for NoMacosObjects {
    fn move_to_trash(&self, _path: &Path) -> Result<PathBuf, TrashError> {
        Err(TrashError {
            kind: TrashErrorKind::Unsupported,
            message: "this run moves nothing to the Trash".to_string(),
        })
    }

    fn installed_package_ids(&self) -> io::Result<BTreeSet<String>> {
        Err(unsupported())
    }

    fn refresh_audio_units(&self) -> io::Result<()> {
        Err(unsupported())
    }

    fn runs_as_root(&self) -> bool {
        false
    }
}

/// The installer receipts a [`FakeMacosObjects`] lists, shared with a
/// [`RecordingAdministratorShell`](super::admin_batch::RecordingAdministratorShell)
/// whose `pkgutil --forget` steps take them out. `None` makes listing fail.
pub type FakeReceipts = Arc<Mutex<Option<BTreeSet<String>>>>;

/// How a [`FakeMacosObjects`] answers a move to the Trash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum FakeTrashAnswer {
    /// Move it into the test's Trash folder and say where.
    #[default]
    Move,
    /// Move nothing, and fail like this.
    Fail(TrashErrorKind, String),
    /// Say it moved to the Trash, but leave it where it is.
    ClaimWithoutMoving,
    /// Move it, and say nothing RABBIT can use about where.
    MoveWithoutSaying,
}

/// What a [`FakeMacosObjects`] was asked to change.
#[derive(Debug, Clone, Default)]
struct FakeCalls {
    trashed: Vec<PathBuf>,
    audio_unit_refreshes: usize,
}

/// Facts a test hands in, for paths it names and for the user's Trash.
/// A path it doesn't name, and a Trash it wasn't given, are `NotFound`.
///
/// For a run, a temporary folder stands in for the Trash that moves go
/// into, a list stands in for the installer's receipts, and every change
/// it was asked for is recorded. It changes nothing outside the folders a
/// test gives it. It starts with no receipts listed and no Trash folder to
/// move things into.
#[derive(Debug, Clone)]
pub struct FakeMacosObjects {
    objects: HashMap<PathBuf, Result<ObjectFacts, io::ErrorKind>>,
    trash: Option<Result<TrashFolderFacts, io::ErrorKind>>,
    trash_bin: Option<PathBuf>,
    trash_answer: FakeTrashAnswer,
    receipts: FakeReceipts,
    calls: Arc<Mutex<FakeCalls>>,
    as_root: bool,
}

impl Default for FakeMacosObjects {
    fn default() -> Self {
        Self {
            objects: HashMap::new(),
            trash: None,
            trash_bin: None,
            trash_answer: FakeTrashAnswer::default(),
            receipts: Arc::new(Mutex::new(Some(BTreeSet::new()))),
            calls: Arc::default(),
            as_root: false,
        }
    }
}

impl FakeMacosObjects {
    pub fn new() -> Self {
        Self::default()
    }

    /// Moves to the Trash go into `folder`, which the test made.
    pub fn with_trash_bin(mut self, folder: impl Into<PathBuf>) -> Self {
        self.trash_bin = Some(folder.into());
        self
    }

    /// A move to the Trash moves nothing and fails with `kind`.
    pub fn failing_to_trash(mut self, kind: TrashErrorKind, message: &str) -> Self {
        self.trash_answer = FakeTrashAnswer::Fail(kind, message.to_string());
        self
    }

    /// A move to the Trash says it worked, names a place in the Trash, and
    /// leaves the app where it was.
    pub fn claiming_to_trash_without_moving(mut self) -> Self {
        self.trash_answer = FakeTrashAnswer::ClaimWithoutMoving;
        self
    }

    /// A move to the Trash moves the app, and then answers that it can't
    /// say where it went.
    pub fn trashing_without_saying_where(mut self) -> Self {
        self.trash_answer = FakeTrashAnswer::MoveWithoutSaying;
        self
    }

    /// The installer lists these receipts.
    pub fn with_receipts<I, S>(self, ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        *self.receipts.lock().unwrap() = Some(ids.into_iter().map(Into::into).collect());
        self
    }

    /// Listing the installer's receipts fails.
    pub fn with_unlistable_receipts(self) -> Self {
        *self.receipts.lock().unwrap() = None;
        self
    }

    /// RABBIT runs as root.
    pub fn running_as_root(mut self) -> Self {
        self.as_root = true;
        self
    }

    /// The receipts, for a recording administrator shell to forget.
    pub fn receipt_list(&self) -> FakeReceipts {
        Arc::clone(&self.receipts)
    }

    /// The receipts listed now, `None` while listing fails.
    pub fn receipts(&self) -> Option<BTreeSet<String>> {
        self.receipts.lock().unwrap().clone()
    }

    /// Every path the run asked to move to the Trash, in order.
    pub fn trashed(&self) -> Vec<PathBuf> {
        self.calls.lock().unwrap().trashed.clone()
    }

    /// How often the run asked for the Audio Unit list to be read again.
    pub fn audio_unit_refreshes(&self) -> usize {
        self.calls.lock().unwrap().audio_unit_refreshes
    }

    /// `path` is described by `facts`.
    pub fn with_object(mut self, path: impl Into<PathBuf>, facts: ObjectFacts) -> Self {
        self.objects.insert(path.into(), Ok(facts));
        self
    }

    /// Reading `path` fails with `kind`.
    pub fn with_unreadable_object(mut self, path: impl Into<PathBuf>, kind: io::ErrorKind) -> Self {
        self.objects.insert(path.into(), Err(kind));
        self
    }

    /// The user's Trash is described by `facts`.
    pub fn with_trash(mut self, facts: TrashFolderFacts) -> Self {
        self.trash = Some(Ok(facts));
        self
    }

    /// Reading the user's Trash fails with `kind`.
    pub fn with_unreadable_trash(mut self, kind: io::ErrorKind) -> Self {
        self.trash = Some(Err(kind));
        self
    }
}

impl ObjectProbe for FakeMacosObjects {
    fn facts(&self, path: &Path) -> io::Result<ObjectFacts> {
        match self.objects.get(path) {
            Some(Ok(facts)) => Ok(facts.clone()),
            Some(Err(kind)) => Err(io::Error::new(
                *kind,
                format!("{}: the test made it unreadable", path.display()),
            )),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: the test has no facts for it", path.display()),
            )),
        }
    }

    fn user_trash_folder_facts(&self) -> io::Result<TrashFolderFacts> {
        match &self.trash {
            Some(Ok(facts)) => Ok(facts.clone()),
            Some(Err(kind)) => Err(io::Error::new(*kind, "the test made the Trash unreadable")),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the test has no Trash",
            )),
        }
    }
}

impl MacosObjects for FakeMacosObjects {
    /// Renames `path` into the test's Trash folder, under its own name, or
    /// with a number added when that is taken, as macOS does; or answers
    /// the way the test asked.
    fn move_to_trash(&self, path: &Path) -> Result<PathBuf, TrashError> {
        self.calls.lock().unwrap().trashed.push(path.to_path_buf());
        let fail = |kind: TrashErrorKind, message: &str| TrashError {
            kind,
            message: message.to_string(),
        };
        if let FakeTrashAnswer::Fail(kind, message) = &self.trash_answer {
            return Err(fail(*kind, message));
        }
        let Some(bin) = &self.trash_bin else {
            return Err(fail(TrashErrorKind::Unsupported, "the test has no Trash"));
        };
        let Some(name) = path.file_name() else {
            return Err(fail(TrashErrorKind::Other, "the path has no name"));
        };
        if fs::symlink_metadata(path).is_err() {
            return Err(fail(TrashErrorKind::NotFound, "nothing is there"));
        }
        let mut destination = bin.join(name);
        let mut number = 2;
        while fs::symlink_metadata(&destination).is_ok() {
            let mut numbered = name.to_os_string();
            numbered.push(format!(" {number}"));
            destination = bin.join(numbered);
            number += 1;
        }
        if self.trash_answer == FakeTrashAnswer::ClaimWithoutMoving {
            return Ok(destination);
        }
        fs::rename(path, &destination)
            .map_err(|error| fail(TrashErrorKind::Other, &error.to_string()))?;
        match self.trash_answer {
            FakeTrashAnswer::MoveWithoutSaying => Err(fail(
                TrashErrorKind::Unclear,
                "macOS gave no new path RABBIT could read",
            )),
            _ => Ok(destination),
        }
    }

    fn installed_package_ids(&self) -> io::Result<BTreeSet<String>> {
        self.receipts
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| io::Error::other("the test made the receipts unlistable"))
    }

    fn refresh_audio_units(&self) -> io::Result<()> {
        self.calls.lock().unwrap().audio_unit_refreshes += 1;
        Ok(())
    }

    fn runs_as_root(&self) -> bool {
        self.as_root
    }
}

/// How one tree of a `to_trash` plan leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRoute {
    /// This account may move it: RABBIT moves it to the Trash as the user,
    /// with no password.
    Trash,
    /// Only root may move it, and nothing about it or the user's Trash
    /// looks wrong: the run's administrator command moves it into the
    /// user's Trash, if the plan allows that.
    TrashAsAdministrator,
    /// RABBIT leaves it, and says how to move it in Finder.
    ByHand(ByHandReason),
}

/// How the tree at `path` leaves, from what [`ObjectProbe`] read about it
/// (`facts`) and about the user's Trash (`trash`). `expected_bundle_id` is
/// the manifest's `bundle_id`, and `system_root` the folder absolute macOS
/// paths start from, `/` on a real computer. Reads nothing itself.
///
/// In order:
/// - facts that couldn't be read leave it to the user;
/// - so does anything but a real folder, and a bundle whose identifier
///   isn't the expected one: it isn't the app the manifest means;
/// - so does a Locked bundle (`uchg`, `schg`, `uappnd` or `sappnd`), which
///   not even root can move until the flag is cleared;
/// - one this account may move goes to the Trash as the user;
/// - otherwise only root could move it. It goes through the administrator
///   command only when it sits directly in `/Applications`, a folder only
///   an administrator can change, never a folder of the user's or another
///   volume; the manifest names its bundle identifier; and the user's
///   Trash is a real folder, not a link, that the user owns, on the same
///   volume, so root's move is a rename that can't leave a half-copied app
///   behind. Anything short of that leaves it to the user, and so does an
///   `/Applications` carrying `uchg`, `schg`, `uappnd` or `sappnd`, which
///   stops root's move too.
pub fn tree_route(
    path: &Path,
    facts: &io::Result<ObjectFacts>,
    trash: &io::Result<TrashFolderFacts>,
    expected_bundle_id: Option<&str>,
    system_root: &Path,
) -> TreeRoute {
    let facts = match facts {
        Ok(facts) => facts,
        Err(error) => {
            return TreeRoute::ByHand(ByHandReason::CouldNotCheck {
                error: format!("{}: {error}", path.display()),
            });
        }
    };
    let expected_app = facts.is_dir
        && !facts.is_symlink
        && expected_bundle_id
            .is_none_or(|expected| facts.bundle_identifier.as_deref() == Some(expected));
    if !expected_app {
        return TreeRoute::ByHand(ByHandReason::NotTheExpectedApp);
    }
    if facts.locked {
        return TreeRoute::ByHand(ByHandReason::Locked);
    }
    if movable_by_me(facts) {
        return TreeRoute::Trash;
    }
    let in_applications = path.parent() == Some(system_root.join("Applications").as_path());
    if !in_applications || expected_bundle_id.is_none() || facts.parent_locked {
        return TreeRoute::ByHand(ByHandReason::NotMovableByYou);
    }
    let trash = match trash {
        Ok(trash) => trash,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return TreeRoute::ByHand(ByHandReason::NoTrashFolder);
        }
        Err(error) => {
            return TreeRoute::ByHand(ByHandReason::CouldNotCheck {
                error: format!("your Trash: {error}"),
            });
        }
    };
    if !trash.is_dir || trash.is_symlink || !trash.owned_by_me {
        return TreeRoute::ByHand(ByHandReason::NoTrashFolder);
    }
    if trash.dev != facts.dev {
        return TreeRoute::ByHand(ByHandReason::NotOnTrashVolume);
    }
    TreeRoute::TrashAsAdministrator
}

/// Whether this account can move the object to another folder itself, as
/// macOS's Trash function does: take its name out of its folder
/// (`removable_by_me`, which covers ACLs, the sticky rule and the folder's
/// own flags), add names to and search that folder, and for a folder also
/// write it, because its `..` entry changes. The sticky rule is checked
/// here as well: in a sticky folder only the object's owner or the
/// folder's may move it.
fn movable_by_me(facts: &ObjectFacts) -> bool {
    let sticky_blocks = facts.parent_sticky && !facts.owned_by_me && !facts.parent_owned_by_me;
    facts.removable_by_me
        && facts.parent_writable_searchable
        && (!facts.is_dir || facts.writable)
        && !sticky_blocks
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::{Path, PathBuf};

    use super::{
        FakeMacosObjects, NoMacosObjects, ObjectFacts, ObjectProbe, TrashFolderFacts, TreeRoute,
        tree_route,
    };
    use crate::uninstall::ByHandReason;

    const REAPER_ID: &str = "com.cockos.reaper";
    const ME: u32 = 501;
    const DISK: u64 = 16_777_220;

    fn system_root() -> PathBuf {
        PathBuf::from("/")
    }

    fn app_in_applications() -> PathBuf {
        system_root().join("Applications").join("REAPER.app")
    }

    /// REAPER.app as a user who dragged it into /Applications owns it.
    fn owned_by_me() -> ObjectFacts {
        ObjectFacts {
            is_dir: true,
            is_symlink: false,
            owner_uid: ME,
            owned_by_me: true,
            writable: true,
            parent_writable_searchable: true,
            removable_by_me: true,
            parent_sticky: false,
            parent_owned_by_me: false,
            locked: false,
            parent_locked: false,
            dev: DISK,
            bundle_identifier: Some(REAPER_ID.to_string()),
        }
    }

    /// REAPER.app as an installer running as root left it.
    fn owned_by_root() -> ObjectFacts {
        ObjectFacts {
            owner_uid: 0,
            owned_by_me: false,
            writable: false,
            removable_by_me: false,
            ..owned_by_me()
        }
    }

    fn my_trash() -> TrashFolderFacts {
        TrashFolderFacts {
            path: PathBuf::from("/Users/me/.Trash"),
            is_dir: true,
            is_symlink: false,
            owner_uid: ME,
            owned_by_me: true,
            dev: DISK,
        }
    }

    fn route(path: &Path, facts: ObjectFacts, trash: TrashFolderFacts) -> TreeRoute {
        tree_route(
            path,
            &Ok(facts),
            &Ok(trash),
            Some(REAPER_ID),
            &system_root(),
        )
    }

    #[test]
    fn an_app_this_account_may_move_goes_to_the_trash_as_the_user() {
        assert_eq!(
            route(&app_in_applications(), owned_by_me(), my_trash()),
            TreeRoute::Trash
        );
        // Wherever it is: a portable REAPER's folder, another volume.
        assert_eq!(
            route(
                Path::new("/Volumes/Stick/REAPER/REAPER.app"),
                ObjectFacts {
                    dev: 1,
                    ..owned_by_me()
                },
                my_trash()
            ),
            TreeRoute::Trash
        );
        // Someone else's app in a folder of the user's: the folder decides.
        assert_eq!(
            route(
                &app_in_applications(),
                ObjectFacts {
                    owned_by_me: false,
                    owner_uid: 502,
                    ..owned_by_me()
                },
                my_trash()
            ),
            TreeRoute::Trash
        );
    }

    #[test]
    fn a_root_owned_app_in_applications_goes_through_the_administrator_command() {
        assert_eq!(
            route(&app_in_applications(), owned_by_root(), my_trash()),
            TreeRoute::TrashAsAdministrator
        );
    }

    /// Each of the things that keep this account from moving the app; then
    /// only root can, and in /Applications it may.
    #[test]
    fn what_keeps_this_account_from_moving_the_app() {
        for facts in [
            ObjectFacts {
                removable_by_me: false,
                ..owned_by_me()
            },
            ObjectFacts {
                parent_writable_searchable: false,
                ..owned_by_me()
            },
            // XNU needs write access to a folder to move it, because its
            // `..` entry changes.
            ObjectFacts {
                writable: false,
                ..owned_by_me()
            },
            // Another account's app in a sticky folder neither owns.
            ObjectFacts {
                owned_by_me: false,
                owner_uid: 502,
                parent_sticky: true,
                parent_owned_by_me: false,
                ..owned_by_me()
            },
        ] {
            assert_eq!(
                route(&app_in_applications(), facts.clone(), my_trash()),
                TreeRoute::TrashAsAdministrator,
                "{facts:?}"
            );
        }
        // A sticky folder of the user's own still lets the user move it.
        assert_eq!(
            route(
                &app_in_applications(),
                ObjectFacts {
                    owned_by_me: false,
                    parent_sticky: true,
                    parent_owned_by_me: true,
                    ..owned_by_me()
                },
                my_trash()
            ),
            TreeRoute::Trash
        );
    }

    #[test]
    fn root_never_moves_an_app_outside_applications() {
        for path in [
            "/Users/me/Applications/REAPER.app",
            "/Users/me/REAPER/REAPER.app",
            "/Volumes/Stick/REAPER.app",
            "/Applications/Audio/REAPER.app",
        ] {
            assert_eq!(
                route(Path::new(path), owned_by_root(), my_trash()),
                TreeRoute::ByHand(ByHandReason::NotMovableByYou),
                "{path}"
            );
        }
        // Nor one the manifest names no bundle identifier for.
        assert_eq!(
            tree_route(
                &app_in_applications(),
                &Ok(owned_by_root()),
                &Ok(my_trash()),
                None,
                &system_root()
            ),
            TreeRoute::ByHand(ByHandReason::NotMovableByYou)
        );
    }

    #[test]
    fn the_system_root_places_applications() {
        let root = PathBuf::from("/tmp/system");
        assert_eq!(
            tree_route(
                &root.join("Applications").join("REAPER.app"),
                &Ok(owned_by_root()),
                &Ok(my_trash()),
                Some(REAPER_ID),
                &root
            ),
            TreeRoute::TrashAsAdministrator
        );
        assert_eq!(
            tree_route(
                &app_in_applications(),
                &Ok(owned_by_root()),
                &Ok(my_trash()),
                Some(REAPER_ID),
                &root
            ),
            TreeRoute::ByHand(ByHandReason::NotMovableByYou)
        );
    }

    #[test]
    fn a_locked_app_is_left_to_the_user_whoever_owns_it() {
        for facts in [owned_by_me(), owned_by_root()] {
            assert_eq!(
                route(
                    &app_in_applications(),
                    ObjectFacts {
                        locked: true,
                        ..facts
                    },
                    my_trash()
                ),
                TreeRoute::ByHand(ByHandReason::Locked)
            );
        }
    }

    /// An immutable or append-only `/Applications` refuses root's move as
    /// it refuses the user's, so root is never sent to try.
    #[test]
    fn an_app_in_a_locked_applications_folder_is_left_to_the_user() {
        assert_eq!(
            route(
                &app_in_applications(),
                ObjectFacts {
                    parent_locked: true,
                    ..owned_by_root()
                },
                my_trash()
            ),
            TreeRoute::ByHand(ByHandReason::NotMovableByYou)
        );
    }

    #[test]
    fn an_app_that_isnt_the_expected_one_is_left_to_the_user() {
        for facts in [
            ObjectFacts {
                bundle_identifier: Some("com.example.other".to_string()),
                ..owned_by_me()
            },
            ObjectFacts {
                bundle_identifier: None,
                ..owned_by_root()
            },
            // A link, or a file, is never an app bundle.
            ObjectFacts {
                is_dir: false,
                is_symlink: true,
                ..owned_by_me()
            },
            ObjectFacts {
                is_dir: false,
                ..owned_by_me()
            },
        ] {
            assert_eq!(
                route(&app_in_applications(), facts.clone(), my_trash()),
                TreeRoute::ByHand(ByHandReason::NotTheExpectedApp),
                "{facts:?}"
            );
        }
        // Without an identifier to expect, a folder is taken by its name,
        // which the guard has checked.
        assert_eq!(
            tree_route(
                &app_in_applications(),
                &Ok(ObjectFacts {
                    bundle_identifier: None,
                    ..owned_by_me()
                }),
                &Ok(my_trash()),
                None,
                &system_root()
            ),
            TreeRoute::Trash
        );
    }

    #[test]
    fn facts_that_cant_be_read_leave_the_app_to_the_user() {
        let route = tree_route(
            &app_in_applications(),
            &Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            &Ok(my_trash()),
            Some(REAPER_ID),
            &system_root(),
        );
        assert!(
            matches!(
                &route,
                TreeRoute::ByHand(ByHandReason::CouldNotCheck { error })
                    if error.contains("REAPER.app")
            ),
            "{route:?}"
        );
    }

    /// Root moves the app only into a Trash that is plainly the user's, on
    /// the app's own volume.
    #[test]
    fn root_moves_the_app_only_into_a_plain_trash_of_the_users_on_its_volume() {
        let by_root = |trash: io::Result<TrashFolderFacts>| {
            tree_route(
                &app_in_applications(),
                &Ok(owned_by_root()),
                &trash,
                Some(REAPER_ID),
                &system_root(),
            )
        };
        for trash in [
            TrashFolderFacts {
                is_dir: false,
                is_symlink: true,
                ..my_trash()
            },
            TrashFolderFacts {
                is_dir: false,
                ..my_trash()
            },
            TrashFolderFacts {
                owned_by_me: false,
                owner_uid: 0,
                ..my_trash()
            },
        ] {
            assert_eq!(
                by_root(Ok(trash.clone())),
                TreeRoute::ByHand(ByHandReason::NoTrashFolder),
                "{trash:?}"
            );
        }
        assert_eq!(
            by_root(Err(io::Error::from(io::ErrorKind::NotFound))),
            TreeRoute::ByHand(ByHandReason::NoTrashFolder)
        );
        assert!(matches!(
            by_root(Err(io::Error::from(io::ErrorKind::PermissionDenied))),
            TreeRoute::ByHand(ByHandReason::CouldNotCheck { .. })
        ));
        assert_eq!(
            by_root(Ok(TrashFolderFacts {
                dev: DISK + 1,
                ..my_trash()
            })),
            TreeRoute::ByHand(ByHandReason::NotOnTrashVolume)
        );
        // An app the user may move needs nothing from the Trash folder:
        // macOS picks the Trash for its volume.
        assert_eq!(
            tree_route(
                &app_in_applications(),
                &Ok(owned_by_me()),
                &Err(io::Error::from(io::ErrorKind::NotFound)),
                Some(REAPER_ID),
                &system_root()
            ),
            TreeRoute::Trash
        );
    }

    #[test]
    fn the_stand_ins_answer_as_told() {
        let app = app_in_applications();
        let fake = FakeMacosObjects::new()
            .with_object(&app, owned_by_me())
            .with_unreadable_object("/Applications/Locked.app", io::ErrorKind::PermissionDenied)
            .with_trash(my_trash());
        assert_eq!(fake.facts(&app).unwrap(), owned_by_me());
        assert_eq!(
            fake.facts(Path::new("/Applications/Locked.app"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            fake.facts(Path::new("/Applications/Other.app"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(fake.user_trash_folder_facts().unwrap(), my_trash());
        assert_eq!(
            FakeMacosObjects::new()
                .user_trash_folder_facts()
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            FakeMacosObjects::new()
                .with_unreadable_trash(io::ErrorKind::PermissionDenied)
                .user_trash_folder_facts()
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            NoMacosObjects.facts(&app).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            NoMacosObjects.user_trash_folder_facts().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }

    /// The real probe is `rabbit-platform`'s, which reads only on macOS
    /// and answers `Unsupported` elsewhere. Both only read.
    #[test]
    fn the_real_probe_asks_rabbit_platform() {
        use super::SystemMacosObjects;

        let dir = tempfile::tempdir().unwrap();
        let kind = |result: io::Result<ObjectFacts>| result.map_err(|error| error.kind());
        assert_eq!(
            kind(SystemMacosObjects.facts(dir.path())),
            kind(rabbit_platform::object_facts(dir.path()))
        );
        assert_eq!(
            SystemMacosObjects
                .user_trash_folder_facts()
                .map_err(|error| error.kind()),
            rabbit_platform::user_trash_folder_facts().map_err(|error| error.kind())
        );
    }
}
