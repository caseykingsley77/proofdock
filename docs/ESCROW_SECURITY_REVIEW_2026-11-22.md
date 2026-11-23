# Escrow security review — original 2026-11-22 observations and 2026-11-23 proposal

**The original review predated these changes.** This file records the initial risks and their proposed mitigations on the new PR branch. Tests and CI must pass before considering these verified, and this is **not an independent audit**.

## Changes proposed in this branch

1. **Client-controlled settlement:** On new escrows, the contractor must sign creation, agreeing to an independent arbitrator. Either party can flag an escrow as disputed, blocking ordinary settlement. The agreed arbitrator can resolve a disputed SOL or SPL escrow in either party's favor, without client permission. Ordinary expiry refunds require both original party signatures. This addresses unilateral client withholding/refund at the cost of relying on arbitrator availability and trust.
2. **Token surplus payouts:** Contractor payouts use the recorded `token_escrow.amount` rather than `vault.amount`. The balance must cover the agreed amount. Extra deposited tokens are returned to the original client, and the vault closes. Refunds send the full vault balance to the client. The original donor of extra token transfers is not tracked.
3. **Regression tests:** Add successful and unauthorized SOL dispute tests, token dispute resolutions, exact payout and surplus cases, and jointly signed refunds. All still require CI validation against a real local validator.

## Remaining risks / gaps

- **High — Arbitration liveness and trust:** Arbitrator loss/unavailability, coercion, collusion and ambiguous evidence can still lock or misdirect funds. There is no arbitrator rotation, backup or appeal.
- **High — Breaking upgrade:** Escrow state size, creation instruction payload and expiry refund account lists changed. Existing escrow accounts and clients are not compatible; no migration or upgrade path is implemented.
- **Medium — Unattributed deposits:** Extra SPL vault tokens are returned to client even if sent by a third party. The contract cannot attribute donors.
- **Medium — Token compatibility:** Classic SPL only; Token-2022, transfer hooks, fees, freeze authorities and malicious/unsupported mints need a separate threat model.
- **Medium — No production governance:** Program upgrade authority management, deployment policy, signing keys and independent audit not completed.
- **Low — Historical key reuse:** Escrow accounts close, permitting reuse of (client, id) PDA. Indexers need tx signature/slot.
- **Low — Wall-clock integration tests:** Expiration tests sleep on validator wall time and can be flaky in busy CI.

No real funds should be used with this prototype. A green CI suite tests specified scenarios only and is not an audit.
