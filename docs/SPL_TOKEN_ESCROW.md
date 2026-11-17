# SPL-token escrow (prototype; unaudited)

This extension supports the **classic SPL Token program** only. Token-2022
features, transfer fees, frozen accounts, and arbitrary custom token programs
are outside the current design scope.

## Flow

1. The client creates `TokenEscrow`, a PDA seeded by `token-escrow`, client
   wallet, and little-endian milestone ID; and a vault token account seeded
   by `token-vault` and the escrow PDA. Both are initialized atomically.
2. The program transfers the exact base-unit amount from the client's token
   account to the PDA-controlled vault using `transfer_checked`.
3. Only the recorded client may release the escrow. The vault's entire balance
   is transferred to a token account owned by the recorded contractor.
4. After the deadline, only the client can refund the full vault balance to a
   client-owned token account.
5. The vault and escrow accounts are closed, and their rent returns to the
   client.

**Extra-token policy:** A third party can transfer additional tokens into
any existing SPL token account. To avoid a dusting denial of service, release
and refund deliberately transfer the vault's **entire** token balance, not
just the originally committed amount. The event includes the amount actually
paid. This behavior must be clearly disclosed to users.

## Required tests before merge or deployment

- Create and fund vault with a test mint and two wallets
- Client releases; contractor receives tokens; both accounts close
- Unauthorized signer cannot release or refund
- Client cannot refund before deadline
- Expired escrow refunds to the client
- Wrong mint, wrong vault authority, or wrong destination owner fail
- Zero amount and invalid metadata fail
- Extraneous tokens deposited in vault do not block closure

**Status:** This change is not compiled/tested in the provided environment.
Run `anchor build` and `anchor test --validator legacy` on a configured Mac.
Do not use with real funds until full integration tests and a security audit.
