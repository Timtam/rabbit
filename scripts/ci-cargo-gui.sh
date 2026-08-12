#!/usr/bin/env bash
# Run a cargo command that builds the GUI, retrying ONLY the known transient
# dependency download.
#
# `wxdragon-sys`'s build script fetches the wxWidgets source archive from
# GitHub releases itself, with no retry. That transfer intermittently ends
# early — reqwest reports `hyper::Error(IncompleteMessage)` — and fails
# builds that have nothing to do with the change under test. It is the same
# GitHub-releases flakiness that made us wrap our own LLVM/Ninja downloads
# in curl retries; this one happens inside a dependency we do not control,
# so the retry has to sit around the cargo invocation instead.
#
# Deliberately NOT a blanket retry: the output is matched for that specific
# download failure, so a genuine compile or clippy error fails on the first
# attempt and is never masked or slowed down. Setting WXWIDGETS_DIR would
# skip the download entirely, but it also skips the crate's own version
# check, and the wxWidgets version would then be pinned in this repo as well
# as in wxdragon-sys — silently drifting apart the next time the dependency
# is bumped.
#
# Usage: scripts/ci-cargo-gui.sh cargo clippy -p rabbit-ui-wxdragon --features gui
set -uo pipefail

ATTEMPTS="${CI_CARGO_GUI_ATTEMPTS:-3}"
DELAY="${CI_CARGO_GUI_DELAY:-15}"
# Matched against the build script's own `cargo::error=` line.
TRANSIENT='Could not download wxWidgets source archive'

if [ "$#" -eq 0 ]; then
	echo "usage: $0 <command> [args...]" >&2
	exit 2
fi

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

attempt=1
while :; do
	if "$@" 2>&1 | tee "$log"; then
		exit 0
	fi
	status="${PIPESTATUS[0]}"

	if ! grep -qF "$TRANSIENT" "$log"; then
		echo "command failed for a reason other than the wxWidgets download; not retrying" >&2
		exit "$status"
	fi
	if [ "$attempt" -ge "$ATTEMPTS" ]; then
		echo "::error::wxWidgets source download still failing after $ATTEMPTS attempts" >&2
		exit "$status"
	fi

	echo "::warning::wxWidgets source download failed (attempt $attempt/$ATTEMPTS); retrying in ${DELAY}s"
	# The build script leaves a partial archive in the temp dir; clear it so
	# the retry starts clean rather than tripping over a truncated file.
	rm -f "${TMPDIR:-${RUNNER_TEMP:-/tmp}}/wxWidgets.zip" 2>/dev/null || true
	sleep "$DELAY"
	attempt=$((attempt + 1))
done
