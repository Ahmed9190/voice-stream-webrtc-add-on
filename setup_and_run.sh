#!/bin/bash
set -e

# Configuration
SSL_DIR="ssl"
CERT_FILE="$SSL_DIR/cert.pem"
KEY_FILE="$SSL_DIR/key.pem"

# Ensure SSL directory exists
mkdir -p "$SSL_DIR"

# 1. Detect Local LAN IP
# Tries to find the primary IP address (ignoring loopback)
detect_ip() {
    # Linux detection
    ip addr show | grep -inet | grep -v 127.0.0.1 | grep -v inet6 | awk '{print $2}' | cut -d/ -f1 | head -n1
}

LAN_IP=$(detect_ip)

if [ -z "$LAN_IP" ]; then
    echo "⚠️  Could not detect LAN IP automatically. Defaulting to 127.0.0.1"
    LAN_IP="127.0.0.1"
fi

echo "🔍 Detected System IP: $LAN_IP"

# 2. Check if Certificate Exists and is Valid for this IP
GENERATE_NEW=true

if [ -f "$CERT_FILE" ] && [ -f "$KEY_FILE" ]; then
    # Check if the existing cert is valid for the detected IP
    # We look for "IP Address:192.168.x.x" in the cert details
    if openssl x509 -in "$CERT_FILE" -noout -text | grep -q "IP Address:$LAN_IP"; then
        echo "✅ Existing certificate is valid for $LAN_IP. Keeping it."
        GENERATE_NEW=false
    else
        echo "⚠️  Existing certificate is NOT for $LAN_IP (IP changed?). Regenerating..."
        # Optional: Backup old certs
        mv "$CERT_FILE" "$CERT_FILE.bak"
        mv "$KEY_FILE" "$KEY_FILE.bak"
    fi
else
    echo "🔒 No certificates found."
fi

# 3. Generate Certificate if needed
if [ "$GENERATE_NEW" = true ]; then
    echo "⚙️  Generating new self-signed certificate for $LAN_IP..."

    # Create OpenSSL config file
    cat > "$SSL_DIR/openssl.cnf" <<EOF
[req]
distinguished_name = req_distinguished_name
req_extensions = v3_req
prompt = no

[req_distinguished_name]
C = US
ST = Dev
L = Home
O = WebRTC
CN = $LAN_IP

[v3_req]
keyUsage = keyEncipherment, dataEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
IP.1 = $LAN_IP
EOF

    # Generate Key and Certificate
    openssl req -new -newkey rsa:2048 -days 365 -nodes -x509 \
        -keyout "$KEY_FILE" -out "$CERT_FILE" \
        -config "$SSL_DIR/openssl.cnf" > /dev/null 2>&1

    # Cleanup config
    rm "$SSL_DIR/openssl.cnf"

    echo "✅ New certificate generated in $SSL_DIR/"
fi

# 4. Run the Project
echo "🚀 Building and starting WebRTC Server..."
# Pass remaining arguments to cargo run if any
cargo run --release -- "$@"
