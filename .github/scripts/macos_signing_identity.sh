#!/usr/bin/env bash
# The project's self-signed macOS code-signing identity (ADR 0012).
#
# macOS keeps a privacy grant such as the microphone by the app's designated
# requirement (Apple TN3127). Ad hoc signing ties that requirement to one build
# (`cdhash H"..."`), so every update asked again. One fixed certificate gives
# every release `identifier "dev.impulcifer.app" and certificate leaf = H"<SHA-1>"`.
# Gatekeeper still treats the app as coming from an unidentified developer;
# only a Developer ID changes that.
#
# The private key is kept in the repository as a PKCS#12 file. Its password is
# derived from the updater signing secrets (TAURI_SIGNING_PRIVATE_KEY and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD), so no further secret is needed and the
# key exists in clear only inside a CI job. Replacing the updater key means
# generating a new identity (users are asked for the microphone once more).
#
#   generate <dir>     write <dir>/identity.p12 and <dir>/certificate.pem (once, in CI)
#   import <dir>       import identity.p12 into a temporary keychain on a macOS CI
#                      runner, trust it for code signing there (needs sudo) and
#                      print the identity name for APPLE_SIGNING_IDENTITY
#   fingerprint <dir>  print the certificate's SHA-1, as the designated requirement shows it
set -euo pipefail

IDENTITY_NAME="Impulcifer Self-Signed Code Signing"
LABEL="impulcifer macos code signing identity v1"

die() { echo "::error::$*" >&2; exit 1; }

mask() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::add-mask::$1"; fi
}

p12_password() {
  [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || die "TAURI_SIGNING_PRIVATE_KEY is required to derive the signing identity password"
  LABEL="$LABEL" python3 -I - <<'PY'
import hashlib, hmac, os
key = (os.environ["TAURI_SIGNING_PRIVATE_KEY"] + "\n"
       + os.environ.get("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", "")).encode()
print(hmac.new(key, os.environ["LABEL"].encode(), hashlib.sha256).hexdigest())
PY
}

generate() {
  local dir=$1
  [ ! -e "$dir/identity.p12" ] || die "$dir/identity.p12 already exists; delete it on purpose to make a new identity"
  mkdir -p "$dir"
  local work
  work=$(mktemp -d)
  # Expanded now: the key's directory goes even when a later step fails.
  # shellcheck disable=SC2064
  trap "rm -rf '$work'" EXIT
  umask 077
  local password
  password=$(p12_password)
  mask "$password"
  cat > "$work/cert.cnf" <<EOF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $IDENTITY_NAME
[ext]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
subjectKeyIdentifier = hash
EOF
  # Twenty years: codesign refuses an expired certificate, and a new one would
  # change the designated requirement.
  openssl req -x509 -newkey rsa:3072 -nodes -sha256 -days 7305 \
    -keyout "$work/key.pem" -out "$dir/certificate.pem" -config "$work/cert.cnf" 2>/dev/null
  # PBE-SHA1-3DES with a SHA-1 MAC is the PKCS#12 form macOS `security import`
  # reads on every version; the 256-bit password carries the strength.
  P12_PASSWORD="$password" openssl pkcs12 -export -name "$IDENTITY_NAME" \
    -inkey "$work/key.pem" -in "$dir/certificate.pem" \
    -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
    -passout env:P12_PASSWORD -out "$dir/identity.p12"
  chmod 644 "$dir/identity.p12" "$dir/certificate.pem"
  openssl x509 -in "$dir/certificate.pem" -noout -subject -enddate >&2
  echo "SHA-1 $(fingerprint "$dir")" >&2
}

import_identity() {
  local dir=$1
  [ -f "$dir/identity.p12" ] || die "$dir/identity.p12 is missing"
  local password keychain keychain_password
  password=$(p12_password)
  mask "$password"
  keychain="${RUNNER_TEMP:-$(mktemp -d)}/impulcifer-signing.keychain-db"
  keychain_password=$(openssl rand -hex 24)
  mask "$keychain_password"
  security create-keychain -p "$keychain_password" "$keychain"
  security set-keychain-settings "$keychain"
  security unlock-keychain -p "$keychain_password" "$keychain"
  security import "$dir/identity.p12" -k "$keychain" -P "$password" -T /usr/bin/codesign >/dev/null \
    || die "could not import $dir/identity.p12: the updater signing secrets no longer match the ones it was made with. Generate a new identity (ADR 0012)."
  security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain" >/dev/null
  # codesign finds an identity by name only when it is trusted for code
  # signing (an untrusted one is "no identity found" unless --keychain is
  # passed, and tauri-macos-sign passes none). The trust setting lives on this
  # CI machine only; users' Macs never need it, since macOS checks the
  # designated requirement, not the certificate's trust.
  sudo security add-trusted-cert -d -r trustRoot -p codeSign \
    -k /Library/Keychains/System.keychain "$dir/certificate.pem" \
    || die "could not mark the signing certificate as trusted for code signing on this machine"
  # Searched by codesign; the runner's own keychains stay in the list.
  local existing
  existing=$(security list-keychains -d user | tr -d '"')
  # shellcheck disable=SC2086
  security list-keychains -d user -s "$keychain" $existing
  echo "$IDENTITY_NAME"
}

fingerprint() {
  openssl x509 -in "$1/certificate.pem" -noout -fingerprint -sha1 | sed 's/.*=//; s/://g' | tr 'A-F' 'a-f'
}

case "${1:-}" in
  generate) generate "${2:?directory}" ;;
  import) import_identity "${2:?directory}" ;;
  fingerprint) fingerprint "${2:?directory}" ;;
  *) die "usage: $0 generate|import|fingerprint <dir>" ;;
esac
