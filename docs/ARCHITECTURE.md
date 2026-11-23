# Architecture and accounting invariants

> Development-only contract, not audited or deployed with production funds.
> This revision changes the SOL escrow account layout and instruction signatures; it is incompatible with old on-chain accounts and SDKs. No migration strategy has been implemented.

## Roles and consent

- **Client**: funds escrow, may voluntarily release funds, can file a dispute.
- **Contractor**: signs creation to accept the arbitrator, can file a dispute, co-signs ordinary expiry refunds.
- **Arbitrator**: independent, nonzero public key selected at creation by the client's and contractor's signed transaction. Cannot be a party. Can decide where escrow funds go **only after a dispute**. The arbitrator signs the resolution instruction, choosing contractor payout or refund to client.
- **Anchor program**: escrow authority and accounting.
- **Python API**: read-only inspection of current funded/disputed SOL escrows; holds no keys.

A non-disputed SOL or classic SPL-token escrow is released by the original client. An ordinary expiry refund requires signatures of **both original parties** and is blocked when a dispute is active. Either party can open a dispute at any time while the escrow exists, including after expiry. Once disputed, ordinary release/refund is frozen. Only the accepted arbitrator may resolve it, and settlement closes the account. Arbitration is all-or-nothing for each escrow, not a partial-payment system.

**Important liveness limitation:** A missing/uncooperative arbitrator can leave funds locked indefinitely after a dispute, and absent contractor consent can block ordinary expiry refunds. A key compromise or colluding arbitrator can choose an unfair verdict. Production work needs arbitration governance, replacement/recovery mechanisms, dispute evidence rules, timelocks and professional audit.

## SOL escrow PDA and account layout

PDA: `[b"escrow", client_pubkey, milestone_id_u64_le]`. Account format uses Anchor's eight-byte discriminator plus:

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | `Escrow` discriminator |
| 8 | 8 | Milestone ID (u64 LE) |
| 16 | 32 | Client public key |
| 48 | 32 | Contractor public key |
| 80 | 32 | Arbitrator public key |
| 112 | 8 | Agreed lamports (u64 LE) |
| 120 | 8 | Deadline (i64 LE) |
| 128 | 32 | SHA-256 terms commitment |
| 160 | 1 | Disputed flag (bool) |
| 161 | 1 | PDA bump (u8) |

**162 bytes total.** The Python `ESCROW_LAYOUT` must match this layout. The client memcmp offset remains 16.

## Token payout invariants

- Classic SPL Token Program only; Token-2022 extensions are not supported or audited.
- Vault PDA is controlled by the token escrow PDA.
- `token_escrow.amount` is the **maximum contractor payout** in a full contractor-favoring settlement, regardless of unsolicited extra deposits. `vault.amount` must be at least the agreed amount or settlement fails.
- On contractor release or contractor-favoring arbitration, send **exactly the agreed amount** to the bound contractor token account. Any remaining vault balance goes to the client's validated token account.
- On ordinary refund or client-favoring arbitration, send the **entire vault balance** to the client's validated token account.
- After a successful settlement, close the vault and token escrow, returning rent to client.
- Extra deposit provenance is not tracked; unsolicited third-party token deposits end up with the client on settlement. This is an explicit limitation, not fair donor restitution.
- All operations are atomic Solana transactions; a token transfer/closure failure reverts settlement.

## Other limitations

- Account closure makes previously used `(client,id)` escrow addresses reusable; historical identity needs transaction signature/slot, not just PDA.
- Metadata hashes do not prove deliverable completion or encrypt off-chain agreements.
- Read-only API cannot prove full historical events or guarantee up-to-date RPC responses.
- Upgrade authority, identity verification, arbitration policy, dispute evidence and custody remain unaudited.
