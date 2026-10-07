# QUSD reserve statement — TESTNET DEMO

**This is a demonstration document for Stellar testnet. QUSD is not a real
asset and nothing here describes real money.** It exists to show what
Plimsoll does when an asset is under-backed.

| Field | Value |
| --- | --- |
| Asset | QUSD |
| Network | Stellar testnet |
| As of | 2026-10-08 00:00 UTC |
| Reserves held | 480,000.00 USD-equivalent |
| Circulating supply at as-of | 500,000.00 QUSD |
| Coverage | 96.00% |

## Reserve composition (illustrative)

| Holding | Amount |
| --- | --- |
| Cash at custodian bank | 80,000.00 |
| US Treasury bills, < 90 days | 400,000.00 |
| **Total** | **480,000.00** |

## How this document is used

In this demo the QUSD issuer does not post its own figure. A registered
transcriber copied the reserve total from this published statement and posted
it at the **Transcribed** tier, recording this file's SHA-256 as `doc_hash`.
Coverage is 96%, so `is_covered(QUSD, 10000, …)` returns false and the QUSD
example vault refuses deposits.
