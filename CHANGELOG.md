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

### Fixed

- The **Set REAPER's language** row no longer claims to require one
  particular language pack. It listed every pack it accepts and then named
  whichever sorted first, so a step that any pack satisfies advertised
  itself as "requires REAPER in Spanish". It now says "requires a language
  pack" in all five interface languages. Steps with a single dependency —
  the ReaPack ones — still name it, since there it is the truth.
  (Reported by Scott Chesworth from Q&A.)

- The package list on the **Packages** page could go missing entirely
  unless the window was maximized. The page had grown — the OSARA key-map
  note, then the Spanish-variant and REAPER-language dropdowns — until the
  controls stacked below the list no longer fitted the default window, and
  the layout took the whole shortfall out of the only control allowed to
  stretch: the list itself. It collapsed to nothing, and a control of zero
  height is not exposed to screen readers at all, so VoiceOver, NVDA,
  JAWS and Narrator alike found no list to read. The wizard pages now
  scroll instead of clipping what does not fit, so every control keeps its
  size whatever the window size and the language of the interface.

## [0.4.2] - 2026-08-18

### Fixed

- Installing into a folder whose path contains a space — a portable REAPER
  on the Desktop under `REAPER Portable`, say — failed with
  `post-install verification failed; missing paths: [...reaper.exe]`, and
  took OSARA, SWS and FFmpeg down with it as dependents of a REAPER that
  never landed. REAPER, OSARA and SWS all ship NSIS installers, and NSIS
  reads its `/D=<path>` destination straight out of the raw command line
  rather than from parsed arguments, so it needs that argument last and
  unquoted. Rust quotes any argument containing a space when it builds a
  Windows command line, so NSIS was handed `"/D=C:\...\REAPER Portable"`
  and took the quote as part of the directory name. RABBIT already got this
  right for installs that need administrator rights; a portable install into
  a folder you own doesn't, and that path had been quoting it all along.

## [0.4.1] - 2026-08-18

### Added

- Accepting a RABBIT update now opens a progress window instead of leaving
  you watching a status line: it names the phase ("Downloading RABBIT…
  4.1 MB of 10.2 MB"), fills a progress bar as the bytes arrive, and keeps
  a running log of each step as it completes, so a screen reader can be
  walked back through what happened. The bar covers the download, the
  checksum verification and the install, and the window closes on its own
  when RABBIT relaunches into the new version. It has no Cancel button on
  purpose — once the file swap starts there is nothing to safely undo — and
  it can't be dismissed by mistake with Escape.

- The "RABBIT update available" prompt now answers "what would I be
  updating to?" before you decide: alongside the question it shows the
  release notes for the update, in a read-only box you can read through
  and scroll like the package details pane. The notes cover every release
  published since the version you are running, not just the newest one, so
  skipping a couple of updates still tells the whole story. Update now is
  the default button, so Enter accepts from anywhere in the dialog and
  Escape declines. When the notes can't be fetched — no network, GitHub
  down — the prompt falls back to the short question it has always asked,
  since nothing about the notes should stand between you and an update.
  `rabbit self-update check` prints the same notes. Release notes are also
  stripped of their Markdown wherever they are shown, RABBIT's own and
  ReaPack's alike — headings, bold, inline code and links now read as the
  words they contain rather than as strings of asterisks, hashes and
  backticks spoken out loud. Underscores are left alone, since in these
  notes they are far more often part of a name you need to hear exactly
  (`depends_on`) than emphasis, and a link's address is dropped while the
  text you would have clicked is kept.

- A **French REAPER language pack** joins the Spanish and German ones, so
  REAPER and the SWS extension speak French too. It is maintained by Lee
  Julien and Pierre-Marie Curt for ReaperAccessible, the French
  screen-reader community. RABBIT installs it as `fr_CA.ReaperLangPack`,
  the name that makes OSARA load its own French translation — the `fr_CA`
  one, which is all but complete where OSARA's `fr_FR` translation still
  leaves about a third of its messages in English. Unlike the other two
  packs, this one is published with a version number, which RABBIT reads
  from ReaperAccessible's ReaPack index and shows in the wizard.

### Changed

- **A language pack you turn down stays turned down.** RABBIT ticks the pack
  matching its own language for you, which is right for most people and was
  wrong for anyone who doesn't want it: unticking it lasted exactly one run,
  and the pack came back ticked on the next launch. That refusal is now
  remembered in the install receipt, so the pack stays listed and tickable
  but is not ticked for you again — including its updates, so a pack you
  installed and later stopped using stops re-offering itself every time the
  translators publish a fix. A pack that is merely installed and up to date
  is not a refusal: it sits unticked because there is nothing to do, and
  RABBIT reads that as silence rather than a decision. Tick it yourself and
  the refusal is forgotten, so a change of mind sticks too. The memory is
  per REAPER install, so you can say no on one and yes on another, and
  packages opt
  into this through a new `remember_opt_out` manifest flag — only the
  language packs set it, because they are the ones RABBIT turns on by
  guessing. Receipts stay readable by older RABBITs, which simply ignore the
  new field.

- The wizard no longer grows a "Set REAPER's language to X" configuration
  step for every language it supports. There is one **Set REAPER's
  language** step, and which pack it activates comes from a **REAPER
  language after installation** dropdown listing the packs you ticked — one
  decision instead of a row per language, and two translated strings rather
  than two per language. On the command line the step id is now
  `set-reaper-language` (it was `set-reaper-language-<language>`), and
  `--reaper-language <package>` picks which pack to activate. An unknown
  package there is now rejected with the list of language packs rather than
  quietly leaving REAPER in English.

- **Set REAPER's language** is now tied to the language packs you tick.
  With none ticked the step is greyed out rather than tickable — the
  "REAPER language after installation" dropdown lists exactly the packs you
  ticked, so an unticked list left the step pointing at nothing. Tick a
  pack and the step ticks itself, ready to be unticked if you want the
  files without the switch; untick the last pack and the step clears
  itself. A pack merely sitting on disk from an earlier run doesn't arm it:
  someone who keeps a German pack installed but runs REAPER in English
  chose English, and a run that installs nothing language-related must not
  switch the interface out from under them. `rabbit setup` follows the same
  rule for its defaults, while `--config-step set-reaper-language` still
  activates an already-installed pack for people who want exactly that.

- `RABBIT <command> --help` now explains every flag. 47 of the 50 printed
  nothing but their own name, which made RABBIT's own advice — "run
  `--help` for anything not covered here" — worth very little.
  `--accept-reapack-donation-notice` was the sharp edge: ReaPack cannot be
  installed unattended without it, and neither the help nor the README said
  so, which simply reads as a broken install. The README also gains that
  example, a Spanish-variant one, and a short tour of the flags most likely
  to be wanted.

### Fixed

- On macOS and Linux, ticking a package on the row that already had focus
  (Space, or clicking its checkbox) left every control below the package
  list stale: the OSARA keymap note, the Spanish translation picker and the
  new REAPER-language dropdown only refreshed when the *selection* changed.
  Arrow onto a row and press Space and nothing below the list caught up.
  Windows was unaffected — it refreshes them on every toggle route.
- On macOS and Linux, the **Configuration** group checkbox could be ticked
  but never unticked, and always drew as unchecked, as soon as one of its
  steps was already applied at the target — which is the normal case, since
  an existing ReaPack remote counts as applied. It counted already-applied
  steps as unticked members of the group, so "all ticked" was never true.

- Installing a language pack no longer deletes the other ones. RABBIT
  treated language packs as mutually exclusive and removed the previously
  installed pack, which was simply wrong: REAPER keeps every pack in its
  `LangPack` folder and only *activates* one, so a multilingual user can
  install German and Spanish and switch between them in REAPER whenever
  they like. Packs now sit side by side; the only file RABBIT still
  replaces is the one it installed for that same pack, such as when you
  switch between the two Spanish translations.

- A failed REAPER install no longer reports itself as a confusing language
  error. If REAPER can't be installed, the language pack that depends on it
  is correctly skipped — but the step that sets REAPER's language ran
  anyway and failed the whole run with "cannot set REAPER's language: no
  installed language-pack file is recorded for langpack-de", hiding the
  real problem. Configuration steps now look at what actually landed on
  disk rather than what was planned, so the genuine failure is what you
  see; and if a language pack is missing for any other reason, setting the
  language is skipped with an explanation instead of aborting the install.

- macOS: the app bundle now declares Spanish, so VoiceOver reads RABBIT's
  Spanish interface with a Spanish voice instead of an English one. RABBIT
  gained its Spanish translation in 0.4.0, but the bundle still advertised
  only English, German, French and Italian — and macOS decides what language
  an app speaks from that declaration.

## [0.4.0] - 2026-08-12

### Added

- RABBIT can now install **REAPER language packs**, so REAPER itself speaks
  your language rather than only RABBIT's wizard. Two are available to
  start with, both of which also translate the SWS extension: Spanish,
  maintained by Javier Robledo for the Spanish REAPER community, and
  German, maintained by MrData in the REAPER Stash. The pack matching the
  language RABBIT is running in is suggested and ticked for you (a Mexican
  Spanish system gets the Spanish pack); packs for other languages are
  listed but left unticked, and nothing is offered in English because
  REAPER is already English. By default RABBIT also sets the pack as
  REAPER's language, so you don't have to find it under Options,
  Preferences, General — untick that step if you only want the file. Only
  one language pack can be active, so installing one removes the previous
  one RABBIT installed; a pack you put in the `LangPack` folder yourself is
  never touched. They appear under their own **Language packs** heading in
  the package list. Language packs are published without version numbers,
  so RABBIT identifies them by what the server reports about the file (its
  ETag, or last-modified date and size, falling back to the file's
  contents) and offers an update when the translator publishes a new one —
  no RABBIT release needed. Spanish additionally offers a choice of OSARA
  translation — *REAPER Accesible español* (the default) or *Team PMA* —
  which OSARA picks up from the pack's file name. Choose it from the
  **Spanish OSARA translation** dropdown on the packages page (or
  `--package-variant langpack-es=pma` on the command line); your choice is
  remembered, so a later update keeps the translation you picked instead of
  reverting to the default, and switching cleans up the old file. Suggested
  by Carlos Esteban Martínez (#19).
- RABBIT now speaks Spanish. The translation was contributed by Carlos
  Esteban Martínez (#19) and is picked automatically on Spanish systems —
  including regional variants, so an es-MX machine gets the Spanish UI —
  or from the language picker at any time.
- Windows: the wizard now adds a Microsoft Defender exclusion for RABBIT's
  own download cache folder (`%TEMP%\rabbit-cache`) so freshly built,
  low-prevalence — but code-signed — installers, chiefly OSARA's
  development snapshots, aren't quarantined as false positives mid-install.
  It's scoped to that one folder (never all of `%TEMP%`), added once under a
  single administrator prompt, and skipped without prompting on later runs
  once present. RABBIT never turns real-time protection off, and if the
  prompt is declined or blocked by policy/Tamper Protection the install
  proceeds anyway (a real block still shows the "allow it in Protection
  history" guidance). The command-line tool is left untouched so it stays
  non-interactive for unattended/CI use.
- The packages page's details pane now answers "what's new?": REAPER and
  SWS show the newest section of their official changelogs, ReaPack shows
  its release notes, and OSARA shows the latest snapshots' list of changes
  — the feed OSARA's own updater reads, lightly cleaned for listening
  rather than reading: developer-facing "dev:" commits are skipped,
  repeated chore lines are collapsed, and trailing GitHub issue/PR
  references are stripped. The notes are fetched together with the online
  version check under a localized heading; packages whose feeds carry no
  real notes (rolling snapshot releases) don't declare a notes source, and
  if notes can't be fetched the pane simply keeps its usual contents. (#4)

### Changed

- The check for the latest versions now runs several packages at once
  instead of one after another, so it finishes in roughly the time of the
  slowest single check rather than the sum of all of them. The status line
  reports "Checking versions… (3 of 11)" as they complete, so it keeps
  moving even while a slow check is still running — previously a language
  pack, whose check has to look at the file itself, could leave the
  progress bar sitting still for several seconds.
- A single package failing no longer aborts the whole install. RABBIT now
  records the failure, keeps installing every other selected package, and
  reports what succeeded and what failed at the end (the wizard's result
  page shows "Finished with errors"; the CLI still exits non-zero). The one
  exception is dependencies: packages install *into* REAPER, so if REAPER
  itself fails, its extensions (OSARA, SWS, ReaPack, ReaKontrol, the JAWS
  scripts, FFmpeg) are skipped with a "REAPER didn't install" note rather
  than being installed into a broken or absent REAPER. Standalone extras
  (Surge XT, app2clap) are unaffected by a REAPER failure. Dependencies are
  declared per package in the manifest (`depends_on`), and a dependency
  that isn't part of the current run (installing an extension into an
  already-present REAPER) never gates anything.
- macOS self-updates now replace the whole `RABBIT.app` bundle with the
  release's notarized bundle, instead of swapping just the executable inside
  it. The app keeps its original Apple-verified (Developer ID + notarized)
  signature across updates, so Gatekeeper stays satisfied without any
  workarounds and macOS permission grants (such as Full Disk Access, which
  are tied to the app's signature) survive updates instead of needing to be
  granted again. The previous bundle is kept next to the app as
  `RABBIT.app.rabbit-old` for manual rollback, and a renamed app keeps its
  name. Requires an updater that already has this change, so the first
  bundle-swap update is the one *after* this release; installs outside an
  `.app` bundle and Windows keep the existing binary swap.

### Fixed

- Windows: when antivirus software blocks a package, RABBIT now says so and
  explains how to continue, instead of failing with a raw
  "I/O error … (os error 225)". Microsoft Defender occasionally flags a
  legitimately code-signed third-party installer it hasn't seen widely yet —
  OSARA's development snapshots, rebuilt on every change, are the usual case —
  and quarantines it mid-install. The new message states that this is
  typically a false positive, that the installer is signed and RABBIT had
  already verified the download against the publisher's checksum, and walks
  through allowing the
  item under Windows Security → Virus & threat protection → Protection
  history (or excluding RABBIT's download folder, or installing that one
  package by hand). The steps also appear in your own language on the
  wizard's result page, and the README gained a matching section. RABBIT
  does not, and will not, switch your virus protection off.
- macOS: Cmd+Q (and the application menu's Quit item) now quits RABBIT.
  The wizard installed no menu bar at all, so macOS had no functional
  Quit item to offer and Cmd+Q was a dead key. RABBIT now installs a
  minimal menu bar on macOS whose stock Quit item lands in the
  application menu as usual; quitting this way behaves exactly like the
  window's close button.
- macOS: RABBIT no longer refuses to launch after a self-update. Updating
  replaces the binary inside `RABBIT.app` and re-signs the bundle with a
  local (ad-hoc) signature — but the bundle still carried the quarantine
  marker from its original download, so on the next launch from Finder or
  the Dock, Gatekeeper re-checked it, no longer found the notarized
  signature it had originally approved, and blocked the app with "RABBIT
  could not be opened" (the relaunch right after updating worked because it
  bypasses that check). The self-update now clears the quarantine marker
  from the whole bundle after re-signing, so subsequent launches work
  normally. Anyone already affected can fix their copy by running
  `xattr -dr com.apple.quarantine /path/to/RABBIT.app` in Terminal once (or
  re-downloading the release).
- Pressing Enter on the wizard's final **Done** page now activates **Close**.
  Enter already triggered each step's primary button, but the Done page left
  the disabled **Next** button as the default, so Enter did nothing there.
  Close is now the default button on that page, and Enter is additionally
  re-routed from the read-only summary and details text boxes — which
  otherwise swallow the key — so it works from the control the screen reader
  is parked on when the page opens. Enter still does nothing while an install
  is running.

## [0.3.2] - 2026-07-03

### Added

- RABBIT now writes a crash report when it crashes due to an internal error:
  a small plain-text file (`crash-<timestamp>.log`) with the RABBIT version,
  OS, the error and where it happened, saved right next to the RABBIT
  executable — next to the `RABBIT.app` bundle on macOS — in keeping with
  RABBIT being fully portable. If that location isn't writable (for example
  RABBIT running from a read-only disk image), the report falls back to the
  system temp folder as `rabbit-crash-<timestamp>.log`. When reporting a
  crash, attaching that file makes diagnosing it much easier.

### Fixed

- macOS: the app no longer crashes shortly after the first Next click, when
  the online version check finishes and the wizard advances to the packages
  page (a 0.3.1 regression: the screen-reader focus improvement selected the
  package list's top row through an API that is only valid for flat lists,
  not the tree the packages page uses, crashing the native macOS view; the
  page now simply receives keyboard focus, which already starts reading at
  the top).

## [0.3.1] - 2026-07-03

### Added

- Two new ReaPack repository configuration steps from the **REAPER
  Accessible** team: **REAPER Accessible (EN)** (reaperaccessible/rap_en,
  English-language resources, recommended) and **REAPER Accessible (FR)**
  (reaperaccessible/rap_fr, French-language resources, optional — offered
  but not pre-selected, since its content is French-specific). Like the
  existing REAPER Accessibility step, they're offered after install when
  ReaPack is present and wire the repositories into `reapack.ini`.

### Changed

- Installations are faster: package downloads now run in parallel (up to
  three at a time), and each package installs as soon as its own download
  finishes instead of waiting for every download to complete. In a typical
  setup, REAPER, OSARA, and SWS are already installed while FFmpeg's large
  archive is still downloading. Installs themselves still run one at a
  time and in the usual order, so nothing changes about elevation prompts
  or the order packages land on disk. The progress page now tracks several
  simultaneous downloads: the bar combines their progress, and the status
  line summarizes them ("Downloading 3 packages…") while an install is not
  running. If something fails partway through, packages that already
  installed keep their receipts, so RABBIT still knows about them
  afterwards.

### Fixed

- After the online version check, the packages page now places screen-reader
  focus on the top-most entry of the package list (the "Packages" group
  header) instead of landing on an arbitrary row a few items down. Going
  Back to the page still keeps your previous position.
- Large downloads no longer fail on slow or briefly stalling servers.
  Package downloads (and RABBIT's own self-update download) previously
  inherited a 30-second network timeout, so a download whose connection
  stalled for longer — routine for FFmpeg's ~390 MB archive from the busy
  gyan.dev server — was aborted with a cryptic "I/O error … error decoding
  response body" and took the whole installation with it. Downloads now
  tolerate stalls of up to a minute, and an interrupted connection is
  retried up to three times, resuming from the bytes already received when
  the server supports it (verified so a resumed file can never mix two
  different upstream versions) instead of starting over. If the connection
  keeps dropping, the error now clearly names the download URL and suggests
  checking the internet connection, rather than pointing at a temp file on
  disk.

## [0.3.0] - 2026-06-30

### Added

- **app2clap** (Windows only) — install and update support for jcsteh's
  app2clap, a CLAP plug-in that captures audio from other applications into
  REAPER (or any CLAP host). RABBIT tracks the rolling GitHub `snapshots`
  release, picks the highest-versioned build, and installs `app2clap.clap`
  into the per-user CLAP folder (`%LOCALAPPDATA%\Programs\Common\CLAP`), so no
  administrator rights are needed. It appears under the new **Additional
  software** group. Updates replace the existing copy (backing up the old one
  first); a copy installed for a different REAPER target or by hand is detected
  by file presence, with the version reported as unknown until RABBIT installs
  it itself. Like Surge XT, it's disabled on portable REAPER targets — the CLAP
  folder lives outside any portable REAPER folder.
- French (fr-FR) UI translation. RABBIT now ships with English, German, and
  French out of the box; the language picker lists Français (France) and the
  OS-language auto-detection on first launch covers French locales (including
  regional variants such as fr-CA, which map to fr-FR).
- Italian (it-IT) UI translation. The language picker now lists Italiano
  (Italia), it ships in the macOS app bundle, and OS-language auto-detection on
  first launch covers Italian locales (any `it-*`, which map to it-IT).
- The wizard's primary action is now the window's default button, so pressing
  Enter activates it from anywhere in the page (Next on the navigation steps,
  Install on the review step) instead of only when a button already holds
  focus. This matches the standard dialog convention on both macOS and Windows.
  A disabled default button stays a no-op, so an invalid Target step or a
  review step that can't install yet won't advance.

### Changed

- The wizard's package list now groups packages into an **Additional
  software** section, separate from the main **Packages** list, for extras
  that aren't part of REAPER itself. **Surge XT** moves into this new group
  (and future additional software like app2clap will land there too). Each
  group has its own check-all header and aggregate state; the main Packages
  group is unaffected. The new group is localized in all four UI languages
  and only appears when at least one additional package is available for the
  selected target.
- REAPER updates no longer leave an unwanted desktop icon on Windows.
  REAPER's silent installer always (re)creates a desktop shortcut, with no
  switch to suppress it, so updating a REAPER whose icon you'd deleted put it
  back. RABBIT now snapshots the desktop shortcuts before running the
  installer and, once the install is confirmed, removes a REAPER shortcut the
  installer freshly created — unless this is a brand-new standard install (the
  one case a new icon is wanted) or an icon was already there (never touched).
  Portable installs already create no shortcut, so this is a no-op for them.
  Windows only; checks both the per-user and the all-users desktop, and only
  removes a `REAPER*.lnk` that appeared during the install.
- A single unreachable upstream no longer blocks the entire update check.
  Previously, when one latest-version provider failed (e.g. the SWS homepage
  being down), the wizard stopped on the version-check page and the CLI's
  `latest` / `plan --online` commands aborted — even though every other
  package could still be checked and updated. Now the check always completes
  with whatever providers answered: in the wizard, each failed package's row
  is disabled (unchecked, with a localized "online version check failed"
  indicator) and a note with the full error appears in the review summary,
  while all other packages keep their normal install/update flow. On the
  CLI, `latest` prints resolved versions to stdout and per-package warnings
  to stderr, and `plan --online` records each failure as a plan note. The
  `latest --json` output shape changed accordingly: it is now an object with
  `packages` and `failures` arrays instead of a bare array.

### Fixed

- A blocked plugin overwrite on macOS now explains how to fix it instead of
  showing a bare "OS error 1". That error is EPERM ("operation not
  permitted") and is *not* REAPER being open (that would be a different error
  — RABBIT already handles it), so closing REAPER doesn't help; it's a macOS
  permission/modification gate (App Management on Sonoma and later, an
  immutable file flag, or ownership). When replacing an installed extension
  such as `reaper_kontrol.dylib` fails with EPERM/EACCES, RABBIT now reports
  that it's a permission block and points the user to grant RABBIT Full Disk
  Access (or App Management) under System Settings → Privacy & Security, then
  quit and relaunch — rather than leaving them with an opaque error.
  Additionally, a new macOS preflight check catches this *before* downloading
  and installing: it rehearses the write into the `UserPlugins` folder
  (creating and deleting a probe file, and renaming each already-installed
  `reaper_*` plugin aside and back — non-destructively, the files are left
  exactly as they were) and fails up front with the same guidance if the OS
  denies it. macOS only; other failures and platforms are unchanged.
- The "close REAPER before installing" preflight no longer fails open when
  REAPER's process is running but its executable path can't be read. On
  Windows that happens routinely — most often when REAPER is running elevated
  while RABBIT is not (the OS denies the image-path query), or under some
  antivirus — and RABBIT would detect the `reaper.exe` process by name but,
  unable to match its (unknown) path to the install target, silently treat it
  as a *different* REAPER and let the installer overwrite a running one. The
  check is now fail-safe: a running REAPER whose path we can't read is treated
  as the target and blocks the install (or warns, with the override on), so
  the user is told to close REAPER instead of getting a corrupted update. A
  REAPER with a readable, non-matching path is still correctly ignored.
- Setup no longer fails with "stream did not contain valid UTF-8" when
  `reapack.ini` isn't UTF-8-encoded. ReaPack writes its config through the
  Win32 profile-string APIs, which use the active ANSI code page (or UTF-16
  when the file carries a BOM), so a repository name with a single non-ASCII
  character — e.g. a CP-1252 curly apostrophe — made RABBIT's strict UTF-8
  read error out and abort the run's post-install configuration step, right
  after all packages had installed successfully. RABBIT now decodes such
  files losslessly (UTF-8, UTF-16 LE/BE with BOM, or byte-preserving ANSI),
  edits them, and writes them back in their original encoding, so existing
  bytes survive untouched and ReaPack still parses the file. The same
  encoding-tolerant handling now covers `reaper-kb.ini` during OSARA key-map
  replacement, whose script names can carry ANSI bytes too. Reported in
  issue #7.

## [0.2.1] - 2026-06-10

### Added

- The wizard now auto-detects a portable REAPER sitting in RABBIT's own
  folder. If you drop `rabbit` next to a portable REAPER (reaper.exe +
  reaper.ini on Windows, a `REAPER*.app` bundle on macOS), that install is
  discovered as a target and selected by default — so you can check for
  updates straight away without going through the Browse button.

### Changed

- macOS builds are now signed with an Apple Developer ID and notarized by
  Apple (then stapled) when the release's `MACOS_*` signing secrets are
  configured. Gatekeeper trusts a notarized bundle on first launch, so the
  download is just "unzip and double-click `Rabbit.app`" — the
  `Open Me First.command` quarantine helper is gone from signed builds. Forks
  and credential-less builds still fall back to ad-hoc signing with the
  helper. A new manual `macos-signing-smoke` workflow exercises and verifies
  the full sign → notarize → staple path so it can be validated before a
  release tag.

### Fixed

- Elevated NSIS installers now receive their `/D=<path>` install-directory
  flag correctly, so packages whose vendor installer is NSIS land in the
  REAPER folder RABBIT planned instead of NSIS's default
  `C:\Program Files\REAPER (x64)\`. `quote_one` was doubling every
  backslash and wrapping the flag in double quotes, but NSIS reads `/D=`
  straight from `GetCommandLine()` and takes everything after it verbatim
  to end-of-string — so the quoted path arrived with a trailing `"` baked
  into the directory name (an invalid path). NSIS fell back to its default
  location, and post-install verification at the planned path then
  surfaced the installer's `1223` exit code as a hard error, leaving every
  package after REAPER unprocessed. `/D=…` arguments are now passed
  unquoted and last as NSIS requires, and backslash escaping follows the
  Windows command-line spec (only backslashes immediately before a `"` are
  doubled). Windows only. Fixed in PR #6 by @trypsynth.
- Self-update apply on macOS no longer leaves the swapped binary
  non-executable. The staged source is the bare universal Mach-O off
  the GitHub release, and HTTPS downloads strip Unix mode bits, so
  `fs::copy` propagated a 0o644 mode onto `Rabbit.app/Contents/MacOS/rabbit`.
  Finder then labelled the file "document" instead of "Unix executable"
  and the bundle refused to launch — even though `codesign --force --deep`
  succeeded on it. `swap_install_file` now re-asserts 0o755 on the
  install target right after the copy, before the bundle re-sign step.
  No-op on Windows. Reported in issue #5.
- Self-update apply in the GUI now exits the old RABBIT process after
  spawning the relaunched copy. Previously the swap completed and the
  new version launched, but the pre-update window kept running next to
  it because the apply callback never asked the wx event loop to shut
  down. The relaunch path now mirrors the language-switch relaunch and
  calls `std::process::exit(0)` once the new process has been spawned;
  the error path is unchanged so a failed spawn still leaves the
  original window open. CLI `apply --restart` was already correct.

## [0.2.0] - 2026-05-15

### Added

- New package: **Surge XT**, the free open-source hybrid synthesizer from
  the [Surge Synth Team](https://surge-synthesizer.github.io/). Opt-in,
  Windows + macOS, standard REAPER installations only. RABBIT runs the
  vendor installer (Inno Setup on Windows, productbuild-wrapped `.pkg`
  on macOS) under elevation so the VST3, CLAP, AU (macOS) and standalone
  formats land system-wide for REAPER and other DAWs to pick up. Tracks
  the rolling nightly channel at
  `surge-synthesizer/surge` releases tag `Nightly` rather than the
  stable 1.3.4 release (2024-08-11) — the project effectively ships
  through nightlies now. Version detection layers a `NIGHTLY-<date>-<sha>`
  token from the receipt over a Medium-confidence semver fallback read
  from the vendor-installed VST3 bundle's file metadata.
- macOS elevation primitive in `rabbit-platform::elevation`: wraps the
  elevated command in `osascript -e 'do shell script "…" with
  administrator privileges'` so the system raises its native
  AuthorizationServices dialog. First (and currently only) consumer is
  the Surge XT `MountDiskImageAndRunPkgInstaller` runner.
- New `PlannedExecutionKind::MountDiskImageAndRunPkgInstaller` runner:
  mounts a `.dmg`, locates the inner `.pkg` via a filename-suffix glob
  matched against the mounted volume root, invokes `/usr/sbin/installer
  -pkg <path> -target /` under admin authorization, and detaches the
  image whether the install succeeded or failed.

- Automatic ReaPack script-action preservation across OSARA key-map
  replacement. Replacing `reaper-kb.ini` with `OSARA.ReaperKeyMap` —
  the default OSARA-recommended flow — drops every `SCR` line ReaPack
  had registered through REAPER's `AddRemoveReaScript` API, so installed
  ReaScripts disappear from REAPER's actions list until the user runs
  "ReaPack: Synchronize packages" inside REAPER (or re-installs every
  package). The unattended replacement path now reads the existing
  `reaper-kb.ini` first, captures all of its `SCR` records, lets the
  OSARA key map overwrite the file as before, then re-appends the
  preserved lines using the written file's newline convention. Any
  user `KEY` binding that targeted one of those scripts keeps working —
  REAPER derives the `_RS<hex>` action command ID deterministically
  from the script path, so the re-appended SCR lines bind to the same
  IDs the user already has in their key map. No opt-out: the
  preservation always runs when the OSARA key map is replaced.

- Live per-package progress reporting on the wizard's Installation
  progress page. The previous progress page set the gauge to 10 % when
  install kicked off, then jumped straight to 100 % at the end —
  everything in between was a black box even though packages like
  REAPER's macOS dmg take ~30 MB of network transfer per install.
  The setup pipeline now emits structured `ProgressEvent`s
  (`DownloadStarted` / `DownloadProgress` / `DownloadCompleted` /
  `InstallStarted` / `InstallCompleted` / `ConfigurationStarted` /
  `ConfigurationCompleted`) through an optional `ProgressReporter`
  threaded down from `execute_setup_operation_with_progress` into
  `download_artifacts_with_progress` and
  `install_cached_artifacts_with_progress`. The artifact downloader
  swapped `std::io::copy` for a chunked read/write loop that emits a
  byte-progress event every ~256 KiB or ~200 ms (whichever is rarer),
  so the gauge moves smoothly during the REAPER dmg pull instead of
  stalling. The wxdragon wizard forwards each event to the UI thread
  via `wxdragon::call_after`: the gauge advances by a per-phase
  fraction (weighted by completed downloads/installs plus the
  in-flight byte fraction), the status label updates to "Downloading
  REAPER… 12.4 MB / 30.0 MB", and a running log of completed
  transitions appends to the progress details TextCtrl (screen
  readers announce each new line as it lands). The no-progress
  entry points (`execute_setup_operation`,
  `execute_resolved_setup_operation`, `download_artifacts`,
  `install_cached_artifacts`, `execute_wizard_install`) stay on
  their existing signatures and delegate via `ProgressReporter::noop`,
  so the CLI and existing tests are unaffected.

### Fixed

- Wizard startup detection no longer SHA-256-hashes every receipted
  install file. `verify_package_receipt` used to verify each entry in
  a package's receipt by hashing the on-disk file, which on Windows
  meant 14 seconds of stalled UI just for FFmpeg (~200 MB of DLLs)
  and ~1 second for REAPER on every wizard launch — plus another
  round of the same after every install via the post-install rescan
  hook. The receipt verifier now checks file existence and size only;
  size mismatch alone catches every realistic regression the detection
  layer cares about (partial overwrites, truncated files), and the
  receipt's own stamped version is what gets shown either way.
  Recording hashes during install is unchanged.

- FFmpeg version detection no longer freezes the wizard for tens of
  seconds on Windows when an FFmpeg install is present. Probe 2
  previously called `ffmpeg.exe -version` via `std::process::Command`
  on the UI thread, which on Windows blocks for the entire AV scan of
  FFmpeg's dozens of DLL dependencies — easily 20-30 s per launch on
  a default-configured machine, and the same stall ran on the
  post-install rescan after installing FFmpeg too. Probe 2 now scans
  the `ffmpeg.exe` binary for the contiguous `show_banner` format
  string anchored on the unique `the FFmpeg developers` literal, and
  pulls `<VERSION>` out between the trailing `version ` token and the
  next `Copyright` marker. That matches both the upstream FFmpeg
  banner (`%s version <VERSION>, Copyright (c) …`) and the Gyan.dev
  full-builds variant that drops the comma and pads with spaces
  (`%s version 8.1.1-full_build-www.gyan.dev         Copyright (c) …`),
  so externally-installed Gyan FFmpegs now report `8.1.1` instead of
  the libavformat-major fallback's `8.0.0`. Same `High` confidence as
  before; the matching detector id changed from `ffmpeg-cli-version`
  to `ffmpeg-binary-version-string`.

## [0.1.2] - 2026-05-13

### Changed

- Self-update is now a modal Yes/No prompt at startup instead of a
  button on the wizard's Done page. The previous design was effectively
  unreachable: users had to finish an install before they ever saw the
  "Apply RABBIT update" button, and the always-visible status bar line
  pointed at a button most users couldn't find. The startup self-update
  check now raises a Yes/No dialog as soon as it completes; "Yes" runs
  the apply inline (with progress in the status bar) and relaunches
  RABBIT, "No" dismisses the prompt for the rest of the session. Users
  who change their mind can relaunch RABBIT to be re-prompted; the
  status-bar line spells that out.

### Fixed

- Per-arch artifact dispatch on a fresh first-time install. On macOS
  with no existing `/Applications/REAPER.app`, the binary-header probe
  in `standard_macos_installation` couldn't read a file that wasn't
  there yet and returned `Architecture::Unknown`. The SWS and ReaPack
  resolvers then fell through to their `Unknown → X64` fallback arms
  and downloaded `sws-…-Darwin-x86_64.dmg` and
  `reaper_reapack-x86_64.dylib`, even on Apple Silicon hosts running
  natively where the freshly-installed REAPER would launch as `arm64`
  and refuse to load the mismatched extension binaries. The dispatch-
  time canonicalizer (renamed from `canonicalize_macos_universal_arch`
  to `canonicalize_dispatch_arch`) now collapses `Unknown` to the host
  slice the same way it collapses `Universal` — `Architecture::current()`
  with Rosetta correction — so the upcoming install lands arch-correct
  plug-ins regardless of whether REAPER was already on disk when the
  wizard launched. The fix also closes the equivalent x64-fallback bug
  on Windows-on-ARM, where an unprobed target would have produced
  `Windows-x64.exe` SWS and `reaper_reapack-x64.dll` instead of the
  arm64ec variants.

- macOS Rosetta detection. v0.1.1 shelled out to `/usr/sbin/sysctl -n
  sysctl.proc_translated` to determine whether RABBIT was running
  under Rosetta, but `sysctl.proc_translated` reports the *querying*
  process's translation state — and the shelled-out `sysctl` binary
  always runs as the host's native arch (the kernel picks its native
  slice at exec time, ignoring the parent's translation state). The
  probe therefore always reported `false`, including when RABBIT
  itself was the translated `x86_64` slice on Apple Silicon. The
  artifact dispatcher then canonicalized REAPER-Universal to
  `Architecture::current()` (i.e., `X64`) and installed `x86_64`
  plug-ins against an `arm64`-native REAPER process. The probe now
  calls `sysctlbyname` directly via FFI from RABBIT's own process, so
  the kernel resolves the key against RABBIT's translation state.

- macOS self-update used to leave `Rabbit.app` structurally invalid
  after the binary swap. The release pipeline now ad-hoc signs the
  bare `rabbit-<version>-macos-universal` artifact itself (so the
  staged-in binary has a valid embedded signature even though
  `lipo -create` strips its inputs' sigs), and `apply_self_update`
  re-seals the enclosing `.app` bundle with `codesign --force --deep
  --sign -` after the swap. Both pieces are needed: the bare-binary
  signing keeps Apple Silicon's exec checks happy and the bundle
  re-seal restores the `_CodeSignature/CodeResources` consistency that
  Gatekeeper checks on Finder launch. Without these, post-update
  Finder launches on macOS 15 (Sequoia) and 26 (Tahoe) would refuse
  the bundle as corrupt rather than just untrusted.

## [0.1.1] - 2026-05-10

### Added

- Stable always-latest download URLs for every platform, e.g.
  `https://github.com/Timtam/rabbit/releases/latest/download/rabbit-windows-x86_64.exe`
  and `…/rabbit-macos-universal.app.zip`. The release pipeline now
  publishes version-less aliases of each artifact alongside the
  versioned originals, so the URLs above resolve to whatever release is
  current at the time of the click. The README's Download section has
  been switched to direct links; the GitHub Releases page is still
  there for users who want to pin a version or verify SHA-256 sums.

### Changed

- macOS: ship a single universal Mach-O instead of separate Apple Silicon
  and Intel builds. The release pipeline now builds both arches and
  `lipo -create`s them into one binary, published as
  `rabbit-<version>-macos-universal` (and `…-universal.app.zip`). The
  self-update manifest's `platforms` map keeps its `macos-aarch64` and
  `macos-x86_64` keys for backward compatibility with already-released
  RABBIT 0.1.0 clients, both pointing at the universal artifact, so
  existing installs migrate to the fat binary on their next self-update
  check without a manual download.

### Fixed

- macOS: `Cmd+C` (and `Cmd+S` in the German build) no longer closes the
  wizard. Wizard buttons used `&Close` / `&Schließen`-style mnemonics
  for Alt-key access on Windows and Linux, but wxWidgets' OSX backend
  binds those `&` mnemonics as `Cmd+letter` accelerators — colliding
  with macOS system shortcuts (Cmd+C copy, Cmd+S save, …). Mnemonics
  on buttons aren't HIG-conformant on macOS anyway, so they're now
  stripped from the label entirely on that platform; underlined
  Alt-key access continues to work on Windows and Linux.

- macOS first-launch helper: `Open Me First.command` no longer falsely
  reports success on macOS 15 (Sequoia) and 26 (Tahoe). Removing the
  `com.apple.quarantine` xattr is no longer enough on those versions —
  Gatekeeper blocks unsigned bundles on first launch regardless of
  quarantine state. The helper now (a) verifies the xattr was actually
  cleared *recursively* across the bundle (the previous version checked
  only the bundle root, missing inner-file failures), (b) detects the
  macOS version, and (c) on macOS 15+ triggers a launch attempt to
  register Rabbit.app with Gatekeeper, then deep-links System Settings
  → Privacy & Security so the user's "Open Anyway" approval is one
  click away. macOS 14 and earlier keep the original quiet behavior.

- macOS: install no longer aborts with
  `no artifact found for sws on MacOs/Universal` (and the analogous
  silent ReaPack-arm64 mis-install on Intel hosts) when REAPER is a
  universal Mach-O. The artifact dispatcher now canonicalizes
  `Architecture::Universal` to the host slice on macOS before
  per-package resolvers run, so SWS picks the matching `Darwin-x86_64`
  / `Darwin-arm64` `.dmg` and ReaPack picks the matching
  `reaper_reapack-x86_64.dylib` / `reaper_reapack-arm64.dylib`. On
  Apple Silicon Macs running RABBIT under Rosetta, the dispatcher
  consults `sysctl.proc_translated` and forces the `arm64` slice so
  plug-ins match the `arm64`-native REAPER process rather than the
  `x86_64` translator's view.

- macOS: VoiceOver now reads the German UI with a German voice. The
  bundle previously declared only English (`CFBundleDevelopmentRegion`
  with no `CFBundleLocalizations` and no `.lproj` directories), so
  Cocoa picked the English voice for every accessibility string
  regardless of the in-app language. The bundle now ships
  `CFBundleLocalizations` for `en` and `de`, matching empty
  `en.lproj` / `de.lproj` stubs, and seats `AppleLanguages` from the
  resolved runtime locale before wxDragon brings up Cocoa so the
  override also takes effect for users on an English-language macOS
  who switch RABBIT to German.

## [0.1.0] - 2026-05-09

Initial public release. RABBIT is a REAPER accessibility bootstrap and
bundle installer with a screen-reader-friendly GUI wizard and a matching
CLI, packaged as a single self-contained executable per platform.

### Added

- **Accessibility-first wizard** built on wxDragon: keyboard-first
  navigation, native controls, tested with NVDA, JAWS, Narrator, and
  VoiceOver. Wizard runs when launched without arguments; arguments hand
  off to the CLI.
- **Cross-platform builds**: Windows x86_64, Windows aarch64, macOS
  aarch64, and macOS x86_64. macOS ships as an ad-hoc-signed `.app.zip`
  with an `Open Me First.command` helper that clears Gatekeeper
  quarantine on first launch.
- **Localization**: English (en-US) and German (de-DE) bundled, with
  Fluent-based runtime locale selection. Auto-picks the OS language when
  a translation is available.
- **REAPER discovery**: detects standard installations on Windows and
  macOS, plus user-supplied portable folders. Reports app path, resource
  path, version, architecture, writability, and confidence.
- **Package install / update** for REAPER, OSARA, SWS, ReaPack,
  ReaKontrol, JAWS-for-REAPER scripts (Windows + JAWS only), and FFmpeg
  shared runtime (Windows, opt-in; pulled from gyan.dev on x64 and
  tordona/ffmpeg-win-arm64 on ARM64, pinned to FFmpeg 8.x).
- **Architecture-aware artifact resolution**: selects x86_64 / aarch64 /
  arm64ec / universal builds appropriate to the detected REAPER, with
  per-arch self-update assets in the release manifest.
- **Configuration steps**: post-install tweaks managed alongside package
  installs. Ships with `reapack-add-reaper-accessibility-remote`, which
  adds the REAPER Accessibility ReaPack repository
  (`Timtam/reapack`) and is idempotent on re-runs.
- **Preflight checks** that run before any apply: REAPER process state,
  resource path writability, donation acknowledgement for ReaPack, etc.
  CLI returns a non-zero exit when checks fail.
- **Dry-run by default in the CLI**: every install / update / restore
  command requires `--apply` to make changes. JSON output available on
  every command via `--json`.
- **Backups and rollback**: each install writes a backup set under
  `<resource>/RABBIT/backups/<timestamp>/`; `rabbit backups` lists them
  and `rabbit restore-backup` rolls one back.
- **JSON + text reports** under `<resource>/RABBIT/logs/` for every
  install, update, restore, and setup operation.
- **Self-update**: signed update manifest published alongside each
  GitHub release with per-arch URLs and SHA-256 sums; `rabbit
  self-update check / stage / apply [--restart]` (CLI) and an automatic
  on-startup check (GUI). Handles bare-binary replacement on Windows and
  macOS without the user re-clearing Gatekeeper.
- **Portable runtime**: the binary carries embedded resources (locales,
  package manifest). Cache and lock files live under the OS cache directory by
  default; nothing is left behind in the resource path beyond explicit
  reports and backups.
