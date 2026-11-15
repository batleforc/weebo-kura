#!/usr/bin/env bash
# An Android emulator against the local stack, for the plugin on mobile
# (RFC 0001 decisions 53–55, observation 12; RFC 0002 observation 4).
#
# The stack listens on loopback only, and so it stays: `adb reverse` makes the
# emulator's own localhost:58080 (kura-server) and localhost:59443 (Authentik)
# reach this PC, so the plugin uses the same URLs as on the desktop, the
# server's https issuer is unchanged and the dev certificate's `localhost`
# name matches. Android apps do not trust user-installed CAs, so the dev CA
# goes into the system store: that needs a Google APIs image (no Play Store,
# `adb root`) started with -writable-system.
#
# usage: dev/android.sh setup|start|trust|reverse|obsidian [APK]|deploy VAULT
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
export ANDROID_HOME=${ANDROID_HOME:-$HOME/Android/Sdk}
AVD=${KURA_AVD:-kura-dev}
IMAGE="system-images;android-33;google_apis;x86_64"
SDK_TOOLS="vfox:mise-plugins/vfox-android-sdk@13.0"
JAVA="java@temurin-21"
ca="$root/dev/certs/ca.pem"

sdk() { mise exec "$SDK_TOOLS" "$JAVA" -- "$@"; }
adb() { "$ANDROID_HOME/platform-tools/adb" "$@"; }
booted() { [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ]; }
wait_boot() {
  adb wait-for-device
  for _ in $(seq 120); do booted && return 0; sleep 2; done
  echo "the emulator did not finish booting in 4 minutes" >&2; exit 1
}

case "${1:-}" in
  setup)
    # About 3 GB under $ANDROID_HOME, once.
    [ -r /dev/kvm ] || { echo "no /dev/kvm: the emulator needs KVM" >&2; exit 1; }
    yes | sdk sdkmanager --sdk_root="$ANDROID_HOME" --licenses >/dev/null
    sdk sdkmanager --sdk_root="$ANDROID_HOME" "platform-tools" "emulator" "$IMAGE"
    echo no | sdk avdmanager create avd --force --name "$AVD" --package "$IMAGE" --device pixel_6
    echo "AVD $AVD ready under $ANDROID_HOME; next: task android:start"
    ;;
  start)
    "$ANDROID_HOME/emulator/emulator" -avd "$AVD" -writable-system -no-snapshot-save -no-boot-anim >"${TMPDIR:-/tmp}/kura-emulator.log" 2>&1 &
    wait_boot
    echo "emulator $AVD booted (log: ${TMPDIR:-/tmp}/kura-emulator.log)"
    "$0" trust
    "$0" reverse
    ;;
  trust)
    # The dev CA as a system certificate: <subject_hash_old>.0 (Android's naming).
    name="$(openssl x509 -inform PEM -subject_hash_old -noout -in "$ca").0"
    adb root >/dev/null; adb wait-for-device
    if ! adb shell test -f "/system/etc/security/cacerts/$name"; then
      # The first remount turns verity off and asks for a reboot.
      if ! adb remount 2>&1 | grep -q "remounted"; then
        adb reboot; wait_boot; adb root >/dev/null; adb wait-for-device; adb remount >/dev/null
      fi
      adb push "$ca" "/system/etc/security/cacerts/$name" >/dev/null
      adb shell chmod 644 "/system/etc/security/cacerts/$name"
      adb reboot; wait_boot
    fi
    echo "dev CA trusted by the emulator's system store ($name)"
    ;;
  reverse)
    adb reverse tcp:58080 tcp:58080
    adb reverse tcp:59443 tcp:59443
    echo "emulator localhost:58080 and localhost:59443 now reach this PC"
    ;;
  obsidian)
    # Obsidian publishes its Android APK with each release on GitHub.
    apk=${2:-}
    if [ -z "$apk" ]; then
      apk="${TMPDIR:-/tmp}/obsidian.apk"
      url=$(curl -sf https://api.github.com/repos/obsidianmd/obsidian-releases/releases/latest |
        jq -r '.assets[] | select(.name | endswith(".apk")) | .browser_download_url' | head -1)
      [ -n "$url" ] || { echo "no APK in the latest Obsidian release; pass one: task android:obsidian APK=<file>" >&2; exit 1; }
      curl -sfL -o "$apk" "$url"
    fi
    adb install -r "$apk"
    echo "Obsidian installed; open it, create a vault in Documents, then task android:deploy VAULT=<name>"
    ;;
  deploy)
    vault=${2:?usage: dev/android.sh deploy <vault name in Documents>}
    dir="/sdcard/Documents/$vault/.obsidian/plugins/weebo-kura"
    adb shell mkdir -p "$dir"
    for f in main.js manifest.json styles.css; do adb push "$root/plugin/$f" "$dir/$f" >/dev/null; done
    echo "Kura Sync copied into $dir; settings as on the desktop:"
    echo "  Server        http://127.0.0.1:58080"
    echo "  Sign-in URL   https://localhost:59443/application/o/kura-plugin/"
    echo "  Sign in       alice / kura-dev; vault obsidian or notes"
    ;;
  *)
    echo "usage: $0 setup|start|trust|reverse|obsidian [APK]|deploy VAULT" >&2
    exit 2
    ;;
esac
