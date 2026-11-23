# Changelog

All notable changes to RABBIT are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## How to update

Add entries under `## [Unreleased]` as part of each change. Use these
subsection headings (omit any that are empty):

- `### Added` — new features
- `### Changed` — changes to existing behavior
- `### Deprecated` — soon-to-be-removed features
- `### Removed` — now-removed features
- `### Fixed` — bug fixes
- `### Security` — security fixes

Inside each of them, put every entry under one of these, in this order
(omit any that are empty):

- `#### All platforms` — on both Windows and macOS
- `#### Windows`
- `#### macOS`
- `#### Command line` — the `rabbit` command and its JSON and report
  output, for people who script RABBIT

A release may start with a short untitled overview right under its
version heading: two to four plain sentences on what it brings, in the
order a user meets it.

Write each entry for the people who read it, mostly with a screen reader,
on GitHub and in RABBIT's update prompt: one bullet of one to three short
lines, in plain words, saying what changed for them and the one thing
they must know. Leave out internal names such as functions and types;
flags, exit codes and JSON fields go under `#### Command line`. Order
entries the way a user meets them, and mention a tab, group or button
only after the overview or an earlier entry introduced it.

To cut a release, run the **prepare-release** workflow from the GitHub
Actions tab (workflow_dispatch) with the version `X.Y.Z` as input. It will:

1. Validate that `[Unreleased]` is non-empty and the tag is unused.
2. Bump the workspace `version` in `Cargo.toml` (and refresh `Cargo.lock`).
3. Insert `## [X.Y.Z] - YYYY-MM-DD` below `## [Unreleased]`, leaving a
   fresh empty `## [Unreleased]` on top.
4. Commit, tag `vX.Y.Z`, push both, and dispatch the release workflow.

The release workflow then extracts the section matching the tag's version
from this file and posts it as the GitHub release body.

## [Unreleased]

The **Packages** page now has three tabs, **Install or update**,
**Uninstall** and **Backups**; Ctrl+Tab moves between them. The
**Uninstall** tab removes packages, keeping most files in a backup; the
**Backups** tab restores what RABBIT kept, such as an uninstalled package,
an earlier version or your old key map, or deletes it. Also new: Lo-lo's
instruments and LReaParams on Windows, and a step adding ReaWarp's ReaPack
repository.

### Added

#### All platforms

- The page opens with focus on the tabs, which read "Install or update,
  tab, 1 of 3" (on a Mac only with keyboard navigation on). Tab reaches
  the package list, and Shift+Tab goes back to the tabs.
- The **Uninstall** tab lists what is installed in the chosen REAPER,
  with nothing ticked. It can remove OSARA, SWS, ReaPack, ReaKontrol, the
  language packs, Surge XT and REAPER; a row it can't tick says why.
- Removed files go into a backup, with exceptions under Windows and macOS.
  Settings stay unless you tick **Also remove settings**; your key map,
  OSARA's key map backups and OSARA's settings always stay.
- Removing the language pack REAPER uses sets REAPER back to English.
- Review says what each package loses and keeps. Enter never starts the
  uninstall: press the **Uninstall** button, with REAPER closed. Closing
  RABBIT midway asks first, then stops after the step it is on.
- The Done page says what went and why anything stayed; **Show details**
  lists the files. To get a package back, tick it on the **Install or
  update** tab, or restore it on the **Backups** tab.
- The **Backups** tab lists what RABBIT kept in four groups: **Software
  you uninstalled**, **Earlier versions**, **Key maps** and **Other
  backups**. The text below the list says if a backup is the only copy.
- Below the list, choose **Restore** or **Delete** for the ticked
  backups; both go through Review, and Enter never starts either.
- A restore brings an uninstalled package back as it was, takes a
  package back to before an update, or puts your old key map back,
  keeping the scripts ReaPack added since. Rows it can't restore say why.
- Updates of REAPER, OSARA, Surge XT, the JAWS scripts and, on Windows,
  SWS keep no earlier version to go back to.
- A restore is whole or not at all, and needs the same programs closed as
  uninstalling the package, such as every REAPER for the JAWS scripts.
  Restoring an earlier version or a key map makes a backup that undoes it.
- Choosing **Delete** removes the ticked backups for good, after asking
  with **No** selected; REAPER may stay open. Nothing is deleted by itself.
- Review names rows that must go together. What a deletion can't remove
  stays and is named; deleting it again finishes the job.
- A new **Configuration** step, ticked with ReaPack, adds the repository
  of **ReaWarp**, Gabriel Schuck's accessible scripts for warping, tempo
  and quantizing; install them from Extensions, ReaPack, Browse Packages.
- RABBIT's interface is now available in Brazilian Portuguese, picked on
  first launch for a Portuguese system.

#### Windows

- Switch tabs with Ctrl+Tab or Ctrl+Page Down, and back with
  Ctrl+Shift+Tab or Ctrl+Page Up.
- The **Uninstall** tab can also remove FFmpeg, app2clap, LReaParams,
  Lo-lo's instruments and the JAWS scripts. OSARA also leaves Windows'
  list of installed programs, unless that entry is another REAPER's.
- A standard REAPER and Surge XT go for the whole computer, by their own
  uninstallers, and no backup brings them back. Your Surge XT patches and
  REAPER's settings, extensions and projects stay. A portable REAPER stays.
- The JAWS scripts can be uninstalled from a standard REAPER. Script
  files you changed, or that RABBIT didn't see their installer add, as
  with installs before this version, stay; RABBIT says how to remove them.
- Surge XT, REAPER and the JAWS scripts' uninstaller folder and registry
  entry need Windows' permission, once per run, before anything goes;
  without it, nothing does. No restore brings that folder or entry back.
- Close every REAPER, Surge XT and other music programs first. A removal
  that needs a restart says so. If an uninstaller takes long, RABBIT says
  it may be asking a question in a window of its own, which Alt+Tab finds.
- Lo-lo's VST3 instruments, made to be played with a screen reader, are
  new and unticked under **Instruments** (formerly Additional software):
  **LJuno-116**, **LB-303**, **LR-608** and **LSampler-24**; 64-bit REAPER.
- RABBIT installs them into REAPER's own `UserPlugins\FX`, also for a
  portable REAPER, and REAPER finds them after a restart or plug-in rescan.
  Uninstalling keeps your presets and samples. Thanks to Lo-lo for asking.
- **LReaParams**, Lo-lo's extension to control FX and track parameters
  from the keyboard, unticked in the new **Tools** group; 64-bit REAPER
  only. A copy ReaPack manages is left to ReaPack.

#### macOS

- Without keyboard navigation, the page opens in the package list. Switch
  tabs with Ctrl+Tab, the arrow keys on the tabs or VoiceOver's tab
  navigation; VoiceOver mentions Ctrl+Tab once.
- Uninstalling REAPER moves its app to the Trash; your settings,
  extensions and projects stay. If only an administrator may move it, a
  password moves it and Finder offers no Put Back; a Locked app can't go.
- Uninstalling Surge XT from a standard REAPER deletes it for good, for
  every music program, with an administrator's password; your patches
  and settings stay.
- Close Surge XT, Surge XT Effects, `surge-xt-cli` and every REAPER first.
- RABBIT asks for an administrator's password at most once per run,
  before anything goes; the window may name osascript or offer Touch ID.
  Cancelling it uninstalls nothing.
- Closing RABBIT while that window is open, or after it, lets the one
  administrator command finish, even when it removes several packages.
- If macOS refuses to change an app, turn RABBIT on under System
  Settings > Privacy & Security > App Management.

#### Command line

- `rabbit uninstall --package <id>` removes what the **Uninstall** tab
  does, only with `--apply`, and changes nothing if a named package can't
  go; `--remove-settings` takes settings too. REAPER must be closed.
- Exit codes: 0, all named are gone or weren't installed, also when a
  restart finishes it; 2, one can't go and nothing changed; 1, a removal
  failed or was stopped, or a prompt was declined; earlier removals stay.
- On a Mac, `rabbit uninstall --apply` won't run as root, and a first
  Ctrl+C lets the administrator command finish.
- `rabbit components` says whether each package can be uninstalled.
- `rabbit restore-backup --package <id>` restores one package, as it was
  uninstalled or from before an update, and `--key-map` only the key map.
- For a whole update's backup, `--json` lists each package under `swaps`;
  `status` is `partly-restored` when only some went back.
- `rabbit delete-backup --backup-id <id>` deletes a set for good, or with
  `--package` or `--key-map` one backup in it, only with `--apply`.
- Exit codes: 0, all gone, already gone included; 2, refused and nothing
  changed; 1, a deletion failed, was stopped or an error came first.
- An install's `--json` and report give each file an update dropped the
  `action` `moved-to-backup` (`would-move-to-backup` in a dry run) or
  `left-in-place` with a `left_reason`, and add `reaper_language`.
- `rabbit restore-backup --json` adds `langpack` and `variant` to each
  swap. With `--allow-reaper-running`, both keep REAPER's language and
  name the file to pick.
- ReaWarp's step is `--config-step reapack-add-reawarp-remote`.

### Changed

#### All platforms

- The **Packages** page's heading reads "Choose what RABBIT should do".
- The **Additional software** group is now **Instruments**, also on the
  **Uninstall** tab. On Windows, app2clap and LReaParams are in **Tools**.
- A package you uninstalled is no longer ticked again by itself while it
  is gone; ticking or restoring it ends that. Language packs you removed
  or left unticked stay unticked, as before, even while installed.
- Downloads now trust the system's certificate store, so they work with
  antivirus HTTPS scanning; a revoked certificate is refused. If revocation
  checks are blocked, a first connection can take up to 10 seconds longer.
- Updates RABBIT installs file by file now move files the new version no
  longer has into the backup instead of deleting them. One that can't be
  moved stays, so that version can't be restored; **Show details** says so.
- While REAPER uses the Spanish language pack, switching it between es_ES
  and es_MX, or restoring the other one, switches REAPER's language too.
- While RABBIT's folder or its backups folder is a link or junction,
  RABBIT changes nothing, lists no backups and says why; move the backups
  back into a real folder. Dry runs and `rabbit preflight` report it.
- Another RABBIT busy with the same REAPER is now reported in plain words.

#### Windows

- app2clap now installs into REAPER's own `UserPlugins\FX`, also for a
  portable REAPER; other CLAP hosts no longer load it.
- Update or reinstall app2clap in the standard REAPER before a portable
  one: its old copy in the CLAP folder, even one you put there, moves to
  the backup. Until then, uninstalling it needs every REAPER closed.

#### macOS

- VoiceOver names text fields, dropdowns and the package list by their
  label, and opens context menus with VO+Shift+M.
- Only a RABBIT whose program file is named `rabbit` (any case), alone or
  followed by a dash, space or opening parenthesis, can install or
  uninstall Surge XT or uninstall REAPER; renaming the app is fine.

#### Command line

- `rabbit setup` and `apply-packages` without `--package` skip a package
  you uninstalled, while it is gone, and say so; naming it ends that.
- `rabbit backups` lists sets newest first, what each holds and how to
  restore it. `--json` adds `kind`, `created_unix`, `size`, `items`,
  `fingerprint`, `swaps_whole`; kind `deleting` is an unfinished deletion.
- `rabbit restore-backup` no longer copies RABBIT's records back, refuses
  a backup holding a copy of them, and needs the same programs closed as
  the package's uninstall.
- A restore is all or nothing and exits with 1 when it fails (`status`
  `files-left` if a file couldn't be put back). With `--apply` it saves a
  report even then, naming the backup of the files it replaced.
- Restoring a key map now keeps the scripts ReaPack registered since;
  before, they dropped out until "ReaPack: Synchronize packages".
- `rabbit restore-backup` refuses `--allow-reaper-running` when it puts a
  key map back. On a Mac, its `--apply` no longer runs as root, and another
  account's backup isn't restored, not even in a dry run.
- `rabbit packages --json` and `--manifest --json` give `category` as
  `instruments` or `tools`, not `additional`.

### Fixed

#### All platforms

- Backups now note each file before copying it, so even one from an
  interrupted run can be restored.
- When RABBIT can't back up your key map before installing OSARA's, it
  leaves yours alone and now says only that OSARA was installed.
- In a portable REAPER, a package that needs a standard install, such as
  Surge XT, keeps saying so when its version check fails.
- The French language pack installs and updates again: RABBIT found no
  version once its description stopped ending in "par ReaperAccessible".
  (Fixed by math65 in #36.)
- The text below the package list now follows a package's tick: unticked,
  it no longer still says "Will install". (Fixed by math65 in #35.)

#### Windows

- The **Language packs** heading's checkbox updates as you tick a pack.
- A package with no version for your REAPER, such as FFmpeg for a 32-bit
  REAPER, can't be ticked and says why, instead of stopping the install.
- An installed REAPER on Windows on ARM is recognised as the ARM64EC
  program it is, so its updates keep their ARM64EC builds instead of x64.
- On Windows on ARM, FFmpeg was an ARM64 build REAPER there can't load;
  it is now the x64 one. The next FFmpeg update replaces and backs it up.
- On Windows on ARM, ticking SWS or app2clap no longer stops a new
  REAPER's install; they install their x64 versions, which REAPER loads.
- RABBIT no longer misses some running copies of REAPER when it checks
  that REAPER is closed.
- Surge XT's installer no longer closes other programs, such as your
  screen reader, or fails with "exit code Some(5)"; RABBIT asks you to
  close Surge XT first. Installer errors now say what to do.
- A stopped Surge XT update is no longer reported as installed, and its
  installer's log is in `RABBIT\logs`.
- Installing OSARA into a portable REAPER no longer breaks the standard
  OSARA's entry in Windows' list of installed programs. One an earlier
  RABBIT broke is fixed the next time you install or update OSARA there.

#### macOS

- VoiceOver now reads the names of the package list's group rows.
- Tab no longer stops on a wizard page's scroll bar.
- A Surge XT password window that fails now says nothing was installed.

#### Command line

- `rabbit restore-backup` puts each file back where it came from, not
  all into the REAPER resource folder.
- `rabbit backups` no longer fails when one backup is damaged.
- `rabbit install-extension --apply` no longer runs while another RABBIT
  works on the same REAPER.
- `rabbit apply-packages` checks that REAPER is closed before running any
  installer, not only before the first extension file;
  `--allow-reaper-running` still overrides it.
- `rabbit setup`, `apply-packages` and `plan` skip a package with no
  version for the target REAPER; naming it exits with 2. `setup`,
  `apply-packages` and `install-extension` now go by its architecture.

### Security

#### All platforms

- A restore never follows a link or junction in RABBIT's backups, never
  writes through one leading out of REAPER's resource folder, and on
  Windows never by a device name such as `CON`.

#### Command line

- `rabbit restore-backup` no longer takes a backup id that leads out of
  RABBIT's backups folder, such as `D:` on Windows. The id must match
  exactly, case included.

## [0.4.4] - 2026-10-13

### Changed

#### All platforms

- While an install runs, the **Close** button reads **Cancel**. It asks
  first, then stops after the step it is on and closes RABBIT.
- The **Replace your current key map with latest OSARA key map** box now
  starts unticked, keeping your keys, if you changed your key map or use
  REAPER's default keys; scripts ReaPack added don't count as changes.
- The note below it and the Review page say which applies. For a changed
  key map, the note and the Done page point to **Merge OSARA key map**
  (Extensions menu, OSARA), which adds OSARA's new keys to yours.

#### Command line

- `rabbit setup` and `apply-packages` no longer always replace the key
  map: without `--preserve-osara-keymap` or the new
  `--replace-osara-keymap`, they choose as the wizard does and say why.

### Fixed

#### All platforms

- Replacing the key map no longer adds one more copy of OSARA's own
  script lines to `reaper-kb.ini` each time.

## [0.4.3] - 2026-10-06

A hidden expert mode installs development builds of REAPER and test
builds of OSARA pull requests, for people who test pre-release software.
Closing RABBIT during an install now stops it cleanly.

### Added

#### All platforms

- Turn expert mode on with **Ctrl+Shift+E** (**Cmd+Shift+E** on a Mac)
  on the first page and confirm the warning, or start RABBIT with
  `RABBIT_EXPERT=1`. It is never saved; the window title shows it is on.
- Its **REAPER builds** choice adds landoleet.org's development builds,
  and **OSARA builds** the test build of an open pull request, picked from
  a list. A package's row says when it comes from such a build.
- Packages from such builds go back to their regular release on the next
  install without expert mode, or once an OSARA pull request is merged or
  closed or its build is gone, but never over a network failure.
- The README now explains `GITHUB_TOKEN`: set to a personal access token,
  it raises GitHub's limit from 60 to 5,000 requests an hour.

#### macOS

- VoiceOver now announces a dropdown appearing or going away below the
  package list, each package and step as the install reaches it, and why
  a version check failed.

#### Command line

- `--package-channel` gets the same builds without expert mode, such as
  `reaper=dev` or `osara=pr:1454`; `<package>=stable` goes back. A package
  keeps the channel it was last installed from until told otherwise.
- It works with `latest`, `artifacts`, `download`, `plan`,
  `install-extension`, `apply-packages` and `setup`, and `rabbit packages`
  lists each package's channels.
- A pull request that is merged or closed, or has no build left, gets a
  warning and the regular snapshot. Switching a package's channel
  reinstalls it, even at the same version.

### Changed

#### All platforms

- The **REAPER language after installation** and **Spanish OSARA
  translation** dropdowns now hide while they have nothing to decide.
- Closing RABBIT during an install now asks first, with **No** focused,
  then finishes the step it is on and closes. Its report lists what got
  installed and what RABBIT never reached, and counts neither as failed.
- While RABBIT updates itself, closing it is refused; it says why and
  finishes the update.

#### Command line

- `rabbit install-extension` no longer downloads anything without
  `--apply`; it shows what it would download and install.

### Fixed

#### All platforms

- A portable REAPER is no longer reinstalled on every run once opened,
  and one that RABBIT didn't install is now recognised too.
- An OSARA test build that RABBIT has no record of now shows its own
  version, not one from 2024.
- The package list no longer vanishes, even for screen readers, when the
  window isn't maximized; the wizard's pages now scroll instead.
- The **Set REAPER's language** step now asks for any language pack, not
  one particular one. (Reported by Scott Chesworth from Q&A.)
- **Enter** on the **Review** page now starts the install.
- A failed install now says why, read out right away, with an open REAPER
  named in plain words, and **Install** catches an open REAPER before
  anything starts. (Reported by acerbt in #32.)

#### Windows

- No console windows flash up any more, such as when RABBIT adds its
  Defender exclusion or opens a link; installers still show their own.

#### macOS

- **Space** now ticks and unticks rows in the package list, and VoiceOver
  says if a row is checked
  ([#28](https://github.com/Timtam/rabbit/issues/28)).

### Security

#### All platforms

- Programs RABBIT starts no longer get the `GITHUB_TOKEN` you set for it:
  installers that run without admin rights, OSARA test builds, the macOS
  admin shell and REAPER. RABBIT only sends it to GitHub's API.

## [0.4.2] - 2026-08-18

### Fixed

#### Windows

- Installing into a folder with a space in its path, such as a portable
  REAPER in `REAPER Portable`, no longer fails with "post-install
  verification failed" and takes OSARA, SWS and FFmpeg down with it.

## [0.4.1] - 2026-08-18

### Added

#### All platforms

- The "RABBIT update available" prompt now shows the release notes of
  every version since yours, when it can fetch them; Enter updates,
  Escape declines.
- Updating RABBIT opens a progress window with its steps, a bar and a log;
  it has no Cancel, ignores Escape and closes when the new RABBIT starts.
- A French REAPER language pack, also for SWS, by Lee Julien and
  Pierre-Marie Curt for ReaperAccessible. It is installed as
  `fr_CA.ReaperLangPack`, so OSARA uses its more complete French.

### Changed

#### All platforms

- Release notes, RABBIT's and ReaPack's, no longer read out Markdown
  symbols; a link keeps only its text.
- A language pack you untick stays unticked in that REAPER, its updates
  included, until you tick it again.
- One **Set REAPER's language** step replaces one per language. It is
  greyed out until you tick a language pack, then ticks itself; **REAPER
  language after installation** picks which ticked pack becomes active.

#### Command line

- `rabbit self-update check` prints the release notes too, and
  `rabbit <command> --help` explains every option, such as
  `--accept-reapack-donation-notice`, needed to install ReaPack unattended.
- The language step's id is `set-reaper-language`, no longer
  `set-reaper-language-<language>`; `--reaper-language <package>` picks
  the pack, and an unknown one is refused.
- `rabbit setup` runs it by default only when you ask for a language
  pack; `--config-step set-reaper-language` activates an installed one.

### Fixed

#### All platforms

- Installing a language pack no longer deletes the others; switch between
  them in REAPER. Only the Spanish pack's two OSARA translations replace
  each other.
- When REAPER fails to install, RABBIT reports that, not a language error;
  a missing language pack skips setting the language, with a note.

#### macOS

- Ticking the focused package with Space or a click now updates the
  OSARA key map options and the dropdowns below the list.
- The **Configuration** group's checkbox can be unticked again, and shows
  its state, while one of its steps is already done.
- VoiceOver now reads RABBIT's Spanish interface with a Spanish voice.

## [0.4.0] - 2026-08-12

RABBIT can now install REAPER language packs, so REAPER itself speaks
Spanish or German, and RABBIT's own interface now speaks Spanish.

### Added

#### All platforms

- **REAPER in Spanish**, by Javier Robledo, and **REAPER in German**, by
  MrData, both also translating SWS, under a new **Language packs**
  heading. Suggested by Carlos Esteban Martínez (#19).
- The pack for RABBIT's language comes ticked, others unticked, with a
  step that makes it REAPER's language; untick the step for only the file.
- Installing a pack replaces the one RABBIT installed before; one you put
  in `LangPack` yourself stays. A pack's new versions come as updates.
- For Spanish, **Spanish OSARA translation** picks *REAPER Accesible
  español* (es_ES), the default, or *Team PMA* (es_MX). Updates keep your
  choice, and switching removes the old file.
- RABBIT's interface is now in Spanish, contributed by Carlos Esteban
  Martínez (#19), picked on any Spanish system or in the language picker.
- The package details now say what's new in REAPER, SWS, ReaPack and
  OSARA, with OSARA's list trimmed for listening. (#4)

#### Windows

- RABBIT asks to exclude only its download folder, `%TEMP%\rabbit-cache`,
  from Microsoft Defender, so OSARA's snapshots aren't quarantined; once
  added it stops asking, and saying no doesn't stop the install.

#### Command line

- `--package-variant langpack-es=pma` picks Team PMA's OSARA translation.
- The `rabbit` command never adds the Defender exclusion, nor asks to.

### Changed

#### All platforms

- The version check now checks several packages at once, so it's faster,
  and the status line counts them, as in "Checking versions… (3 of 11)".
- A failed package no longer stops the rest; the result page says what
  failed. If REAPER fails to install, its extensions such as OSARA and SWS
  are skipped, while Surge XT and app2clap still install.

#### macOS

- Starting with the update after this one, RABBIT replaces the whole
  `Rabbit.app`, keeping its name, Apple's signature and permissions such
  as Full Disk Access. The old one stays as `Rabbit.app.rabbit-old`.

#### Command line

- Commands go on past a failed package too, and still exit non-zero.

### Fixed

#### All platforms

- Enter on the **Done** page now presses **Close**, also in its text boxes.

#### Windows

- When antivirus blocks a package, often a false alarm on OSARA's signed
  snapshots, RABBIT now says how to allow it, as does the README, instead
  of "os error 225". RABBIT never turns your virus protection off.

#### macOS

- Cmd+Q and the menu's Quit item now quit RABBIT.
- RABBIT no longer refuses to start after updating itself ("RABBIT could
  not be opened"). If yours does, run this in Terminal once, or download
  it again: `xattr -dr com.apple.quarantine /path/to/Rabbit.app`

## [0.3.2] - 2026-07-03

### Added

#### All platforms

- After a crash, RABBIT saves a short text report, `crash-<timestamp>.log`,
  next to itself (next to `Rabbit.app` on a Mac), or in the temp folder as
  `rabbit-crash-<timestamp>.log`. Please attach it when you report a crash.

### Fixed

#### macOS

- RABBIT no longer crashes when it moves to the packages page after the
  version check, a bug new in 0.3.1.

## [0.3.1] - 2026-07-03

### Added

#### All platforms

- With ReaPack, two new steps add the REAPER Accessible team's
  repositories: **REAPER Accessible (EN)**, ticked, and **REAPER
  Accessible (FR)**, for French resources, unticked.

### Changed

#### All platforms

- Installs are faster: up to three downloads run at once, and each package
  installs as soon as it's downloaded, still one at a time, in order. The
  status line says, for example, "Downloading 3 packages…".

### Fixed

#### All platforms

- After the version check, focus starts on the "Packages" group at the
  top of the package list; coming Back to the page keeps your place.
- Large downloads, RABBIT's own updates too, now wait out stalls of up to
  a minute and retry up to three times, resuming where possible; then the
  error names the address and suggests checking your connection.

## [0.3.0] - 2026-06-30

RABBIT's interface is now in French and Italian. Surge XT moves into a
new **Additional software** group, where Windows also gets jcsteh's
app2clap.

### Added

#### All platforms

- RABBIT's interface is now available in French and Italian, picked on
  first launch for any French or Italian system, such as Canadian French.
- Enter now presses the page's main button, Next or Install, wherever
  focus is; while that button is unavailable, Enter does nothing.

#### Windows

- **app2clap**, jcsteh's CLAP plug-in that brings other programs' audio
  into REAPER and other CLAP hosts, unticked. It installs into your user's
  CLAP folder without administrator rights, and not for a portable REAPER.

### Changed

#### All platforms

- Surge XT moved from **Packages** to **Additional software**, whose
  heading ticks the whole group. The group shows only when something in
  it is available for the chosen REAPER.
- One package's failed online version check, such as with the SWS site
  down, no longer stops the others. That row reads "online version check
  failed" and can't be ticked; Review shows the error.

#### Windows

- Updating REAPER no longer puts back a desktop icon you deleted. A new
  standard install still gets one, and an icon already there stays.

#### Command line

- `rabbit latest` and `plan --online` no longer stop when one package's
  check fails. `latest --json` now prints an object with `packages` and
  `failures` arrays; plain `latest` warns on stderr, `plan` adds a note.

### Fixed

#### Windows

- When RABBIT can't tell where a running REAPER is, as when REAPER runs as
  administrator, it now asks you to close it instead of letting the
  installer overwrite it.
- Setup no longer fails with "stream did not contain valid UTF-8" when
  `reapack.ini` or `reaper-kb.ini` holds a character such as a curly
  apostrophe. Reported in #7.

#### macOS

- If macOS won't let RABBIT replace an extension, RABBIT says so before
  downloading instead of "OS error 1": allow it Full Disk Access or App
  Management in System Settings > Privacy & Security, then restart RABBIT.

## [0.2.1] - 2026-06-10

### Added

#### All platforms

- A portable REAPER in RABBIT's own folder, `reaper.exe` with `reaper.ini`
  on Windows or a `REAPER*.app` on a Mac, is now found and chosen as the
  target, so you can check for updates without browsing for it.

### Changed

#### macOS

- RABBIT is now signed and notarized by Apple: unzip the download and
  double-click `Rabbit.app`. The `Open Me First.command` helper is gone.

### Fixed

#### All platforms

- After a self-update, the old RABBIT window now closes once the new one
  has started, instead of staying open next to it. If the new one fails
  to start, the old one stays.

#### Windows

- Installing or updating a standard REAPER now goes into the chosen
  REAPER's folder instead of `C:\Program Files\REAPER (x64)`, and no longer
  stops the run after REAPER with error 1223. Fixed by trypsynth in #6.

#### macOS

- A self-update no longer leaves RABBIT unable to start, with Finder
  calling its program a document. Reported in #5.

## [0.2.0] - 2026-05-15

### Added

#### All platforms

- **Surge XT**, the free synthesizer from the Surge Synth Team, unticked
  and only for a standard REAPER. Its installer adds VST3, CLAP and the
  standalone app for all music programs, with administrator rights.
- RABBIT installs Surge XT's nightly builds, as the project now mostly
  ships, rather than the older 1.3.4 release.
- The progress page now follows each package: the bar moves during
  downloads, the status names what is downloading and how much has
  arrived, and the details add a line for each finished step.
- Installing OSARA's key map over yours now keeps the scripts ReaPack
  installed in REAPER's actions list, and the keys you bound to them;
  before, they needed "ReaPack: Synchronize packages".

#### macOS

- Surge XT also comes as an Audio Unit. Installing it asks for an
  administrator's password in macOS's own window.

#### Command line

- Surge XT is `--package surge-xt`.

### Changed

#### Command line

- `rabbit components` now names FFmpeg's detector
  `ffmpeg-binary-version-string` instead of `ffmpeg-cli-version`.

### Fixed

#### All platforms

- RABBIT starts much faster with packages installed, and no longer
  stalls again after an install while it checks them.

#### Windows

- With FFmpeg installed, RABBIT no longer freezes for 20 to 30 seconds
  reading its version, and an FFmpeg you got from gyan.dev now shows its
  full version, such as 8.1.1 instead of 8.0.0.

## [0.1.2] - 2026-05-13

### Changed

#### All platforms

- RABBIT now asks at startup whether to update itself, instead of
  offering a button on the Done page. Yes updates and restarts RABBIT;
  No leaves it until you start RABBIT again.

### Fixed

#### Windows

- On Windows on ARM, a first install with no REAPER there yet now gets
  the ARM64EC SWS and ReaPack instead of the x64 ones.

#### macOS

- On Apple Silicon, SWS and ReaPack now match REAPER on a first install
  with no REAPER there yet, and when RABBIT runs under Rosetta, instead
  of Intel versions REAPER couldn't load.
- After RABBIT updates itself, macOS 15 and 26 no longer refuse to open
  `Rabbit.app` as damaged.

## [0.1.1] - 2026-05-10

### Added

#### All platforms

- Links that always download the newest release, as in the README:
  `https://github.com/Timtam/rabbit/releases/latest/download/` plus the
  file name. Versioned files and SHA-256 sums stay on the Releases page.

### Changed

#### macOS

- One universal RABBIT for Apple Silicon and Intel replaces the two
  builds: `rabbit-<version>-macos-universal` and its `.app.zip`. RABBIT
  0.1.0 moves to it at its next self-update, with no new download.

### Fixed

#### macOS

- Cmd+C, and Cmd+S in German, no longer close the wizard; its buttons
  have no letter shortcuts on a Mac any more.
- `Open Me First.command` now checks every file in the app before it
  reports success. On macOS 15 and later it then opens RABBIT once and
  System Settings > Privacy & Security, where you click "Open Anyway".
- With a universal REAPER, installing no longer stops with "no artifact
  found for sws", nor puts ReaPack's Apple Silicon build on an Intel Mac.
  SWS and ReaPack now match the Mac's processor, also under Rosetta.
- VoiceOver reads RABBIT's German interface with a German voice, also on
  an English macOS.

## [0.1.0] - 2026-05-09

Initial public release: one program for Windows (x64, ARM64) and macOS
(Apple Silicon, Intel) that installs and updates REAPER and its
accessibility extensions, through an accessible wizard or a command line.

### Added

#### All platforms

- The wizard is keyboard-first with native controls, tested with NVDA,
  JAWS, Narrator and VoiceOver, in English or German to match your system.
- It finds standard REAPERs and portable ones you choose, and installs or
  updates REAPER, OSARA, SWS, ReaPack and ReaKontrol to match that REAPER.
- A configuration step adds the REAPER Accessibility ReaPack repository
  (`Timtam/reapack`), and is skipped when it is already there.
- Before it installs, RABBIT checks that REAPER is closed and its resource
  folder writable, and asks you to read ReaPack's donation notice.
- Each install keeps a backup in `<resource>/RABBIT/backups/<timestamp>/`
  and a JSON and text report in `<resource>/RABBIT/logs/`; RABBIT leaves
  nothing else in REAPER's resource folder.
- RABBIT checks for its own updates on startup, and the Done page offers
  to apply one, checked by its SHA-256 sum.

#### Windows

- When JAWS is installed, RABBIT can also install the JAWS-for-REAPER
  scripts. FFmpeg 8.x for REAPER's video is unticked; it comes from
  gyan.dev on x64 and from tordona/ffmpeg-win-arm64 on ARM64.

#### macOS

- The `.app.zip` holds `Open Me First.command`, which clears Gatekeeper's
  quarantine on first launch; self-updates don't need it again.

#### Command line

- Given arguments, RABBIT runs as a command. Installs, updates and
  restores are dry runs without `--apply`; every command takes `--json`.
- `rabbit preflight` exits with 2 when a check fails. Installing ReaPack
  needs `--accept-reapack-donation-notice`.
- `rabbit backups` lists backups and `rabbit restore-backup` rolls one
  back; `rabbit self-update check`, `stage` and `apply [--restart]`
  update RABBIT.
- The repository step is
  `--config-step reapack-add-reaper-accessibility-remote`.
