# Roadmap — actual, testable milestones

The following are future tasks, **not completed work**. Commit each change when it is built and reviewed. Do not fabricate commit dates or artificially split unchanged work.

## Phase 1 — local correctness

- [x] Implement native SOL escrow instructions and account constraints.
- [x] Add unit tests for time/amount rules (source authored; run pending).
- [x] Build read-only Python account codec and RPC transport.
- [x] Add Python unit/HTTP tests with mocked RPC (29 passing).
- [x] Add basic Anchor local-validator integration test source (run pending).
- [ ] Run `cargo test` and `anchor build`; resolve any compiler/IDL errors.
- [ ] Run `anchor test` under local validator; confirm create/release paths.
- [x] Add unauthorized release and expired refund integration test source (execution pending).
- [ ] Add test for duplicate milestone IDs and fund conservation.
- [ ] Cover RPC timeouts/rate limits and paginated/large account results.

## Phase 2 — app-quality observability

- [ ] Add event consumer/SQL schema for milestone history.
- [ ] Add idempotent retry/backfill logic for confirmed/finalized slots.
- [ ] Record transaction signatures and failed transaction reasons.
- [ ] Add Postgres persistence and Alembic migrations.
- [ ] Add request IDs, structured logs, metrics, and rate limiting.
- [ ] Add Docker Compose for API/Postgres/worker.

## Phase 3 — Solana/payment features

- [ ] Add SPL token support with Token-2022 compatibility tests.
- [ ] Add a two-party cancellation path and optional arbitration.
- [ ] Add milestones with partial payouts and immutable registry accounts.
- [ ] Add property-based/fuzz tests for accounting invariants.
- [ ] Audit the contract before considering any real-value deployment.

## Phase 4 — demonstrable application

- [ ] Add wallet-connected React dashboard.
- [ ] Run and document devnet deployment with example transaction hashes.
- [ ] Publish a short design walkthrough and honest threat-model review.
- [ ] Add accessibility, CI and dependency-update policies.

## If applying for a ROS 2 position

A Solana escrow does **not** establish ROS experience. A separate Python `rclpy` telemetry/event publisher or robot-fleet payment adapter could be created as a clearly labeled follow-on project, and should be included in a ROS-specific CV only after it runs in a ROS 2 environment.
