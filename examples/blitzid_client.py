#!/usr/bin/env python3
"""
Example Python client for the Blitzid REST API.

Usage:
    python examples/blitzid_client.py --token YOUR_BEARER_TOKEN
"""

import argparse
import requests
import sys
import time


class BlitzidClient:
    def __init__(self, base_url: str, bearer_token: str):
        self.base_url = base_url
        self.headers = {
            "Authorization": f"Bearer {bearer_token}",
            "Content-Type": "application/json"
        }
    
    def health_check(self) -> bool:
        """Check if the server is healthy (no auth required)."""
        try:
            response = requests.get(f"{self.base_url}/health", timeout=5)
            return response.status_code == 200 and response.text == "OK"
        except Exception as e:
            print(f"Health check failed: {e}")
            return False
    
    def get_balance(self) -> int:
        """Get current balance in millisatoshi."""
        response = requests.get(f"{self.base_url}/balance", headers=self.headers)
        response.raise_for_status()
        return response.json()["balance_msats"]
    
    def create_invoice(self, amount_msats: int, description: str) -> dict:
        """Create a Lightning invoice."""
        data = {
            "amount_msats": amount_msats,
            "description": description
        }
        response = requests.post(
            f"{self.base_url}/invoice",
            headers=self.headers,
            json=data
        )
        response.raise_for_status()
        return response.json()
    
    def check_invoice_status(self, payment_hash: str, timeout: int = 300) -> bool:
        """
        Check if an invoice has been paid.
        
        Note: This endpoint blocks until payment is received or times out.
        """
        try:
            response = requests.get(
                f"{self.base_url}/invoice/{payment_hash}",
                headers=self.headers,
                timeout=timeout
            )
            response.raise_for_status()
            return response.json()["paid"]
        except requests.exceptions.Timeout:
            print(f"Invoice check timed out after {timeout} seconds")
            return False
    
    def pay_invoice(self, invoice: str) -> str:
        """Pay a Lightning invoice and return the preimage."""
        data = {"invoice": invoice}
        response = requests.post(
            f"{self.base_url}/pay",
            headers=self.headers,
            json=data
        )
        response.raise_for_status()
        return response.json()["preimage"]


def main():
    parser = argparse.ArgumentParser(description="Blitzid API client example")
    parser.add_argument(
        "--url",
        default="http://127.0.0.1:3000",
        help="Blitzid server URL"
    )
    parser.add_argument(
        "--token",
        required=True,
        help="Bearer token for authentication"
    )
    
    args = parser.parse_args()
    
    client = BlitzidClient(args.url, args.token)
    
    # Test health check
    print("Checking server health...")
    if not client.health_check():
        print("Server is not healthy!")
        sys.exit(1)
    print("✓ Server is healthy")
    
    # Get balance
    print("\nGetting balance...")
    balance = client.get_balance()
    print(f"✓ Current balance: {balance} msats ({balance / 1000:.3f} sats)")
    
    # Create an invoice
    print("\nCreating invoice for 1000 msats...")
    invoice_data = client.create_invoice(1000, "Test payment from Python client")
    print(f"✓ Invoice created: {invoice_data['invoice']}")
    print(f"  Payment hash: {invoice_data['payment_hash']}")
    
    # Note: To test payment checking, you would need to pay the invoice
    # from another Lightning wallet
    print("\nTo test the invoice status check, pay this invoice from another wallet")
    print("The check_invoice_status() method will wait for payment...")
    
    # Example: Uncomment to wait for payment
    # print("Waiting for payment...")
    # paid = client.check_invoice_status(invoice_data['payment_hash'])
    # if paid:
    #     print("✓ Invoice was paid!")
    # else:
    #     print("✗ Invoice was not paid (timed out)")


if __name__ == "__main__":
    main()
