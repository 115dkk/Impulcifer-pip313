#!/usr/bin/env bash
# Runs inside the emulator step of android.yml (ADR 0020, item 12): installs
# the app and its instrumented test, gives the app all-files access, pushes the
# demo measurement folder and runs DemoProcessingTest, which processes that
# folder through the app's page and restores a deleted output.
#
#   android_emulator_test.sh <app.apk> <test.apk> <demo folder> <output folder>
#
# The output folder receives the instrumentation report, the device log and
# what the test left on the device (screenshots, result.json).
set -euo pipefail

app=$1
test_apk=$2
demo=$3
out=$4
package=dev.impulcifer.app
device_demo=/sdcard/Download/impulcifer-demo
device_evidence=/sdcard/Download/impulcifer-test

# The emulator's crash reporter can keep the emulator from exiting when the
# action shuts it down: run 43677207306 passed its test in 25 seconds and then
# sat in "Terminate Emulator" for an hour. Stop the app and the reporter on the
# way out, whatever the outcome.
leave() {
  adb shell am force-stop "$package" >/dev/null 2>&1 || true
  killall -INT crashpad_handler >/dev/null 2>&1 || true
}
trap leave EXIT

mkdir -p "$out"
adb wait-for-device
adb install -r "$app"
adb install -r "$test_apk"
# What a person does on the system screen the app opens (request_storage_access).
adb shell appops set "$package" MANAGE_EXTERNAL_STORAGE allow
adb shell rm -rf "$device_demo" "$device_evidence"
adb push "$demo" "$device_demo" >/dev/null
adb shell ls -la "$device_demo"
# The screen stays on: a sleeping device stops the activity and the page with it.
adb shell svc power stayon true
adb shell input keyevent KEYCODE_WAKEUP
adb logcat -c
adb logcat -G 16M

# The runner must not finish the activity when the tests end: the app ends its
# process when its activity is destroyed (MainActivity.onDestroy), and that
# process is the one reporting the result.
# Bounded from outside as well: the longest the test's own waits add up to is
# about 35 minutes, and a run that is cut here still hands over its log, its
# screenshots and the steps it reached.
status=0
timeout --signal=TERM 45m adb shell am instrument -w \
  -e class "$package.DemoProcessingTest" \
  -e waitForActivitiesToComplete false \
  -e demoFolder "$device_demo" -e evidenceFolder "$device_evidence" \
  "$package.test/androidx.test.runner.AndroidJUnitRunner" | tee "$out/instrument.txt" || status=$?

# A moment for a crash report, if there was one, to reach the log.
sleep 5
adb logcat -d > "$out/logcat.txt" || true
grep -E 'ImpulciferTest|RustStdoutStderr|FATAL|ANR in|am_anr|Fatal signal' "$out/logcat.txt" | tail -n 200 > "$out/logcat-app.txt" || true
adb exec-out screencap -p > "$out/screen-at-the-end.png" || true
adb pull "$device_evidence" "$out/evidence" || true
cat "$out/evidence/result.json" 2>/dev/null || true
ls -la "$out" "$out/evidence" 2>/dev/null || true

# `am instrument` exits 0 whatever the tests did: its report decides.
if [ "$status" -ne 0 ] || ! grep -q '^OK (1 test)' "$out/instrument.txt"; then
  echo "::error::the instrumented test did not pass; see instrument.txt and logcat.txt in the evidence artifact"
  exit 1
fi
