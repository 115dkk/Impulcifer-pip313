#!/usr/bin/env bash
# The project's Android APK signing key (ADR 0020, item 11).
#
# Android installs an update only over an app signed with the same key, so
# every release must be signed with one key for good. The key is kept in the
# repository as a PKCS#12 store, as the macOS identity is (ADR 0012): its
# password is derived from the updater signing secrets
# (TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD), so no
# further secret is needed and the key exists in clear only inside a CI job.
# Replacing the updater secrets without first re-encrypting this store loses
# the key: a release signed with a new one does not install over the old app.
#
#   generate <dir>              write <dir>/release.p12 and <dir>/certificate.pem
#                               (once, in CI)
#   sign <dir> <in.apk> <out>   align <in.apk>, sign it with the key into <out>
#                               and check the result (needs ANDROID_BUILD_TOOLS,
#                               the SDK's build-tools/<version> folder)
#   check <dir> <apk>           fail unless <apk> is signed by the certificate in
#                               <dir> and by nothing else
#   fingerprint <dir>           print the certificate's SHA-256 as apksigner
#                               prints it (lower-case hex)
#
# Never capture the output of `generate` or `sign` (no `$(...)`): their stdout
# carries the ::add-mask:: command, and a captured mask command is never seen
# by the runner, so the password it should hide would end up in the log.
set -euo pipefail

ALIAS="impulcifer"
LABEL="impulcifer android apk signing key v1"
SUBJECT="/CN=Impulcifer/O=Impulcifer-pip313"

die() { echo "::error::$*" >&2; exit 1; }

# Registers a secret with the runner. Only effective on the step's own stdout.
mask() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::add-mask::$1"; fi
}

store_password() {
  [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || die "TAURI_SIGNING_PRIVATE_KEY is required to derive the signing key password"
  LABEL="$LABEL" python3 -I - <<'PY'
import hashlib, hmac, os
key = (os.environ["TAURI_SIGNING_PRIVATE_KEY"] + "\n"
       + os.environ.get("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "")).encode()
print(hmac.new(key, os.environ["LABEL"].encode(), hashlib.sha256).hexdigest())
PY
}

tool() {
  [ -n "${ANDROID_BUILD_TOOLS:-}" ] || die "ANDROID_BUILD_TOOLS must name the SDK's build-tools folder"
  [ -x "$ANDROID_BUILD_TOOLS/$1" ] || die "$ANDROID_BUILD_TOOLS/$1 is missing"
  echo "$ANDROID_BUILD_TOOLS/$1"
}

fingerprint() {
  local dir=$1
  [ -f "$dir/certificate.pem" ] || die "$dir/certificate.pem is missing"
  openssl x509 -in "$dir/certificate.pem" -noout -fingerprint -sha256 \
    | sed 's/^.*=//' | tr -d ':' | tr 'A-F' 'a-f'
}

generate() {
  local dir=$1
  [ ! -e "$dir/release.p12" ] || die "$dir/release.p12 already exists; a new key cannot update an installed app"
  mkdir -p "$dir"
  local work
  work=$(mktemp -d)
  # Expanded now: the key's directory goes even when a later step fails.
  # shellcheck disable=SC2064
  trap "rm -rf '$work'" EXIT
  umask 077
  local password
  password=$(store_password)
  mask "$password"
  # Thirty years: Android checks an APK's certificate only against the one
  # already installed, but tools refuse to sign with an expired one.
  openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 10957 \
    -subj "$SUBJECT" -keyout "$work/key.pem" -out "$dir/certificate.pem" 2>/dev/null
  STORE_PASSWORD="$password" openssl pkcs12 -export -name "$ALIAS" \
    -inkey "$work/key.pem" -in "$dir/certificate.pem" \
    -passout env:STORE_PASSWORD -out "$dir/release.p12"
  chmod 644 "$dir/release.p12" "$dir/certificate.pem"
  openssl x509 -in "$dir/certificate.pem" -noout -subject -enddate >&2
  echo "SHA-256 $(fingerprint "$dir")" >&2
}

# The signers apksigner finds, one lower-case SHA-256 per line.
signers() {
  local apksigner
  apksigner=$(tool apksigner)
  "$apksigner" verify --print-certs "$1" \
    | sed -n 's/^Signer #[0-9]* certificate SHA-256 digest: //p'
}

check() {
  local dir=$1 apk=$2
  [ -f "$apk" ] || die "$apk is missing"
  local expected found
  expected=$(fingerprint "$dir")
  found=$(signers "$apk")
  [ "$found" = "$expected" ] \
    || die "$apk is not signed by the project key alone (expected $expected, found: ${found:-nothing})"
  echo "$apk is signed by $expected" >&2
}

sign() {
  local dir=$1 input=$2 output=$3
  [ -f "$dir/release.p12" ] || die "$dir/release.p12 is missing"
  [ -f "$input" ] || die "$input is missing"
  local zipalign apksigner password aligned
  zipalign=$(tool zipalign)
  apksigner=$(tool apksigner)
  password=$(store_password)
  mask "$password"
  mkdir -p "$(dirname "$output")"
  aligned=$(mktemp --suffix=.apk)
  # shellcheck disable=SC2064
  trap "rm -f '$aligned'" EXIT
  # 16 KiB pages for the native library, 4 bytes for everything else.
  "$zipalign" -P 16 -f 4 "$input" "$aligned"
  STORE_PASSWORD="$password" "$apksigner" sign \
    --ks "$dir/release.p12" --ks-type PKCS12 --ks-key-alias "$ALIAS" \
    --ks-pass env:STORE_PASSWORD --key-pass env:STORE_PASSWORD \
    --v2-signing-enabled true --v3-signing-enabled true \
    --out "$output" "$aligned" \
    || die "could not sign with $dir/release.p12: the updater signing secrets no longer match the ones it was made with (ADR 0020)"
  rm -f "$output.idsig"
  check "$dir" "$output"
}

command=${1:-}
case "$command" in
  generate) [ $# -eq 2 ] || die "usage: $0 generate <dir>"; generate "$2" ;;
  sign) [ $# -eq 4 ] || die "usage: $0 sign <dir> <in.apk> <out.apk>"; sign "$2" "$3" "$4" ;;
  check) [ $# -eq 3 ] || die "usage: $0 check <dir> <apk>"; check "$2" "$3" ;;
  fingerprint) [ $# -eq 2 ] || die "usage: $0 fingerprint <dir>"; fingerprint "$2" ;;
  *) die "usage: $0 {generate <dir>|sign <dir> <in.apk> <out.apk>|check <dir> <apk>|fingerprint <dir>}" ;;
esac
