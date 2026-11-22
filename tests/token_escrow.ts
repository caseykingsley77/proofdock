/**
 * Classic SPL Token escrow tests against a real local validator.
 * We use the stable classic Token Program instruction layout directly so this
 * suite can run with the repository's existing lockfile (no new npm packages).
 */
import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { expect } from "chai";
import { createHash, randomBytes } from "node:crypto";
import {
  Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY,
  Transaction, TransactionInstruction,
} from "@solana/web3.js";
import type { Proofdock } from "../target/types/proofdock";

const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const MINT_SPACE = 82;
const TOKEN_ACCOUNT_SPACE = 165;
const INITIAL_TOKENS = 500_000;
const ESCROW_TOKENS = 100_000;

function amountLE(n: number): Buffer {
  return new anchor.BN(n).toArrayLike(Buffer, "le", 8);
}

function tokenInstruction(data: Buffer, keys: Array<{pubkey: PublicKey; isSigner: boolean; isWritable: boolean}>): TransactionInstruction {
  return new TransactionInstruction({ programId: TOKEN_PROGRAM, keys, data });
}

function initMint(mint: PublicKey, authority: PublicKey): TransactionInstruction {
  // Classic TokenInstruction.InitializeMint = 0, decimals = 0,
  // freeze-authority COption = None (one zero byte).
  return tokenInstruction(Buffer.concat([
    Buffer.from([0, 0]), authority.toBuffer(), Buffer.from([0]),
  ]), [
    { pubkey: mint, isSigner: false, isWritable: true },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ]);
}

function initAccount3(account: PublicKey, mint: PublicKey, owner: PublicKey): TransactionInstruction {
  // Classic TokenInstruction.InitializeAccount3 = 18.
  return tokenInstruction(Buffer.concat([Buffer.from([18]), owner.toBuffer()]), [
    { pubkey: account, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: false },
  ]);
}

function mintTo(mint: PublicKey, account: PublicKey, authority: PublicKey, tokens: number): TransactionInstruction {
  // Classic TokenInstruction.MintTo = 7.
  return tokenInstruction(Buffer.concat([Buffer.from([7]), amountLE(tokens)]), [
    { pubkey: mint, isSigner: false, isWritable: true },
    { pubkey: account, isSigner: false, isWritable: true },
    { pubkey: authority, isSigner: true, isWritable: false },
  ]);
}

function transferTokens(source: PublicKey, destination: PublicKey, authority: PublicKey, tokens: number): TransactionInstruction {
  // Classic TokenInstruction.Transfer = 3; used only to model an external deposit.
  return tokenInstruction(Buffer.concat([Buffer.from([3]), amountLE(tokens)]), [
    { pubkey: source, isSigner: false, isWritable: true },
    { pubkey: destination, isSigner: false, isWritable: true },
    { pubkey: authority, isSigner: true, isWritable: false },
  ]);
}

describe("proofdock classic SPL token escrow on local validator", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.proofdock as Program<Proofdock>;
  const client = provider.wallet.publicKey;

  async function balance(tokenAccount: PublicKey): Promise<number> {
    return Number((await provider.connection.getTokenAccountBalance(tokenAccount)).value.amount);
  }

  async function fixture(deadlineOffsetSeconds = 3600) {
    const contractor = Keypair.generate();
    const mint = Keypair.generate();
    const clientToken = Keypair.generate();
    const contractorToken = Keypair.generate();

    const sig = await provider.connection.requestAirdrop(contractor.publicKey, LAMPORTS_PER_SOL);
    const latest = await provider.connection.getLatestBlockhash();
    await provider.connection.confirmTransaction({ signature: sig, ...latest }, "confirmed");

    const mintRent = await provider.connection.getMinimumBalanceForRentExemption(MINT_SPACE);
    const accountRent = await provider.connection.getMinimumBalanceForRentExemption(TOKEN_ACCOUNT_SPACE);
    const tx = new Transaction()
      .add(SystemProgram.createAccount({
        fromPubkey: client, newAccountPubkey: mint.publicKey,
        lamports: mintRent, space: MINT_SPACE, programId: TOKEN_PROGRAM,
      }))
      .add(initMint(mint.publicKey, client))
      .add(SystemProgram.createAccount({
        fromPubkey: client, newAccountPubkey: clientToken.publicKey,
        lamports: accountRent, space: TOKEN_ACCOUNT_SPACE, programId: TOKEN_PROGRAM,
      }))
      .add(initAccount3(clientToken.publicKey, mint.publicKey, client))
      .add(SystemProgram.createAccount({
        fromPubkey: client, newAccountPubkey: contractorToken.publicKey,
        lamports: accountRent, space: TOKEN_ACCOUNT_SPACE, programId: TOKEN_PROGRAM,
      }))
      .add(initAccount3(contractorToken.publicKey, mint.publicKey, contractor.publicKey))
      .add(mintTo(mint.publicKey, clientToken.publicKey, client, INITIAL_TOKENS));
    await provider.sendAndConfirm(tx, [mint, clientToken, contractorToken]);

    const id = new anchor.BN(randomBytes(6).readUIntBE(0, 6));
    const amount = new anchor.BN(ESCROW_TOKENS);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + deadlineOffsetSeconds);
    const metadataHash = Array.from(createHash("sha256").update("token-deliverable-v1").digest());
    const tokenEscrow = PublicKey.findProgramAddressSync(
      [Buffer.from("token-escrow"), client.toBuffer(), id.toArrayLike(Buffer, "le", 8)],
      program.programId,
    )[0];
    const vault = PublicKey.findProgramAddressSync(
      [Buffer.from("token-vault"), tokenEscrow.toBuffer()], program.programId,
    )[0];

    await program.methods.createTokenEscrow(id, amount, deadline, metadataHash)
      .accounts({
        client, contractor: contractor.publicKey,
        mint: mint.publicKey, clientToken: clientToken.publicKey,
        tokenEscrow, vault, tokenProgram: TOKEN_PROGRAM,
        systemProgram: SystemProgram.programId,
      }).rpc();
    return {
      contractor, mint: mint.publicKey, clientToken: clientToken.publicKey,
      contractorToken: contractorToken.publicKey, tokenEscrow, vault,
    };
  }

  it("funds the vault, releases exactly the agreed tokens, and closes escrow + vault", async () => {
    const f = await fixture();
    expect(await balance(f.vault)).to.equal(ESCROW_TOKENS);
    expect(await balance(f.clientToken)).to.equal(INITIAL_TOKENS - ESCROW_TOKENS);
    await program.methods.releaseTokenEscrow().accounts({
      client, contractor: f.contractor.publicKey, mint: f.mint,
      tokenEscrow: f.tokenEscrow, vault: f.vault,
      contractorToken: f.contractorToken, tokenProgram: TOKEN_PROGRAM,
    }).rpc();
    expect(await balance(f.contractorToken)).to.equal(ESCROW_TOKENS);
    expect(await provider.connection.getAccountInfo(f.vault)).to.equal(null);
    expect(await provider.connection.getAccountInfo(f.tokenEscrow)).to.equal(null);
  });

  it("rejects a substituted token payout account", async () => {
    const f = await fixture();
    let failed = false;
    try {
      await program.methods.releaseTokenEscrow().accounts({
        client, contractor: f.contractor.publicKey, mint: f.mint,
        tokenEscrow: f.tokenEscrow, vault: f.vault,
        contractorToken: f.clientToken, tokenProgram: TOKEN_PROGRAM,
      }).rpc();
    } catch (_) { failed = true; }
    expect(failed, "the contractor must own the payout token account").to.equal(true);
    expect(await balance(f.vault)).to.equal(ESCROW_TOKENS);
    expect(await balance(f.contractorToken)).to.equal(0);
  });

  it("blocks early refund, then returns tokens to the client after expiration", async () => {
    const f = await fixture(3);
    let failed = false;
    try {
      await program.methods.refundExpiredTokenEscrow().accounts({
        client, mint: f.mint, tokenEscrow: f.tokenEscrow,
        vault: f.vault, clientToken: f.clientToken, tokenProgram: TOKEN_PROGRAM,
      }).rpc();
    } catch (_) { failed = true; }
    expect(failed, "an unexpired escrow cannot be refunded").to.equal(true);
    await new Promise((resolve) => setTimeout(resolve, 5000));
    await program.methods.refundExpiredTokenEscrow().accounts({
      client, mint: f.mint, tokenEscrow: f.tokenEscrow,
      vault: f.vault, clientToken: f.clientToken, tokenProgram: TOKEN_PROGRAM,
    }).rpc();
    expect(await balance(f.clientToken)).to.equal(INITIAL_TOKENS);
    expect(await provider.connection.getAccountInfo(f.vault)).to.equal(null);
    expect(await provider.connection.getAccountInfo(f.tokenEscrow)).to.equal(null);
  });

  it("documents that unsolicited extra vault tokens follow the full-balance release policy", async () => {
    const f = await fixture();
    const extra = 25_000;
    await provider.sendAndConfirm(new Transaction().add(
      transferTokens(f.clientToken, f.vault, client, extra),
    ), []);
    expect(await balance(f.vault)).to.equal(ESCROW_TOKENS + extra);
    await program.methods.releaseTokenEscrow().accounts({
      client, contractor: f.contractor.publicKey, mint: f.mint,
      tokenEscrow: f.tokenEscrow, vault: f.vault,
      contractorToken: f.contractorToken, tokenProgram: TOKEN_PROGRAM,
    }).rpc();
    expect(await balance(f.contractorToken)).to.equal(ESCROW_TOKENS + extra);
    expect(await provider.connection.getAccountInfo(f.vault)).to.equal(null);
  });
});
