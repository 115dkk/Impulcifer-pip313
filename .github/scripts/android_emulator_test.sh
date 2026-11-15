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

mkdir -p "$out"
adb wait-for-device
adb install -r "$app"
adb install -r "$test_apk"
# What a person does on the system screen the app opens (request_storage_access).
adb shell appops set "$package" MANAGE_EXTERNAL_STORAGE allow
adb shell rm -rf "$device_demo" "$device_evidence"
adb push "$demo" "$device_demo" >/dev/null
adb shell ls -la "$device_demo"
adb logcat -c

status=0
adb shell am instrument -w \
  -e class "$package.DemoProcessingTest" \
  -e demoFolder "$device_demo" -e evidenceFolder "$device_evidence" \
  "$package.test/androidx.test.runner.AndroidJUnitRunner" | tee "$out/instrument.txt" || status=$?

adb logcat -d > "$out/logcat.txt" || true
adb pull "$device_evidence" "$out/evidence" || true
ls -la "$out" "$out/evidence" 2>/dev/null || true

# `am instrument` exits 0 whatever the tests did: its report decides.
if [ "$status" -ne 0 ] || ! grep -q '^OK (1 test)' "$out/instrument.txt"; then
  echo "::error::the instrumented test did not pass; see instrument.txt and logcat.txt in the evidence artifact"
  exit 1
fi
