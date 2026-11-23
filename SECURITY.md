# Security policy — prototype

ProofDock is a development/portfolio escrow prototype. It has **not** received an independent smart-contract audit. **Do not send real funds to the program.**

## Security model

- Both client and contractor sign escrow creation and accept a distinct, preselected arbitrator public key.
- Either party may dispute a live escrow. A dispute freezes ordinary release/refund; only the approved arbitrator can decide a disputed escrow.
- Normal release is authorized by the client. Expired refunds require client and contractor signatures. An arbitrator can pay either side of a disputed escrow, before or after expiry.
- Arbitrator unavailability can permanently block disputed funds, and collusion/key theft can cause loss. This is a trust-based arbitration design, not a permissionless dispute protocol.
- Contractor token payout is limited to the agreed amount. Unexpected extra SPL deposits go to the client at vault closure. The program cannot determine the original donor of extra transfers.
- Changes to the escrow layout are breaking; no account or client migration is included.
- The FastAPI service is read-only; never commit wallets, signing keys, deployment keypairs, secrets, or `.env` files.

## Before production

Run a specialist Solana/Anchor audit; review token mint and Token-2022 policy, timelocks, dispute governance, arbitrator rotation/recovery and evidence integrity; adopt program upgrade authority multisig and monitoring; and deploy/test on devnet with wallet-connected clients first.
