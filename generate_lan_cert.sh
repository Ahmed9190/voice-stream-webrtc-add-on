#!/bin/bash
#
# SSL Certificate Generator for LAN Access
# Generates self-signed certificates with Subject Alternative Names for LAN IPs
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SSL_DIR="${SCRIPT_DIR}"

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     SSL Certificate Generator for LAN Production           ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to detect LAN IP
detect_lan_ip() {
    local ip=""
    
    # Method 1: ip route (most reliable on Linux)
    if command -v ip &> /dev/null; then
        ip=$(ip route get 1.1.1.1 2>/dev/null | grep -oP 'src \K\S+' || true)
    fi
    
    # Method 2: hostname -I (fallback)
    if [ -z "$ip" ] && command -v hostname &> /dev/null; then
        ip=$(hostname -I 2>/dev/null | awk '{print $1}' || true)
    fi
    
    # Method 3: ifconfig (older systems)
    if [ -z "$ip" ] && command -v ifconfig &> /dev/null; then
        ip=$(ifconfig 2>/dev/null | grep -Eo 'inet (addr:)?([0-9]*\.){3}[0-9]*' | grep -v '127.0.0.1' | awk '{print $2}' | head -1 || true)
    fi
    
    echo "$ip"
}

# Function to get hostname
get_hostname() {
    hostname 2>/dev/null || echo "homeassistant"
}

# ... (Keep detection logic)

# Detect LAN IP
echo -e "${YELLOW}🔍 Detecting network configuration...${NC}"
LAN_IP=$(detect_lan_ip)
HOSTNAME=$(get_hostname)

if [ -z "$LAN_IP" ]; then
    echo -e "${RED}❌ Could not detect LAN IP address automatically. Defaulting to 127.0.0.1${NC}"
    LAN_IP="127.0.0.1"
fi

echo -e "${GREEN}✓ Detected LAN IP: ${LAN_IP}${NC}"
echo -e "${GREEN}✓ Hostname: ${HOSTNAME}${NC}"

# Target Directory
TARGET_DIR="/data/ssl"
mkdir -p "$TARGET_DIR"

# Build the SAN list
SAN_IPS="IP.1 = 127.0.0.1\nIP.2 = ${LAN_IP}"

# Create OpenSSL config file
OPENSSL_CONFIG="${TARGET_DIR}/openssl_lan.cnf"

cat > "$OPENSSL_CONFIG" << EOF
[req]
default_bits = 2048
prompt = no
default_md = sha256
distinguished_name = dn
req_extensions = v3_req
x509_extensions = v3_ca

[dn]
C = US
ST = Local
L = Local
O = Home Assistant
OU = Voice Streaming
CN = ${LAN_IP}

[v3_req]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[v3_ca]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
DNS.2 = homeassistant.local
DNS.3 = ${HOSTNAME}
DNS.4 = ${HOSTNAME}.local
$(echo -e "$SAN_IPS")
EOF

# Generate new certificate (always overwrite for fresh container start or ensure validity)
echo -e "${YELLOW}🔐 Generating new SSL certificate...${NC}"

openssl req -x509 \
    -nodes \
    -days 365 \
    -newkey rsa:2048 \
    -keyout "${TARGET_DIR}/key.pem" \
    -out "${TARGET_DIR}/cert.pem" \
    -config "$OPENSSL_CONFIG" \
    -extensions v3_ca \
    2>/dev/null

echo -e "${GREEN}✓ Certificate generated successfully in ${TARGET_DIR}!${NC}"
rm "$OPENSSL_CONFIG"
