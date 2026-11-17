# Architecture and invariants

## Responsibilities

1. **Wallet/client**: selects contractor, milestone ID, terms hash, funding amount, and deadline; signs instructions.
2. **Anchor program**: validates signer ownership and PDA seeds; holds lamports; releases or refunds them atomically.
3. **Python API**: reads accounts from a configured RPC endpoint, validates owner/discriminator/layout, and returns JSON. It never signs transactions or controls funds.
4. **Solana RPC**: authoritative current account data; not necessarily complete event history.

## PDA and account layout

```
PDA seeds: [b"escrow", client_pubkey, milestone_id_u64_le]

Escrow account: 129 bytes total
offset  size  type                 field
  0      8   Anchor discriminator SHA256("account:Escrow")[:8]
  8      8   u64 little-endian     id
 16     32   Pubkey                client
 48     32   Pubkey                contractor
 80      8   u64 little-endian     amount
 88      8   i64 little-endian     deadline
 96     32   bytes                 metadata_hash
128      1   u8                    bump
```

The Rust `InitSpace` calculation and Python `struct.Struct('<Q32s32sQq32sB')` must remain aligned. The Python API queries memcmp at offset **16** (client pubkey) plus dataSize **129**.

## Escrow invariants

- Positive deposited amount; client and contractor are distinct system-owned wallets.
- Future deadline, no more than 365 days after creation.
- Nonzero SHA-256 terms hash (the off-chain document is **not** available or authenticated by the program; parties must separately agree on what was hashed).
- No two active escrows can share one `(client, milestone_id)` PDA.
- The original client is the only authority allowed to release or refund, verified with `has_one` and `Signer` constraints.
- Releasing sends precisely the recorded `amount` to the recorded contractor; remaining rent is returned to the client at close.
- Refund requires `Clock.unix_timestamp >= deadline`; the account's SOL deposit and rent are returned to the client on close.
- A single successful settlement closes the account. Subsequent release/refund attempts fail because the account no longer exists.

## Threat model and limitations

| Concern | Control | Remaining risk |
| --- | --- | --- |
| Impersonated client | `Signer` + `has_one = client` | Client wallet compromise |
| Substitute contractor | `address = escrow.contractor` | User can choose an unintended address at creation |
| Forged PDA | Program seeds + stored bump | Program-upgrade authority risk not addressed |
| Overflow/underflow | `checked_add` / `checked_sub` | On-chain tests not yet executed |
| Front-running / replay | Unique `(client,id)` address, init fails for occupied PDA | IDs can be reused after close; no historical registry |
| False API accounts | RPC `owner` + account discriminator + exact length | Trust in RPC provider for freshness/completeness |
| Release disputes | Client is sole release authority | Contractor has no appeal or arbitration mechanism |
| Confidential data | Store only hash | Terms can still leak off-chain; hash is not encryption |

No program upgrade governance, key management, on-chain arbitration, token-based settlement, or independent audit is included in this prototype. There is no mainnet deployment claim.

## Future enhancements

- Separate escrow-vault authority PDA and immutable agreement registry for non-reusable IDs.
- Multi-milestone projects and partial release balances.
- Independent arbitrator or mutually signed refund cancellation.
- SPL Token/Token-2022 payments through `anchor-spl` token interfaces.
- WebSocket or log ingestion to track closed accounts and historic transactions.
- Devnet demo with transaction signatures, explorer links and screenshots.
