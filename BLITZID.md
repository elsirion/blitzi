# Blitzid - REST API Server

Blitzid is a standalone binary that exposes the Blitzi library functionality as a REST API, allowing it to be used from any programming language.

## Building

### Using Nix (Recommended)

The project provides a Nix development shell with the correct compiler version and dependencies:

```bash
# Install Nix (if not already installed)
sh <(curl -L https://nixos.org/nix/install) --daemon

# Enter the development shell
nix develop

# Build the binary
cargo build --release --bin blitzid
```

### Using Standard Rust Toolchain

If you have a compatible Rust toolchain installed (stable, with RocksDB system dependencies), you can build without Nix:

```bash
# Make sure you have required system dependencies (e.g., on Ubuntu/Debian):
sudo apt-get install pkg-config cmake clang libclang-dev

# Build the binary
cargo build --release --bin blitzid
```

The binary will be available at `target/release/blitzid`.

## Configuration

Blitzid can be configured via environment variables or command-line arguments:

| CLI Flag | Environment Variable | Description | Default |
|----------|---------------------|-------------|---------|
| `-d, --datadir` | `BLITZID_DATADIR` | Directory where Fedimint data will be stored | `$XDG_DATA_HOME/fedimint/default` |
| `-f, --federation` | `BLITZID_FEDERATION` | Federation invite code to connect to | E-Cash Club invite |
| `-b, --bearer-token` | `BLITZID_BEARER_TOKEN` | Bearer token for authentication | Auto-generated |
| `-p, --port` | `BLITZID_PORT` | Port to listen on | 3000 |
| `-h, --host` | `BLITZID_HOST` | Host to bind to | 127.0.0.1 |

## Running

### Basic Usage

```bash
# Run with default settings (auto-generates bearer token)
blitzid

# Run with custom configuration
blitzid --port 8080 --bearer-token "mysecrettoken123"

# Run with environment variables
export BLITZID_PORT=8080
export BLITZID_BEARER_TOKEN="mysecrettoken123"
blitzid
```

When started, blitzid will print the bearer token to the console:

```
Generated bearer token: abc123xyz789...
Starting server on 127.0.0.1:3000
Use Authorization header: Bearer abc123xyz789...
```

## Logging

Blitzid uses `tracing-subscriber` for logging. You can control the log level using the `RUST_LOG` environment variable:

```bash
# Info level (default)
RUST_LOG=info blitzid

# Debug level
RUST_LOG=debug blitzid

# Trace level for specific modules
RUST_LOG=blitzid=trace,axum=debug blitzid
```

## API Endpoints

All endpoints except `/health` require bearer token authentication via the `Authorization` header:

```
Authorization: Bearer <your-token>
```

### Health Check

**GET /health**

Returns server health status. No authentication required.

**Response:**
```
OK
```

### Get Balance

**GET /balance**

Returns the current balance in millisatoshi.

**Response:**
```json
{
  "balance_msats": 1000000
}
```

### Create Invoice

**POST /invoice**

Creates a new Lightning invoice.

**Request:**
```json
{
  "amount_msats": 1000,
  "description": "Test payment"
}
```

**Response:**
```json
{
  "invoice": "lnbc10n1...",
  "payment_hash": "abcd1234..."
}
```

### Check Invoice Status

**GET /invoice/:payment_hash**

Waits for an invoice to be paid and returns the payment status. 

**⚠️ Note:** This endpoint blocks until the invoice is paid or times out. It's designed to be used with long HTTP timeouts (e.g., 5+ minutes) or in a polling scenario. The payment_hash should be a 32-byte hex-encoded hash obtained from the create invoice endpoint.

**Response:**
```json
{
  "paid": true
}
```

**Error Responses:**
- `404 NOT FOUND`: Invoice not found or not issued by this server
- `400 BAD REQUEST`: Invalid payment hash format
- `500 INTERNAL_SERVER_ERROR`: Server error while checking status

### Pay Invoice

**POST /pay**

Pays a Lightning invoice.

**Request:**
```json
{
  "invoice": "lnbc10n1..."
}
```

**Response:**
```json
{
  "preimage": "abcd1234..."
}
```

## Example Usage

### Using curl

```bash
# Set your bearer token
TOKEN="your-bearer-token-here"

# Check balance
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/balance

# Create an invoice
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"amount_msats": 1000, "description": "Test"}' \
  http://localhost:3000/invoice

# Pay an invoice
curl -X POST \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"invoice": "lnbc10n1..."}' \
  http://localhost:3000/pay
```

### Using Python

```python
import requests

BASE_URL = "http://localhost:3000"
TOKEN = "your-bearer-token-here"
HEADERS = {"Authorization": f"Bearer {TOKEN}"}

# Get balance
response = requests.get(f"{BASE_URL}/balance", headers=HEADERS)
print(response.json())

# Create invoice
invoice_data = {
    "amount_msats": 1000,
    "description": "Test payment"
}
response = requests.post(f"{BASE_URL}/invoice", json=invoice_data, headers=HEADERS)
print(response.json())

# Pay invoice
pay_data = {
    "invoice": "lnbc10n1..."
}
response = requests.post(f"{BASE_URL}/pay", json=pay_data, headers=HEADERS)
print(response.json())
```

### Using JavaScript/Node.js

```javascript
const BASE_URL = "http://localhost:3000";
const TOKEN = "your-bearer-token-here";
const headers = {
  "Authorization": `Bearer ${TOKEN}`,
  "Content-Type": "application/json"
};

// Get balance
fetch(`${BASE_URL}/balance`, { headers })
  .then(res => res.json())
  .then(console.log);

// Create invoice
fetch(`${BASE_URL}/invoice`, {
  method: "POST",
  headers,
  body: JSON.stringify({
    amount_msats: 1000,
    description: "Test payment"
  })
})
  .then(res => res.json())
  .then(console.log);

// Pay invoice
fetch(`${BASE_URL}/pay`, {
  method: "POST",
  headers,
  body: JSON.stringify({
    invoice: "lnbc10n1..."
  })
})
  .then(res => res.json())
  .then(console.log);
```

## Security Considerations

1. **Bearer Token**: Keep your bearer token secure. Anyone with the token can access your Lightning wallet.
2. **Network Binding**: By default, blitzid binds to `127.0.0.1` (localhost). If you need to expose it over a network, consider:
   - Using a reverse proxy with TLS (e.g., nginx, caddy)
   - Implementing additional security measures (firewall rules, VPN, etc.)
3. **Data Directory**: Ensure the data directory has appropriate file permissions to protect your wallet data.

## Troubleshooting

### Port Already in Use
```
Error: Failed to bind to address: Address already in use
```
Solution: Use a different port with `--port` flag or stop the process using the current port.

### Authentication Failed
```
401 Unauthorized
```
Solution: Verify that you're sending the correct bearer token in the Authorization header.

### Failed to Initialize Blitzi Client
```
Error: Failed to build Blitzi client
```
Solution: Check that you have network connectivity and the federation invite code is valid.
