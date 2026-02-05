# Blitzid - REST API Server

Blitzid is a standalone binary that exposes the Blitzi library functionality as a REST API, allowing it to be used from any programming language.

## Running with Docker (Recommended)

The easiest way to run blitzid is via Docker.

> **WARNING: Data Persistence is Critical for Fund Safety**
>
> Blitzid stores your Fedimint wallet data in the data directory. **If you lose this data, you lose access to your funds.** Always attach a persistent volume to the `/data` directory to ensure your wallet data survives container restarts and updates.

### Docker Run

```bash
docker run -d \
  --name blitzid \
  -p 3000:3000 \
  -v blitzid-data:/data \
  -e BLITZID_BEARER_TOKEN="your-secure-token-here" \
  ghcr.io/elsirion/blitzid:latest
```

### Docker Compose (Recommended)

Create a `docker-compose.yml` file:

```yaml
services:
  blitzid:
    image: ghcr.io/elsirion/blitzid:latest
    container_name: blitzid
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      - BLITZID_BEARER_TOKEN=your-secure-token-here
      # Optional: connect to a specific federation
      # - BLITZID_FEDERATION=fed11...
    volumes:
      # IMPORTANT: This volume persists your wallet data.
      # Without it, you WILL lose funds on container recreation.
      - blitzid-data:/data

volumes:
  blitzid-data:
```

Then run:

```bash
docker compose up -d

# View logs to get the bearer token (if not set via environment)
docker compose logs blitzid
```

## Configuration

Blitzid can be configured via environment variables or command-line arguments:

| CLI Flag | Environment Variable | Description | Default |
|----------|---------------------|-------------|---------|
| `-d, --datadir` | `BLITZID_DATADIR` | Directory where Fedimint data will be stored | `$XDG_DATA_HOME/fedimint/default` |
| `-f, --federation` | `BLITZID_FEDERATION` | Federation invite code to connect to | E-Cash Club invite |
| `-b, --bearer-token` | `BLITZID_BEARER_TOKEN` | Bearer token for authentication | Auto-generated |
| `-p, --port` | `BLITZID_PORT` | Port to listen on | 3000 |
| `-h, --host` | `BLITZID_HOST` | Host to bind to | 127.0.0.1 |

## Running from Binary

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

### Decode Invoice

**GET /invoice/decode/:invoice**

Decodes a Lightning invoice and returns its details without paying it. This is useful for displaying invoice information to users before payment.

**Response:**
```json
{
  "amount_msats": 1000,
  "description": "Test payment",
  "payee_pubkey": "02a1b2c3...",
  "payment_hash": "abcd1234...",
  "expiry_seconds": 3600,
  "timestamp": 1706140800,
  "network": "bitcoin"
}
```

**Field Descriptions:**
- `amount_msats`: Amount in millisatoshi (null if not specified in invoice)
- `description`: Human-readable description (null if using description hash)
- `payee_pubkey`: Public key of the payment recipient
- `payment_hash`: Payment hash for the invoice
- `expiry_seconds`: Invoice expiry duration in seconds
- `timestamp`: Unix timestamp when invoice was created
- `network`: Bitcoin network (bitcoin, testnet, regtest, or signet)

**Error Responses:**
- `400 BAD REQUEST`: Invalid lightning invoice format

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

For a complete Python example client, see [examples/blitzid_client.py](examples/blitzid_client.py).

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

# Decode an invoice
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:3000/invoice/decode/lnbc10n1...

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

# Decode invoice
response = requests.get(f"{BASE_URL}/invoice/decode/lnbc10n1...", headers=HEADERS)
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

// Decode invoice
fetch(`${BASE_URL}/invoice/decode/lnbc10n1...`, { headers })
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
