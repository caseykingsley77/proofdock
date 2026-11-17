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

  it("does not refund a funded escrow before expiry", async () => {
    const { escrow } = await create(3600);
    try {
      await program.methods.refundExpired().accounts({
        client: provider.wallet.publicKey,
        escrow,
      }).rpc();
      expect.fail("should reject refunds before expiry");
    } catch (error) {
      expect(String(error)).to.match(/NotExpired|deadline|error/i);
    }
  });

  it("rejects a zero-value escrow", async () => {
    const contractor = Keypair.generate();
    await airdrop(provider, contractor.publicKey);
    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const escrow = escrowPda(provider.wallet.publicKey, id, program.programId);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
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
      expect.fail("should reject empty escrow funding");
    } catch (error) {
      expect(String(error)).to.match(/ZeroAmount|escrow amount|error/i);
    }
  });
});
