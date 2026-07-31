#!/usr/bin/env bash
# One-time setup: create a local self-signed code signing certificate.
# This ensures macOS TCC recognises the app across builds and updates.
#
# Run: bash scripts/setup-codesign.sh
# (No $99 Apple Developer account required.)

set -euo pipefail

CERT_NAME="WorkReview Self-Signed"
KEYCHAIN=~/Library/Keychains/login.keychain-db

cleanup() {
    rm -f /tmp/wr_key.pem /tmp/wr_cert.pem /tmp/wr_cert.p12
}
trap cleanup EXIT

if security find-certificate -c "$CERT_NAME" "$KEYCHAIN" &>/dev/null; then
    security find-certificate -c "$CERT_NAME" -p "$KEYCHAIN" > /tmp/wr_cert.pem
    security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" /tmp/wr_cert.pem
    if security find-identity -v -p codesigning "$KEYCHAIN" | grep -Fq "\"$CERT_NAME\""; then
        echo "[OK] Certificate '$CERT_NAME' is already installed and trusted."
        exit 0
    fi
    echo "[ERROR] Certificate '$CERT_NAME' exists but is not a valid code-signing identity."
    exit 1
fi

echo "[1/3] Generating self-signed certificate (valid 10 years)..."

openssl req -x509 \
    -newkey rsa:2048 \
    -keyout /tmp/wr_key.pem \
    -out /tmp/wr_cert.pem \
    -days 3650 \
    -nodes \
    -subj "/CN=$CERT_NAME/O=WorkReview/C=CN" \
    -config <(cat <<'EOF'
[req]
distinguished_name = req_dn
prompt = no
x509_extensions = v3_cs

[req_dn]
CN = WorkReview Self-Signed
O = WorkReview
C = CN

[v3_cs]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
subjectKeyIdentifier = hash
EOF
    ) 2>/dev/null

echo "[2/3] Importing into login keychain..."

PKCS12_ARGS=()
if openssl pkcs12 -help 2>&1 | grep -- '-legacy' >/dev/null; then
    PKCS12_ARGS=(-legacy)
fi
openssl pkcs12 -export "${PKCS12_ARGS[@]}" \
    -out /tmp/wr_cert.p12 \
    -inkey /tmp/wr_key.pem \
    -in /tmp/wr_cert.pem \
    -passout pass:workreview 2>/dev/null

security import /tmp/wr_cert.p12 \
    -k "$KEYCHAIN" \
    -P workreview \
    -T /usr/bin/codesign 2>&1
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" /tmp/wr_cert.pem

rm -f /tmp/wr_key.pem /tmp/wr_cert.pem /tmp/wr_cert.p12

echo "[3/3] Verifying..."
if security find-identity -v -p codesigning "$KEYCHAIN" | grep -Fq "\"$CERT_NAME\""; then
    echo "[OK] Certificate installed. Tauri builds will now use stable code signing."
    echo "     tauri.conf.json signingIdentity should be: \"$CERT_NAME\""
else
    echo "[ERROR] Certificate is not a valid code-signing identity."
    exit 1
fi
