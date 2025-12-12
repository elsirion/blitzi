# Blitzi: Lightning made easy

<img src="blitzi.png" alt="Blitzi logo" align="right" width="300">

[![crates.io](https://img.shields.io/crates/v/blitzi.svg)](https://crates.io/crates/blitzi)
[![docs.rs](https://docs.rs/blitzi/badge.svg)](https://docs.rs/blitzi)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Easy to use Bitcoin Lighning client that uses [Fedimint](https://fedimint.org/) as its backend.

You want to build a lightning powered app but don't look forward to dealing
with the complexity of running your own Lightning node? Blitzi is for you!
With Blitzi you can outsource the infrastructure to any Fedimint federation
of your choosing (or just go with the default for small amounts) and receive
and send Lightning payments without any hassle.

## Blitzid - REST API Server

For non-Rust applications, Blitzi provides `blitzid`, a standalone binary that exposes the same functionality as the library via a REST API. This allows you to use Blitzi from any programming language.

See [BLITZID.md](BLITZID.md) for detailed documentation on building, running, and using the REST API.

## Examples

The fastest way to get started is to create a new Blitzi client with default
settings. This is only advisable for small amounts since it will use a
default Fedimint federation which the author of this library trusts, but
ultimately can't guarantee the security of. For larger amounts we recommend
making your own choice which federation to use based on your own due
diligence.

```rust
# use anyhow::Result;
use blitzi::Blitzi;

# #[tokio::main]
# async fn main() -> Result<()> {
// Create a new Blitzi client with default settings
let blitzi = Blitzi::new().await?;

// Generate a new Lightning invoice for 1000 millisatoshi and await its payment
let invoice = blitzi.lightning_invoice(1000, "Test payment").await?;
println!("Invoice: {}", invoice);

match blitzi.await_incoming_payment(&invoice).await {
    Ok(()) => println!("Payment received"),
    Err(_) => println!("Invoice expired"),
}

# Ok(())
# }
```

## Fedimint

Blitzi uses Fedimint, an open source federated ecash mint implementation on
Bitcoin, to connect you to the Lighning network. Federated in this context
means that each federation is run by a group of people, also called
guardians, who are jointly responsible for the security of the funds held in
the federation. This means, while no signle guardian can steal your funds,
if a majority of the guardians are compromised, the funds are at risk, so
chose your federation wisely.

The default federation used by Blitzi is [E-Cash Club], which for various
reasons seems the most reasonable choice at the time of writing (long run
time, multiple ASNs, etc.). For anything but toy amounts users should make
their own choice though. You can find a list of publicly known federations
on [Fedimint Observer], which also provices statistics and uptime statistics
about them.

[E-Cash Club]: (https://observer.fedimint.org/federations/aeca6cc80ffc530bd2d54b09681f6edb9a415c362e4af2fe3d5e04137006fa21)
[Fedimint Observer]: (https://observer.fedimint.org/)

## About the name

Lightning bolts are called "Blitz" in German and adding an "i" at the end
makes it sound cute and wholesome for me :D
