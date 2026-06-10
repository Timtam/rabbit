#!/usr/bin/env bash
# Assembles a Rabbit.app bundle from a built rabbit binary, then wraps it in a
# zip for distribution. Two output shapes depending on how the bundle is signed:
#
#   * Developer ID + notarized (--sign + --notarize): the zip contains
#     Rabbit.app on its own. Apple has notarized and we staple the ticket, so
#     Gatekeeper trusts it on first launch — the user just unzips and
#     double-clicks. No quarantine helper.
#
#   * unsigned / ad-hoc (--adhoc-sign or neither): the zip contains a wrapper
#     folder with Rabbit.app plus an "Open Me First.command" helper that clears
#     macOS's first-launch quarantine, since Gatekeeper blocks un-notarized
#     bundles. This is the path CI snapshot builds take.
#
# Zip layout (ad-hoc/unsigned):
#   Rabbit/
#     Rabbit.app/Contents/{Info.plist,MacOS/rabbit,Resources,PkgInfo}
#     Open Me First.command
set -euo pipefail

usage() {
	cat >&2 <<'USAGE'
Usage: build-bundle.sh --binary <path> --version <x.y.z> --out <dir> --zip-name <name.zip> [signing flags]
  --binary         Path to the built rabbit Mach-O executable.
  --version        Version string to embed in CFBundleVersion / CFBundleShortVersionString.
  --out            Output directory; will be created if missing. Both Rabbit.app and the zip land here.
  --zip-name       Filename for the zipped bundle (e.g. rabbit-0.1.0-macos-universal.app.zip).

Signing (pick at most one of --sign / --adhoc-sign):
  --sign <identity>   Developer ID sign with the given codesign identity, e.g.
                      "Developer ID Application: Name (TEAMID)". Signs the inner
                      binary and the bundle with the hardened runtime + a secure
                      timestamp (both required for notarization).
  --adhoc-sign        Ad-hoc sign (codesign -s -). Doesn't satisfy Gatekeeper for
                      distribution; ships the "Open Me First.command" helper.

Notarization (requires --sign):
  --notarize             Submit the signed bundle to Apple's notary service and
                         staple the resulting ticket. Ships Rabbit.app with no helper.
  --notary-key <path>    App Store Connect API key (.p8) for notarytool.
  --notary-key-id <id>   App Store Connect API Key ID.
  --notary-issuer <id>   App Store Connect API Issuer ID.
USAGE
	exit 64
}

BINARY=""
VERSION=""
OUT_DIR=""
ZIP_NAME=""
ADHOC_SIGN=0
SIGN_IDENTITY=""
NOTARIZE=0
NOTARY_KEY=""
NOTARY_KEY_ID=""
NOTARY_ISSUER=""

while [ $# -gt 0 ]; do
	case "$1" in
		--binary) BINARY="$2"; shift 2 ;;
		--version) VERSION="$2"; shift 2 ;;
		--out) OUT_DIR="$2"; shift 2 ;;
		--zip-name) ZIP_NAME="$2"; shift 2 ;;
		--adhoc-sign) ADHOC_SIGN=1; shift ;;
		--sign) SIGN_IDENTITY="$2"; shift 2 ;;
		--notarize) NOTARIZE=1; shift ;;
		--notary-key) NOTARY_KEY="$2"; shift 2 ;;
		--notary-key-id) NOTARY_KEY_ID="$2"; shift 2 ;;
		--notary-issuer) NOTARY_ISSUER="$2"; shift 2 ;;
		-h|--help) usage ;;
		*) echo "unknown argument: $1" >&2; usage ;;
	esac
done

if [ -z "$BINARY" ] || [ -z "$VERSION" ] || [ -z "$OUT_DIR" ] || [ -z "$ZIP_NAME" ]; then
	usage
fi
if [ ! -f "$BINARY" ]; then
	echo "binary not found: $BINARY" >&2
	exit 1
fi
if [ -n "$SIGN_IDENTITY" ] && [ "$ADHOC_SIGN" -eq 1 ]; then
	echo "--sign and --adhoc-sign are mutually exclusive" >&2
	exit 64
fi
if [ "$NOTARIZE" -eq 1 ]; then
	if [ -z "$SIGN_IDENTITY" ]; then
		echo "--notarize requires --sign (notarization needs a Developer ID signature)" >&2
		exit 64
	fi
	if [ -z "$NOTARY_KEY" ] || [ -z "$NOTARY_KEY_ID" ] || [ -z "$NOTARY_ISSUER" ]; then
		echo "--notarize requires --notary-key, --notary-key-id, and --notary-issuer" >&2
		exit 64
	fi
	if [ ! -f "$NOTARY_KEY" ]; then
		echo "notary key not found: $NOTARY_KEY" >&2
		exit 1
	fi
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INFO_PLIST_TEMPLATE="$SCRIPT_DIR/Info.plist"
if [ ! -f "$INFO_PLIST_TEMPLATE" ]; then
	echo "Info.plist template missing at $INFO_PLIST_TEMPLATE" >&2
	exit 1
fi

mkdir -p "$OUT_DIR"
APP_DIR="$OUT_DIR/Rabbit.app"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"

# Stub .lproj directories matching CFBundleLocalizations. macOS's accessibility
# stack (and parts of Launch Services) inspects the bundle's .lproj layout in
# addition to the plist key when deciding what language the app speaks; ship
# both so VoiceOver picks the right voice for the in-app language. RABBIT's
# strings live in Fluent files outside the bundle, so the directories are
# deliberately empty — they exist only as a localization signal.
for lproj in en de; do
	mkdir -p "$APP_DIR/Contents/Resources/$lproj.lproj"
done

# Substitute the version token. Escape any '/' or '&' so sed doesn't
# misinterpret them — versions with build metadata can contain '+'.
ESCAPED_VERSION="$(printf '%s' "$VERSION" | sed -e 's/[\/&]/\\&/g')"
sed -e "s/@VERSION@/$ESCAPED_VERSION/g" "$INFO_PLIST_TEMPLATE" > "$APP_DIR/Contents/Info.plist"

cp "$BINARY" "$APP_DIR/Contents/MacOS/rabbit"
chmod +x "$APP_DIR/Contents/MacOS/rabbit"

# PkgInfo is optional but Launch Services historically reads it. APPL????
# matches CFBundlePackageType + CFBundleSignature in Info.plist.
printf 'APPL????' > "$APP_DIR/Contents/PkgInfo"

# --- Sign ---
if [ -n "$SIGN_IDENTITY" ]; then
	# Developer ID: sign the inner Mach-O first, then the bundle. Each gets
	# the hardened runtime (--options runtime) and a secure timestamp
	# (--timestamp) — both are prerequisites for notarization. RABBIT is
	# statically linked and never dlopens external code into its own process,
	# so no entitlements are required.
	codesign --force --options runtime --timestamp \
		--sign "$SIGN_IDENTITY" "$APP_DIR/Contents/MacOS/rabbit"
	codesign --force --options runtime --timestamp \
		--sign "$SIGN_IDENTITY" "$APP_DIR"
elif [ "$ADHOC_SIGN" -eq 1 ]; then
	# Ad-hoc signing (-s -) doesn't satisfy Gatekeeper for distribution but
	# avoids the "damaged and can't be opened" error that hits unsigned
	# binaries on Apple Silicon for downloads carrying the quarantine bit.
	# First-launch trust is cleared by the bundled "Open Me First.command"
	# helper below or by `xattr -dr com.apple.quarantine`.
	codesign --force --deep --sign - "$APP_DIR"
fi

# --- Notarize + staple ---
if [ "$NOTARIZE" -eq 1 ]; then
	# notarytool only accepts a zip/dmg/pkg container, never a bare .app, so
	# zip the bundle just for submission.
	SUBMIT_ZIP="$OUT_DIR/.notarize-submit.zip"
	rm -f "$SUBMIT_ZIP"
	ditto -c -k --keepParent "$APP_DIR" "$SUBMIT_ZIP"
	echo "submitting Rabbit.app for notarization (this can take several minutes)..."
	xcrun notarytool submit "$SUBMIT_ZIP" \
		--key "$NOTARY_KEY" \
		--key-id "$NOTARY_KEY_ID" \
		--issuer "$NOTARY_ISSUER" \
		--wait
	rm -f "$SUBMIT_ZIP"
	# Staple the ticket into the bundle so Gatekeeper validates offline,
	# without a network round-trip on the user's first launch.
	echo "stapling notarization ticket..."
	xcrun stapler staple "$APP_DIR"
fi

ZIP_PATH="$OUT_DIR/$ZIP_NAME"
rm -f "$ZIP_PATH"

if [ "$NOTARIZE" -eq 1 ]; then
	# Notarized public artifact: ship Rabbit.app on its own. No quarantine
	# helper is needed — a notarized, stapled bundle launches normally on
	# first run, so the user just unzips and double-clicks.
	# `ditto -c -k --keepParent` preserves the executable bit and resource
	# forks (plain `zip` does not, which would produce a broken .app).
	ditto -c -k --keepParent "$APP_DIR" "$ZIP_PATH"

	# Fail the build if the shipped bundle isn't actually trusted. spctl's
	# verdict here ("accepted, source=Notarized Developer ID") is the same
	# one a user's Mac reaches, so this is real proof — even from CI.
	echo "verifying signature + notarization..."
	codesign --verify --deep --strict --verbose=2 "$APP_DIR"
	xcrun stapler validate "$APP_DIR"
	spctl --assess --type exec -vvv "$APP_DIR"
else
	# Unsigned / ad-hoc snapshot build: stage Rabbit.app + the unquarantine
	# helper under a single wrapper folder so both extract together when the
	# user double-clicks the zip. These artifacts are not notarized, so
	# Gatekeeper blocks first launch without the helper.
	STAGE_DIR="$OUT_DIR/.bundle-stage"
	WRAPPER_NAME="Rabbit"
	rm -rf "$STAGE_DIR"
	mkdir -p "$STAGE_DIR/$WRAPPER_NAME"
	mv "$APP_DIR" "$STAGE_DIR/$WRAPPER_NAME/Rabbit.app"
	APP_DIR="$STAGE_DIR/$WRAPPER_NAME/Rabbit.app"

	cat > "$STAGE_DIR/$WRAPPER_NAME/Open Me First.command" <<'HELPER'
#!/bin/bash
# RABBIT ships unsigned (no Apple Developer Program enrollment). This helper
# does two things:
#
#   1. Clears `com.apple.quarantine` from Rabbit.app and every file inside
#      it. On older macOS that's enough — the app launches normally
#      afterward.
#
#   2. On macOS 15 (Sequoia) and 26 (Tahoe), removing the xattr is no longer
#      sufficient: Gatekeeper still blocks first-launch of unsigned/ad-hoc
#      bundles regardless of quarantine state. The only path is the one
#      Apple intends — let the launch attempt fail, then approve via
#      System Settings -> Privacy & Security. To make that one-click for
#      the user, the helper triggers the launch (so an entry appears in
#      that settings pane) and immediately deep-links the pane.
set -u

DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET="$DIR/Rabbit.app"

pause() {
	# Keep the Terminal window open after the script finishes so the user can
	# read the result regardless of their Terminal "When the shell exits"
	# preference.
	echo
	printf "Press Return to close this window. "
	read -r _ || true
}

echo "RABBIT first-launch trust helper"
echo "================================"
echo

if [ ! -d "$TARGET" ]; then
	echo "Rabbit.app was not found next to this helper at:"
	echo "  $TARGET"
	echo
	echo "Make sure both items (Rabbit.app and 'Open Me First.command')"
	echo "extracted into the same folder, then run this helper again."
	pause
	exit 1
fi

# Step 1: clear quarantine recursively. Capture xattr's stderr instead of
# silencing it — permission failures (Files-and-Folders gate, read-only zip,
# iCloud sync conflicts) need to surface to the user, not get hidden behind
# `|| true` like the previous version did.
echo "Clearing macOS quarantine from:"
echo "  $TARGET"
echo
xattr_output="$(xattr -dr com.apple.quarantine "$TARGET" 2>&1)" || true
if [ -n "$xattr_output" ]; then
	echo "xattr reported:"
	printf '  %s\n' "$xattr_output" | sed 's/^/  /'
	echo
fi

# Step 2: verify recursively, not just on the top-level bundle. The
# previous script only checked $TARGET itself, which would miss inner
# files (Contents/MacOS/rabbit, frameworks) that retained the xattr after
# a partial clear. Gatekeeper looks at the executable too, so a partial
# clear still triggers the warning — and we'd be lying when we said
# "trusted".
remaining="$(find "$TARGET" -exec sh -c '
	for path in "$@"; do
		if /usr/bin/xattr -p com.apple.quarantine "$path" >/dev/null 2>&1; then
			printf "%s\n" "$path"
		fi
	done
' _ {} +)"
if [ -n "$remaining" ]; then
	count="$(printf '%s\n' "$remaining" | wc -l | tr -d ' ')"
	echo "ERROR: $count file(s) inside Rabbit.app still carry com.apple.quarantine."
	echo "First few paths:"
	printf '%s\n' "$remaining" | head -5 | sed 's/^/  /'
	echo
	echo "Common causes:"
	echo
	echo "  - The Rabbit folder is on Desktop, Documents, or iCloud Drive and"
	echo "    Terminal does not have permission to modify files there."
	echo "    Fix: move the Rabbit folder to ~/Downloads and run this helper"
	echo "    again, OR open System Settings -> Privacy & Security ->"
	echo "    Files and Folders, find Terminal, and enable access for the"
	echo "    folder you extracted into."
	echo
	echo "  - Rabbit.app is still inside the downloaded .zip (read-only)."
	echo "    Fix: extract the Rabbit folder to a writable location first."
	echo
	echo "Manual fallback (run in Terminal):"
	echo "  xattr -dr com.apple.quarantine \"$TARGET\""
	pause
	exit 1
fi

echo "Quarantine cleared from Rabbit.app and every file inside it."
echo

# Step 3: route the user to Gatekeeper approval on macOS 15+ where the
# xattr clear isn't enough, and stay out of their way on macOS 14 and
# earlier where it usually is.
#
# Detection is by major version from `sw_vers`. macOS 15 is Sequoia (the
# version that removed right-click -> Open as a bypass); macOS 26 is Tahoe
# (the version that started flagging unsigned bundles regardless of
# quarantine state). Treat unknown / unparseable versions as strict
# Gatekeeper too, so future macOS releases default to the safer flow
# without a code change.
macos_version="$(/usr/bin/sw_vers -productVersion 2>/dev/null || echo "")"
macos_major="${macos_version%%.*}"

needs_settings_approval=0
if [ -z "$macos_major" ] || ! [[ "$macos_major" =~ ^[0-9]+$ ]] || [ "$macos_major" -ge 15 ]; then
	needs_settings_approval=1
fi

if [ "$needs_settings_approval" -eq 0 ]; then
	echo "Detected macOS $macos_version. Quarantine clearance is sufficient on this version."
	echo "You can close this window and double-click Rabbit.app to launch RABBIT."
	echo
	echo "If macOS still blocks the launch with a security warning, open"
	echo "System Settings -> Privacy & Security, scroll to the Security section,"
	echo "and click 'Open Anyway' next to the Rabbit entry."
	pause
	exit 0
fi

echo "Detected macOS ${macos_version:-unknown} — Gatekeeper approval is required"
echo "even after quarantine is cleared. Setting up the approval flow now..."
echo

# Trigger the launch attempt. We don't care about `open`'s exit status —
# it returns 0 the moment LaunchServices accepts the request, regardless
# of whether Gatekeeper later blocks the actual execution. The point is
# to register Rabbit.app with Gatekeeper so an "Open Anyway" entry
# appears in the Privacy & Security pane.
open "$TARGET" >/dev/null 2>&1 || true

# Brief pause so any Gatekeeper dialog has a chance to render before we
# steal focus by opening Settings. Sleeps shorter than ~1s race the
# dialog on slower hardware; longer than ~3s feels laggy.
sleep 2

# Deep-link System Settings -> Privacy & Security. The `.extension` URL
# is the modern (Ventura+) form; the legacy `com.apple.preference.security`
# pane id keeps Monterey and earlier working. Falling through to
# `open -b com.apple.systempreferences` is the bare-bones last resort.
open "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension" >/dev/null 2>&1 || \
	open "x-apple.systempreferences:com.apple.preference.security" >/dev/null 2>&1 || \
	open -b com.apple.systempreferences >/dev/null 2>&1 || true

cat <<'NEXT_STEPS'

macOS likely showed a security warning instead of launching Rabbit.
That's expected for unsigned apps on macOS 15 and later. To approve:

  1. Dismiss the security warning (click "Done").
  2. In the Settings window we just opened, scroll to the "Security"
     section near the bottom of Privacy & Security.
  3. Click "Open Anyway" next to the Rabbit entry.
  4. Confirm with your password or Touch ID if asked. Rabbit.app
     will launch.

This approval is per-app, not per-launch — once you've clicked
"Open Anyway", future double-clicks on Rabbit.app work normally.
RABBIT's self-update replaces the binary in place under the same bundle
identity, so updates inherit the approval; only a fresh download into a
different location triggers the dance again.
NEXT_STEPS
pause
HELPER
	chmod +x "$STAGE_DIR/$WRAPPER_NAME/Open Me First.command"

	# `ditto -c -k --keepParent` preserves the executable bit and resource
	# forks (plain `zip` does not). The wrapper folder keeps Rabbit.app + the
	# helper grouped after extraction.
	ditto -c -k --keepParent "$STAGE_DIR/$WRAPPER_NAME" "$ZIP_PATH"
	rm -rf "$STAGE_DIR"
fi

shasum -a 256 "$ZIP_PATH" | awk -v name="$ZIP_NAME" '{print tolower($1) "  " name}' > "$ZIP_PATH.sha256"

echo "wrote zip:    $ZIP_PATH"
echo "wrote sha256: $ZIP_PATH.sha256"
