use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use bitcoin::Network;
use blitzi::{Amount, Blitzi};
use clap::Parser;
use fedimint_core::BitcoinHash;
use lightning_invoice::{Bolt11Invoice, Bolt11InvoiceDescriptionRef};
use secp256k1::PublicKey;
use secp256k1::hashes::sha256;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "blitzid")]
#[command(about = "Blitzi Lightning REST API daemon", long_about = None)]
struct Args {
    #[arg(short, long, env = "BLITZID_DATADIR")]
    #[arg(help = "Directory where Fedimint data will be stored")]
    datadir: Option<String>,

    #[arg(short, long, env = "BLITZID_FEDERATION")]
    #[arg(help = "Federation invite code to connect to")]
    federation: Option<String>,

    #[arg(short, long, env = "BLITZID_BEARER_TOKEN")]
    #[arg(help = "Bearer token for authentication (auto-generated if not provided)")]
    bearer_token: Option<String>,

    #[arg(short, long, env = "BLITZID_PORT", default_value = "3000")]
    #[arg(help = "Port to listen on")]
    port: u16,

    #[arg(short = 'H', long, env = "BLITZID_HOST", default_value = "127.0.0.1")]
    #[arg(help = "Host to bind to")]
    host: String,
}

#[derive(Clone)]
struct AppState {
    blitzi: Arc<Blitzi>,
    bearer_token: String,
}

#[derive(Serialize, Deserialize)]
struct CreateInvoiceRequest {
    amount_msats: u64,
    description: String,
}

#[derive(Serialize, Deserialize)]
struct CreateInvoiceResponse {
    invoice: String,
    payment_hash: String,
}

#[derive(Serialize, Deserialize)]
struct PayInvoiceRequest {
    invoice: String,
}

#[derive(Serialize, Deserialize)]
struct PayInvoiceResponse {
    preimage: String,
}

#[derive(Serialize, Deserialize)]
struct BalanceResponse {
    balance_msats: u64,
}

#[derive(Serialize, Deserialize)]
struct InvoiceStatusResponse {
    paid: bool,
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize, Deserialize)]
struct DecodeInvoiceResponse {
    amount_msats: Option<u64>,
    description: Option<String>,
    payee_pubkey: PublicKey,
    payment_hash: sha256::Hash,
    expiry_seconds: u64,
    timestamp: u64,
    network: Network,
}

async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(auth) if auth == format!("Bearer {}", state.bearer_token) => {
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

async fn create_invoice(
    State(state): State<AppState>,
    Json(payload): Json<CreateInvoiceRequest>,
) -> Result<Json<CreateInvoiceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let amount = Amount::from_msats(payload.amount_msats);

    match state
        .blitzi
        .lightning_invoice(amount, &payload.description)
        .await
    {
        Ok(invoice) => {
            let payment_hash = hex::encode(invoice.payment_hash().to_byte_array());
            Ok(Json(CreateInvoiceResponse {
                invoice: invoice.to_string(),
                payment_hash,
            }))
        }
        Err(e) => {
            error!("Failed to create invoice: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create invoice: {}", e),
                }),
            ))
        }
    }
}

async fn pay_invoice(
    State(state): State<AppState>,
    Json(payload): Json<PayInvoiceRequest>,
) -> Result<Json<PayInvoiceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let invoice = match payload.invoice.parse() {
        Ok(inv) => inv,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid invoice: {}", e),
                }),
            ));
        }
    };

    match state.blitzi.pay(&invoice).await {
        Ok(preimage) => Ok(Json(PayInvoiceResponse {
            preimage: hex::encode(preimage),
        })),
        Err(e) => {
            error!("Failed to pay invoice: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to pay invoice: {}", e),
                }),
            ))
        }
    }
}

async fn get_balance(
    State(state): State<AppState>,
) -> Result<Json<BalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let balance = state.blitzi.balance().await;
    Ok(Json(BalanceResponse {
        balance_msats: balance.msats,
    }))
}

/// Checks if an invoice has been paid by waiting for payment.
///
/// Note: This endpoint blocks until the invoice is paid or times out, which is
/// intentional behavior. Clients should use appropriate HTTP timeouts.
async fn check_invoice(
    State(state): State<AppState>,
    Path(payment_hash): Path<String>,
) -> Result<Json<InvoiceStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let payment_hash_bytes = match hex::decode(&payment_hash) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid payment hash: {}", e),
                }),
            ));
        }
    };

    if payment_hash_bytes.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Payment hash must be 32 bytes".to_string(),
            }),
        ));
    }

    let mut hash_array = [0u8; 32];
    hash_array.copy_from_slice(&payment_hash_bytes);
    let payment_hash_obj =
        fedimint_core::bitcoin::hashes::sha256::Hash::from_byte_array(hash_array);

    match state
        .blitzi
        .await_incoming_payment_by_hash(&payment_hash_obj)
        .await
    {
        Ok(()) => Ok(Json(InvoiceStatusResponse { paid: true })),
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("No operation found") {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Invoice not found or not issued by this server".to_string(),
                    }),
                ))
            } else if error_msg.contains("canceled") {
                Ok(Json(InvoiceStatusResponse { paid: false }))
            } else {
                error!("Error checking invoice status: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to check invoice status: {}", e),
                    }),
                ))
            }
        }
    }
}

async fn decode_invoice(
    Path(invoice): Path<String>,
) -> Result<Json<DecodeInvoiceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let inv = match Bolt11Invoice::from_str(&invoice) {
        Ok(inv) => inv,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid lightning invoice: {}", e),
                }),
            ));
        }
    };

    let description = match inv.description() {
        Bolt11InvoiceDescriptionRef::Direct(d) => Some(d.to_string()),
        Bolt11InvoiceDescriptionRef::Hash(_) => None,
    };

    let response = DecodeInvoiceResponse {
        amount_msats: inv.amount_milli_satoshis(),
        description: description,
        payee_pubkey: inv.get_payee_pub_key(),
        payment_hash: *inv.payment_hash(),
        expiry_seconds: inv.expiry_time().as_secs(),
        timestamp: inv
            .timestamp()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        network: inv.network(),
    };

    Ok(Json(response))
}

async fn health_check() -> &'static str {
    "OK"
}

fn generate_bearer_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const TOKEN_LEN: usize = 32;
    let mut rng = rand::thread_rng();

    (0..TOKEN_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let bearer_token = args.bearer_token.unwrap_or_else(|| {
        let token = generate_bearer_token();
        info!("Generated bearer token: {}", token);
        token
    });

    info!("Initializing Blitzi client...");
    let mut builder = Blitzi::builder();

    if let Some(datadir) = args.datadir {
        builder = builder.datadir(datadir);
    }

    if let Some(federation) = args.federation {
        builder = builder
            .federation(&federation)
            .context("Invalid federation invite code")?;
    }

    let blitzi = builder
        .build()
        .await
        .context("Failed to build Blitzi client")?;
    info!("Blitzi client initialized successfully");

    let state = AppState {
        blitzi: Arc::new(blitzi),
        bearer_token: bearer_token.clone(),
    };

    let protected_routes = Router::new()
        .route("/invoice", post(create_invoice))
        .route("/invoice/decode/:invoice", get(decode_invoice))
        .route("/invoice/:payment_hash", get(check_invoice))
        .route("/pay", post(pay_invoice))
        .route("/balance", get(get_balance))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(protected_routes)
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    info!("Starting server on {}", addr);
    info!("Use Authorization header: Bearer {}", bearer_token);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to bind to address")?;

    axum::serve(listener, app).await.context("Server error")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bearer_token() {
        let token = generate_bearer_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_generate_bearer_token_uniqueness() {
        let token1 = generate_bearer_token();
        let token2 = generate_bearer_token();
        assert_ne!(token1, token2, "Generated tokens should be unique");
    }
}
