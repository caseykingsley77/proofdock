/** Integration tests for classic SPL token escrow on a local validator. */
import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { expect } from "chai";
import { createHash, randomBytes } from "node:crypto";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import type { Proofdock } from "../target/types/proofdock";

const MINT_DECIMALS = 6;
const TOKEN_AMOUNT = 1_000_000n;
const META_HASH = Array.from(createHash("sha256").update("SPL agreement v1").digest());

function pda(seeds: Buffer[], programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(seeds, programId)[0];
}

async function airdrop(provider: anchor.AnchorProvider, pubkey: PublicKey): Promise<void> {
  const signature = await provider.connection.requestAirdrop(pubkey, 2 * LAMPORTS_PER_SOL);
  const blockhash = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({ signature, ...blockhash }, "confirmed");
}

describe("proofdock SPL token escrow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.proofdock as Program<Proofdock>;

  async function createTokenEscrow(deadlineSeconds = 3600) {
    const client = Keypair.generate();
    const contractor = Keypair.generate();
    await airdrop(provider, client.publicKey);
    const mint = await createMint(provider.connection, client, client.publicKey, null, MINT_DECIMALS);
    const clientToken = await createAccount(provider.connection, client, mint, client.publicKey);
    const contractorToken = await createAccount(provider.connection, client, mint, contractor.publicKey);
    await mintTo(provider.connection, client, mint, clientToken, client, TOKEN_AMOUNT * 2n);

    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + deadlineSeconds);
    const amount = new anchor.BN(TOKEN_AMOUNT.toString());
    const tokenEscrow = pda([
      Buffer.from("token-escrow"), client.publicKey.toBuffer(), id.toArrayLike(Buffer, "le", 8),
    ], program.programId);
    const vault = pda([Buffer.from("token-vault"), tokenEscrow.toBuffer()], program.programId);
    await program.methods.createTokenEscrow(id, amount, deadline, META_HASH)
      .accounts({
        client: client.publicKey,
        contractor: contractor.publicKey,
        mint,
        clientToken,
        tokenEscrow,
        vault,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([client])
      .rpc();

    return { client, contractor, mint, clientToken, contractorToken, tokenEscrow, vault, id, amount };
  }

  it("creates a token escrow with a funded program-controlled vault", async () => {
    const s = await createTokenEscrow();
    const state = await program.account.tokenEscrow.fetch(s.tokenEscrow);
    const vault = await getAccount(provider.connection, s.vault);
    expect(state.client.equals(s.client.publicKey)).to.equal(true);
    expect(state.contractor.equals(s.contractor.publicKey)).to.equal(true);
    expect(state.mint.equals(s.mint)).to.equal(true);
    expect(state.amount.toString()).to.equal(TOKEN_AMOUNT.toString());
    expect(vault.owner.equals(s.tokenEscrow)).to.equal(true);
    expect(vault.amount).to.equal(TOKEN_AMOUNT);
  });

  it("releases the tokens to the contractor and closes the escrow and vault", async () => {
    const s = await createTokenEscrow();
    await program.methods.releaseTokenEscrow().accounts({
      client: s.client.publicKey,
      contractor: s.contractor.publicKey,
      mint: s.mint,
      tokenEscrow: s.tokenEscrow,
      vault: s.vault,
      contractorToken: s.contractorToken,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([s.client]).rpc();
    const destination = await getAccount(provider.connection, s.contractorToken);
    expect(destination.amount).to.equal(TOKEN_AMOUNT);
    expect(await provider.connection.getAccountInfo(s.vault)).to.equal(null);
    expect(await provider.connection.getAccountInfo(s.tokenEscrow)).to.equal(null);
  });

  it("rejects release by a different wallet", async () => {
    const s = await createTokenEscrow();
    const attacker = Keypair.generate();
    await airdrop(provider, attacker.publicKey);
    let rejected = false;
    try {
      await program.methods.releaseTokenEscrow().accounts({
        client: attacker.publicKey,
        contractor: s.contractor.publicKey,
        mint: s.mint,
        tokenEscrow: s.tokenEscrow,
        vault: s.vault,
        contractorToken: s.contractorToken,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([attacker]).rpc();
    } catch (_) { rejected = true; }
    expect(rejected, "unauthorized release must fail").to.equal(true);
    expect((await getAccount(provider.connection, s.vault)).amount).to.equal(TOKEN_AMOUNT);
  });

  it("rejects releasing to a token account for a different mint", async () => {
    const s = await createTokenEscrow();
    const otherMint = await createMint(provider.connection, s.client, s.client.publicKey, null, 6);
    const wrongAccount = await createAccount(provider.connection, s.client, otherMint, s.contractor.publicKey);
    let rejected = false;
    try {
      await program.methods.releaseTokenEscrow().accounts({
        client: s.client.publicKey,
        contractor: s.contractor.publicKey,
        mint: s.mint,
        tokenEscrow: s.tokenEscrow,
        vault: s.vault,
        contractorToken: wrongAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([s.client]).rpc();
    } catch (_) { rejected = true; }
    expect(rejected, "wrong token mint must fail").to.equal(true);
    expect((await getAccount(provider.connection, s.vault)).amount).to.equal(TOKEN_AMOUNT);
  });

  it("rejects refunding before the deadline", async () => {
    const s = await createTokenEscrow();
    let rejected = false;
    try {
      await program.methods.refundExpiredTokenEscrow().accounts({
        client: s.client.publicKey,
        mint: s.mint,
        tokenEscrow: s.tokenEscrow,
        vault: s.vault,
        clientToken: s.clientToken,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).signers([s.client]).rpc();
    } catch (_) { rejected = true; }
    expect(rejected, "early refund must fail").to.equal(true);
  });

  it("refunds after the deadline and closes escrow accounts", async () => {
    const s = await createTokenEscrow(3);
    await new Promise((resolve) => setTimeout(resolve, 5000));
    await program.methods.refundExpiredTokenEscrow().accounts({
      client: s.client.publicKey,
      mint: s.mint,
      tokenEscrow: s.tokenEscrow,
      vault: s.vault,
      clientToken: s.clientToken,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([s.client]).rpc();
    expect((await getAccount(provider.connection, s.clientToken)).amount).to.equal(TOKEN_AMOUNT * 2n);
    expect(await provider.connection.getAccountInfo(s.vault)).to.equal(null);
    expect(await provider.connection.getAccountInfo(s.tokenEscrow)).to.equal(null);
  });
});
