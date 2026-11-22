/** Anchor local-validator integration tests. Requires `anchor build` first. */
import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { expect } from "chai";
import { createHash, randomBytes } from "node:crypto";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import type { Proofdock } from "../target/types/proofdock";

const ESCROW_SEED = Buffer.from("escrow");

function escrowPda(client: PublicKey, id: anchor.BN, programId: PublicKey): PublicKey {
  const idBytes = id.toArrayLike(Buffer, "le", 8);
  return PublicKey.findProgramAddressSync(
    [ESCROW_SEED, client.toBuffer(), idBytes],
    programId,
  )[0];
}

async function airdrop(
  provider: anchor.AnchorProvider,
  pubkey: PublicKey,
  lamports = LAMPORTS_PER_SOL,
): Promise<void> {
  const signature = await provider.connection.requestAirdrop(pubkey, lamports);
  const latest = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({ signature, ...latest }, "confirmed");
}

describe("proofdock SOL milestone escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.proofdock as Program<Proofdock>;

  async function create(deadlineOffsetSeconds = 3600) {
    const contractor = Keypair.generate();
    await airdrop(provider, contractor.publicKey);
    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const amount = new anchor.BN(100_000);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + deadlineOffsetSeconds);
    const metadataHash = Array.from(createHash("sha256").update("deliverable-v1").digest());
    const escrow = escrowPda(provider.wallet.publicKey, id, program.programId);

    await program.methods.createEscrow(id, amount, deadline, metadataHash)
      .accounts({
        client: provider.wallet.publicKey,
        contractor: contractor.publicKey,
        escrow,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    return { escrow, contractor, id, amount, deadline };
  }

  it("creates an escrow with the correct PDA and state", async () => {
    const { escrow, contractor, amount, deadline, id } = await create();
    const data = await program.account.escrow.fetch(escrow);
    expect(data.client.equals(provider.wallet.publicKey)).to.equal(true);
    expect(data.contractor.equals(contractor.publicKey)).to.equal(true);
    expect(data.id.toString()).to.equal(id.toString());
    expect(data.amount.toString()).to.equal(amount.toString());
    expect(data.deadline.toString()).to.equal(deadline.toString());
  });

  it("releases exactly the escrowed amount and closes the account", async () => {
    const { escrow, contractor, amount } = await create();
    const before = await provider.connection.getBalance(contractor.publicKey);
    await program.methods.release().accounts({
      client: provider.wallet.publicKey,
      contractor: contractor.publicKey,
      escrow,
    }).rpc();
    const after = await provider.connection.getBalance(contractor.publicKey);
    expect(after - before).to.equal(amount.toNumber());
    expect(await provider.connection.getAccountInfo(escrow)).to.equal(null);
  });

  it("blocks a different wallet from releasing someone else's escrow", async () => {
    const { escrow, contractor } = await create();
    const attacker = Keypair.generate();
    await airdrop(provider, attacker.publicKey);
    let rejected = false;
    try {
      await program.methods.release().accounts({
        client: attacker.publicKey,
        contractor: contractor.publicKey,
        escrow,
      }).signers([attacker]).rpc();
    } catch (_) {
      rejected = true; // has_one/client PDA constraints must reject
    }
    expect(rejected, "unauthorized release must fail").to.equal(true);
    expect(await provider.connection.getAccountInfo(escrow)).to.not.equal(null);
  });

  it("does not refund a funded escrow before expiry", async () => {
    const { escrow } = await create(3600);
    let rejected = false;
    try {
      await program.methods.refundExpired().accounts({
        client: provider.wallet.publicKey,
        escrow,
      }).rpc();
    } catch (error) {
      rejected = true;
      expect(String(error)).to.match(/NotExpired|not reached its deadline/i);
    }
    expect(rejected, "refund must fail before the deadline").to.equal(true);
  });

  it("refunds the client and closes an expired escrow", async () => {
    const { escrow } = await create(3);
    // Local-validator time is approximately wall time; allow it to advance.
    await new Promise((resolve) => setTimeout(resolve, 5000));
    await program.methods.refundExpired().accounts({
      client: provider.wallet.publicKey,
      escrow,
    }).rpc();
    expect(await provider.connection.getAccountInfo(escrow)).to.equal(null);
  });

  it("rejects redirecting a payout to a different contractor", async () => {
    const { escrow } = await create();
    const impostor = Keypair.generate();
    await airdrop(provider, impostor.publicKey);
    const before = await provider.connection.getBalance(impostor.publicKey);
    let rejected = false;
    try {
      await program.methods.release().accounts({
        client: provider.wallet.publicKey,
        contractor: impostor.publicKey,
        escrow,
      }).rpc();
    } catch (_) {
      rejected = true;
    }
    expect(rejected, "only the stored contractor may receive payout").to.equal(true);
    expect(await provider.connection.getBalance(impostor.publicKey)).to.equal(before);
    expect(await provider.connection.getAccountInfo(escrow)).to.not.equal(null);
  });

  it("blocks an unauthorized refund even after expiry", async () => {
    const { escrow } = await create(3);
    const attacker = Keypair.generate();
    await airdrop(provider, attacker.publicKey);
    await new Promise((resolve) => setTimeout(resolve, 5000));
    let rejected = false;
    try {
      await program.methods.refundExpired().accounts({
        client: attacker.publicKey,
        escrow,
      }).signers([attacker]).rpc();
    } catch (_) {
      rejected = true;
    }
    expect(rejected, "only the escrow client may request a refund").to.equal(true);
    expect(await provider.connection.getAccountInfo(escrow)).to.not.equal(null);
  });

  it("rejects a duplicate active milestone without changing its funds", async () => {
    const { escrow, contractor, id, amount, deadline } = await create();
    const balanceBefore = await provider.connection.getBalance(escrow);
    let rejected = false;
    try {
      await program.methods.createEscrow(
        id, amount, deadline,
        Array.from(createHash("sha256").update("duplicate-terms").digest()),
      ).accounts({
        client: provider.wallet.publicKey,
        contractor: contractor.publicKey,
        escrow,
        systemProgram: SystemProgram.programId,
      }).rpc();
    } catch (_) {
      rejected = true;
    }
    expect(rejected, "one active (client, milestone) PDA only").to.equal(true);
    expect(await provider.connection.getBalance(escrow)).to.equal(balanceBefore);
    const data = await program.account.escrow.fetch(escrow);
    expect(data.amount.toString()).to.equal(amount.toString());
  });

  it("cannot release an escrow twice after closing it", async () => {
    const { escrow, contractor } = await create();
    await program.methods.release().accounts({
      client: provider.wallet.publicKey,
      contractor: contractor.publicKey,
      escrow,
    }).rpc();
    expect(await provider.connection.getAccountInfo(escrow)).to.equal(null);
    let rejected = false;
    try {
      await program.methods.release().accounts({
        client: provider.wallet.publicKey,
        contractor: contractor.publicKey,
        escrow,
      }).rpc();
    } catch (_) {
      rejected = true;
    }
    expect(rejected, "closed escrows cannot be paid again").to.equal(true);
  });

  it("rejects a blank terms commitment", async () => {
    const contractor = Keypair.generate();
    await airdrop(provider, contractor.publicKey);
    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const escrow = escrowPda(provider.wallet.publicKey, id, program.programId);
    let rejected = false;
    try {
      await program.methods.createEscrow(
        id, new anchor.BN(100_000),
        new anchor.BN(Math.floor(Date.now() / 1000) + 3600),
        Array(32).fill(0),
      ).accounts({
        client: provider.wallet.publicKey,
        contractor: contractor.publicKey,
        escrow,
        systemProgram: SystemProgram.programId,
      }).rpc();
    } catch (_) {
      rejected = true;
    }
    expect(rejected, "zero metadata hash is invalid").to.equal(true);
    expect(await provider.connection.getAccountInfo(escrow)).to.equal(null);
  });

  it("rejects a zero-value escrow", async () => {
    const contractor = Keypair.generate();
    await airdrop(provider, contractor.publicKey);
    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const escrow = escrowPda(provider.wallet.publicKey, id, program.programId);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
    let rejected = false;
    try {
      await program.methods.createEscrow(
        id, new anchor.BN(0), deadline,
        Array.from(createHash("sha256").update("terms").digest()),
      ).accounts({
        client: provider.wallet.publicKey,
        contractor: contractor.publicKey,
        escrow,
        systemProgram: SystemProgram.programId,
      }).rpc();
    } catch (error) {
      rejected = true;
      expect(String(error)).to.match(/ZeroAmount|Escrow amount must be greater than zero/i);
    }
    expect(rejected, "zero-value creation must fail").to.equal(true);
  });
});
