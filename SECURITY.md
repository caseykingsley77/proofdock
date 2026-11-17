# Security policy (prototype)

ProofDock is a learning and portfolio project, not a financial product. It has not been audited or deployed on mainnet. Do not send real funds to this program.

Security-sensitive assumptions:

- A client retains sole control of milestone release. A contractor cannot force payout.
- A client can reclaim funds after deadline even if deliverables were submitted but not acknowledged.
- Upgrading the on-chain program could change trust assumptions. No governance is implemented.
- The API is read-only; all payment transactions must be signed by a wallet on the client side.
- Never commit wallets, mnemonics, RPC API secrets, `.env` files, or program keypairs.

For real deployments: require an independent Solana smart contract audit, threat modeling, end-to-end tests, upgrade-authority governance, and a documented dispute process.
