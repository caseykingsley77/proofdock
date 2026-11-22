# Solana escrow security review — 2026-11-22

Scope: source-level review of `programs/proofdock/src/lib.rs`, the `tests/proofdock.ts` local-validator suite, and CI configuration. **Not an independent audit.** Observations below are code review findings, not proof that integration tests have passed.

## Implemented protections

- SOL `release` requires the original client signer (`has_one = client` and escrow PDA seeds) and binds the payout wallet to `escrow.contractor`.
- SOL `refund_expired` requires the original client and a clock timestamp at or after the recorded deadline.
- `init` of a PDA keyed by `["escrow", client, id]` prevents opening a second *active* SOL escrow for that key. Once the PDA is closed, the ID can be reused. Do not treat ID uniqueness as permanent history.
- SOL release uses checked arithmetic and closes the account; refund closes the account after expiry.
- SPL token settlement uses an escrow PDA signer, enforces mint/authority relationships, and closes the vault after settlement.

## Outstanding findings and test gaps

1. **High — Client-controlled settlement; no contractor dispute path.** Client can choose not to release a completed milestone and can refund after the deadline. This is a product-level trust assumption, not an access-control bug. Define an arbitration or mutual-consent model before production use.
2. **Medium — SPL token settlement transfers `vault.amount`, not `token_escrow.amount`.** Additional tokens transferred into the vault are sent to the contractor on release, or back to the client on refund, even when more than the agreed amount was funded. The code documents this as dust-recovery behavior needed for vault closure. Make this explicit in the protocol and test unsolicited deposits before relying on the invariant “exact payout amount.” Consider a separate surplus-handling design if exact amount is required.
3. **Medium — SPL token lifecycle has no committed validator tests.** Add token mint setup, vault funding, authorized release, expired refund, wrong mint, wrong authority, wrong destination, and unsolicited-extra-token cases. The current workflow tests the SOL milestones only.
4. **Medium — Program upgrade authority and deployment keys are not governed.** Development program ID is a placeholder. Local CI creates ephemeral keys and does not establish safe deployment or immutability. Document authority management and run a professional audit before holding funds.
5. **Low — Historical escrow IDs can be reused after closure.** PDA-derived uniqueness exists only while the old account is active. An indexer must disambiguate historical records by transaction/signature/slot, not just `(client, id)`.
6. **Low — Deadline test uses wall-clock wait.** The existing three-second expiry test sleeps for five seconds. This can be flaky if validator time differs or the runner is overloaded; use validator clock manipulation or a generous duration when expanding coverage.

## Regression coverage added in the test branch

- Reject release by another signer.
- Reject substituted contractor payout address.
- Reject refund by another signer, including after expiry.
- Reject duplicate active milestone and preserve funds.
- Reject a repeated release after account closure.
- Reject blank metadata commitment and zero escrow value.

The dedicated GitHub Actions workflow installs Anchor and Solana CLI, generates local disposable keypairs, and invokes `anchor test --validator legacy` against `solana-test-validator`. **Passing Rust unit tests alone does not establish a passing on-chain validator run.** Keep any PR unmerged until the validator job passes, or explicitly document unresolved blockers.

No production funds should be used with this prototype.
