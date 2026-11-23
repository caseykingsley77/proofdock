//! ProofDock: funded SOL and SPL-token escrows per client-defined milestone.
//!
//! The client funds each escrow at creation. The client may release funds to
//! the contractor, or reclaim them after a deadline. A released or refunded
//! escrow is closed; the payer receives its rent deposit back.
//!
//! This is a demonstration project, NOT an audited production contract.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};
use anchor_spl::token::{self, CloseAccount, Mint, Token, TokenAccount, TransferChecked};

// This is a development placeholder. Generate deployment keys and run
// `anchor keys sync` before building for a cluster.
declare_id!("7SKDkm9iT9AAKA1jkx7vWZGZnHf33fN1iQscyDKdp9os");

const MAX_ESCROW_DURATION_SECONDS: i64 = 365 * 24 * 60 * 60;

#[program]
pub mod proofdock {
    use super::*;

    /// Create a new milestone and transfer exactly `amount` lamports into its PDA.
    /// `metadata_hash` commits to off-chain deliverable terms without storing PII.
    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        id: u64,
        amount: u64,
        deadline: i64,
        metadata_hash: [u8; 32],
        arbitrator: Pubkey,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        validate_terms(amount, deadline, now)?;
        require!(
            ctx.accounts.client.key() != ctx.accounts.contractor.key(),
            EscrowError::SameParties
        );
        require!(metadata_hash != [0u8; 32], EscrowError::EmptyMetadataHash);
        validate_arbitrator(arbitrator, ctx.accounts.client.key(), ctx.accounts.contractor.key())?;

        let escrow = &mut ctx.accounts.escrow;
        escrow.id = id;
        escrow.client = ctx.accounts.client.key();
        escrow.contractor = ctx.accounts.contractor.key();
        escrow.arbitrator = arbitrator;
        escrow.disputed = false;
        escrow.amount = amount;
        escrow.deadline = deadline;
        escrow.metadata_hash = metadata_hash;
        escrow.bump = ctx.bumps.escrow;

        // Only the system-owned client may debit their lamports using a system CPI.
        // Crediting the program-owned escrow account is permitted.
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.key(),
                Transfer {
                    from: ctx.accounts.client.to_account_info(),
                    to: ctx.accounts.escrow.to_account_info(),
                },
            ),
            amount,
        )?;

        emit!(EscrowCreated {
            escrow: ctx.accounts.escrow.key(),
            client: ctx.accounts.client.key(),
            contractor: ctx.accounts.contractor.key(),
            amount,
            deadline,
            metadata_hash,
        });
        Ok(())
    }

    /// Only the original client may release funds to the recorded contractor.
    /// Anchor automatically closes the escrow after this instruction succeeds,
    /// returning the rent-exempt reserve to the client.
    pub fn release(ctx: Context<Release>) -> Result<()> {
        require!(!ctx.accounts.escrow.disputed, EscrowError::Disputed);
        let amount = ctx.accounts.escrow.amount;
        let escrow_info = ctx.accounts.escrow.to_account_info();
        let contractor_info = ctx.accounts.contractor.to_account_info();

        // A program may debit accounts it owns. Transfer the escrowed balance
        // directly, preserving the account's rent reserve until Anchor closes it.
        {
            let mut src_lamports = escrow_info.try_borrow_mut_lamports()?;
            **src_lamports = (**src_lamports)
                .checked_sub(amount)
                .ok_or(EscrowError::BalanceUnderflow)?;
        }
        {
            let mut dst_lamports = contractor_info.try_borrow_mut_lamports()?;
            **dst_lamports = (**dst_lamports)
                .checked_add(amount)
                .ok_or(EscrowError::BalanceOverflow)?;
        }

        emit!(EscrowReleased {
            escrow: ctx.accounts.escrow.key(),
            client: ctx.accounts.client.key(),
            contractor: ctx.accounts.contractor.key(),
            amount,
        });
        Ok(())
    }

    /// After the deadline, the original client can refund the deposited SOL.
    /// Closing the escrow sends its complete lamport balance (amount + rent)
    /// to the client. No contractor signature is required for an expired escrow.
    pub fn refund_expired(ctx: Context<RefundExpired>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(now >= ctx.accounts.escrow.deadline, EscrowError::NotExpired);
        require!(!ctx.accounts.escrow.disputed, EscrowError::Disputed);

        emit!(EscrowRefunded {
            escrow: ctx.accounts.escrow.key(),
            client: ctx.accounts.client.key(),
            amount: ctx.accounts.escrow.amount,
        });
        Ok(())
    }

    /// Lock classic SPL tokens in a program-controlled vault.
    /// The vault authority is the token escrow PDA, not the client wallet.
    pub fn create_token_escrow(
        ctx: Context<CreateTokenEscrow>,
        id: u64,
        amount: u64,
        deadline: i64,
        metadata_hash: [u8; 32],
        arbitrator: Pubkey,
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        validate_terms(amount, deadline, now)?;
        require!(
            ctx.accounts.client.key() != ctx.accounts.contractor.key(),
            EscrowError::SameParties
        );
        require!(metadata_hash != [0; 32], EscrowError::EmptyMetadataHash);
        validate_arbitrator(arbitrator, ctx.accounts.client.key(), ctx.accounts.contractor.key())?;

        let escrow = &mut ctx.accounts.token_escrow;
        escrow.id = id;
        escrow.client = ctx.accounts.client.key();
        escrow.contractor = ctx.accounts.contractor.key();
        escrow.arbitrator = arbitrator;
        escrow.disputed = false;
        escrow.mint = ctx.accounts.mint.key();
        escrow.amount = amount;
        escrow.deadline = deadline;
        escrow.metadata_hash = metadata_hash;
        escrow.bump = ctx.bumps.token_escrow;

        token::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.client_token.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.client.to_account_info(),
                },
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;

        emit!(TokenEscrowCreated {
            escrow: ctx.accounts.token_escrow.key(),
            client: ctx.accounts.client.key(),
            contractor: ctx.accounts.contractor.key(),
            mint: ctx.accounts.mint.key(),
            amount,
        });
        Ok(())
    }

    /// The client authorizes release of the vault's token balance.
    /// Any unsolicited extra tokens deposited in the vault go to the contractor,
    /// preventing a dust transfer from permanently blocking vault closure.
    pub fn release_token_escrow(ctx: Context<ReleaseTokenEscrow>) -> Result<()> {
        let amount = ctx.accounts.vault.amount;
        let client_key = ctx.accounts.client.key();
        let id_bytes = ctx.accounts.token_escrow.id.to_le_bytes();
        let bump = [ctx.accounts.token_escrow.bump];
        let signer_seeds: &[&[u8]] = &[
            b"token-escrow",
            client_key.as_ref(),
            id_bytes.as_ref(),
            &bump,
        ];
        let signer = &[signer_seeds];

        token::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.contractor_token.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
            )
            .with_signer(signer),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        token::close_account(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                CloseAccount {
                    account: ctx.accounts.vault.to_account_info(),
                    destination: ctx.accounts.client.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
            )
            .with_signer(signer),
        )?;

        emit!(TokenEscrowReleased {
            escrow: ctx.accounts.token_escrow.key(),
            client: client_key,
            contractor: ctx.accounts.contractor.key(),
            mint: ctx.accounts.mint.key(),
            amount,
        });
        Ok(())
    }

    /// After the deadline, return the vault's complete balance to the client.
    pub fn refund_expired_token_escrow(ctx: Context<RefundExpiredTokenEscrow>) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(
            now >= ctx.accounts.token_escrow.deadline,
            EscrowError::NotExpired
        );
        let amount = ctx.accounts.vault.amount;
        let client_key = ctx.accounts.client.key();
        let id_bytes = ctx.accounts.token_escrow.id.to_le_bytes();
        let bump = [ctx.accounts.token_escrow.bump];
        let signer_seeds: &[&[u8]] = &[
            b"token-escrow",
            client_key.as_ref(),
            id_bytes.as_ref(),
            &bump,
        ];
        let signer = &[signer_seeds];

        token::transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.client_token.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
            )
            .with_signer(signer),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        token::close_account(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                CloseAccount {
                    account: ctx.accounts.vault.to_account_info(),
                    destination: ctx.accounts.client.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
            )
            .with_signer(signer),
        )?;

        emit!(TokenEscrowRefunded {
            escrow: ctx.accounts.token_escrow.key(),
            client: client_key,
            mint: ctx.accounts.mint.key(),
            amount,
        });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateEscrow<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    /// Both parties explicitly accept the arbitrator chosen for this milestone.
    pub contractor: Signer<'info>,

    #[account(
        init,
        payer = client,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [b"escrow", client.key().as_ref(), &id.to_le_bytes()],
        bump
    )]
    pub escrow: Account<'info, Escrow>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Release<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    #[account(
        mut,
        has_one = client,
        close = client,
        seeds = [b"escrow", client.key().as_ref(), &escrow.id.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(mut, address = escrow.contractor)]
    pub contractor: SystemAccount<'info>,
}

#[derive(Accounts)]
pub struct RefundExpired<'info> {
    #[account(mut)]
    pub client: Signer<'info>,
    pub contractor: Signer<'info>,

    #[account(
        mut,
        has_one = client,
        has_one = contractor,
        close = client,
        seeds = [b"escrow", client.key().as_ref(), &escrow.id.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

/// Account constraints ensure the client owns the source tokens and that
/// the vault is controlled exclusively by the escrow PDA.
#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateTokenEscrow<'info> {
    #[account(mut)]
    pub client: Signer<'info>,
    pub contractor: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = client,
    )]
    pub client_token: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = client,
        space = 8 + TokenEscrow::INIT_SPACE,
        seeds = [b"token-escrow", client.key().as_ref(), &id.to_le_bytes()],
        bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    #[account(
        init,
        payer = client,
        token::mint = mint,
        token::authority = token_escrow,
        seeds = [b"token-vault", token_escrow.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ReleaseTokenEscrow<'info> {
    #[account(mut)]
    pub client: Signer<'info>,
    pub contractor: SystemAccount<'info>,
    #[account(address = token_escrow.mint)]
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        has_one = client,
        has_one = contractor,
        has_one = mint,
        close = client,
        seeds = [b"token-escrow", client.key().as_ref(), &token_escrow.id.to_le_bytes()],
        bump = token_escrow.bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = token_escrow,
        seeds = [b"token-vault", token_escrow.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = contractor)]
    pub contractor_token: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = client)]
    pub client_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RefundExpiredTokenEscrow<'info> {
    #[account(mut)]
    pub client: Signer<'info>,
    pub contractor: Signer<'info>,
    #[account(address = token_escrow.mint)]
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        has_one = client,
        has_one = contractor,
        has_one = mint,
        close = client,
        seeds = [b"token-escrow", client.key().as_ref(), &token_escrow.id.to_le_bytes()],
        bump = token_escrow.bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = token_escrow,
        seeds = [b"token-vault", token_escrow.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = client)]
    pub client_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}


#[derive(Accounts)]
pub struct Dispute<'info> {
    pub party: Signer<'info>,
    #[account(
        mut,
        seeds = [b"escrow", escrow.client.as_ref(), &escrow.id.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

#[derive(Accounts)]
pub struct ResolveDispute<'info> {
    pub arbitrator: Signer<'info>,
    #[account(mut)]
    pub client: SystemAccount<'info>,
    #[account(mut)]
    pub contractor: SystemAccount<'info>,
    #[account(
        mut,
        has_one = arbitrator,
        has_one = client,
        has_one = contractor,
        close = client,
        seeds = [b"escrow", client.key().as_ref(), &escrow.id.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

#[derive(Accounts)]
pub struct DisputeTokenEscrow<'info> {
    pub party: Signer<'info>,
    #[account(
        mut,
        seeds = [b"token-escrow", token_escrow.client.as_ref(), &token_escrow.id.to_le_bytes()],
        bump = token_escrow.bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
}

#[derive(Accounts)]
pub struct ResolveTokenDispute<'info> {
    pub arbitrator: Signer<'info>,
    #[account(mut)]
    pub client: SystemAccount<'info>,
    pub contractor: SystemAccount<'info>,
    #[account(address = token_escrow.mint)]
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        has_one = arbitrator,
        has_one = client,
        has_one = contractor,
        has_one = mint,
        close = client,
        seeds = [b"token-escrow", client.key().as_ref(), &token_escrow.id.to_le_bytes()],
        bump = token_escrow.bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = token_escrow,
        seeds = [b"token-vault", token_escrow.key().as_ref()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = client)]
    pub client_token: Account<'info, TokenAccount>,
    #[account(mut, token::mint = mint, token::authority = contractor)]
    pub contractor_token: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct TokenEscrow {
    pub id: u64,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub arbitrator: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub deadline: i64,
    pub metadata_hash: [u8; 32],
    pub disputed: bool,
    pub bump: u8,
}

#[event]
pub struct EscrowDisputed {
    pub escrow: Pubkey,
    pub party: Pubkey,
}

#[event]
pub struct EscrowResolved {
    pub escrow: Pubkey,
    pub arbitrator: Pubkey,
    pub contractor_paid: bool,
    pub amount: u64,
}

#[event]
pub struct TokenEscrowCreated {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokenEscrowReleased {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct TokenEscrowRefunded {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub id: u64,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub arbitrator: Pubkey,
    pub amount: u64,
    pub deadline: i64,
    pub metadata_hash: [u8; 32],
    pub disputed: bool,
    pub bump: u8,
}

#[event]
pub struct EscrowCreated {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub amount: u64,
    pub deadline: i64,
    pub metadata_hash: [u8; 32],
}

#[event]
pub struct EscrowReleased {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub amount: u64,
}

#[event]
pub struct EscrowRefunded {
    pub escrow: Pubkey,
    pub client: Pubkey,
    pub amount: u64,
}

#[error_code]
pub enum EscrowError {
    #[msg("Escrow amount must be greater than zero")]
    ZeroAmount,
    #[msg("Deadline must be in the future")]
    InvalidDeadline,
    #[msg("Escrow deadline cannot exceed one year")]
    ExcessiveDuration,
    #[msg("Client and contractor must be different wallets")]
    SameParties,
    #[msg("Metadata commitment must be nonzero")]
    EmptyMetadataHash,
    #[msg("Arbitrator must be a nonzero third-party public key")]
    InvalidArbitrator,
    #[msg("Only an escrow party may open a dispute")]
    UnauthorizedParty,
    #[msg("Escrow is under dispute and requires arbitrator resolution")]
    Disputed,
    #[msg("Escrow is not disputed")]
    NotDisputed,
    #[msg("Escrow is already disputed")]
    AlreadyDisputed,
    #[msg("Token vault has insufficient funds for agreed payout")]
    InsufficientVaultBalance,
    #[msg("Escrow has not reached its deadline")]
    NotExpired,
    #[msg("Escrow balance underflow")]
    BalanceUnderflow,
    #[msg("Recipient balance overflow")]
    BalanceOverflow,
}


#[allow(clippy::too_many_arguments)]
fn settle_token_vault<'info>(
    token_program: AccountInfo<'info>,
    token_escrow: AccountInfo<'info>,
    vault: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    client_token: AccountInfo<'info>,
    contractor_token: AccountInfo<'info>,
    client: AccountInfo<'info>,
    decimals: u8,
    agreed: u64,
    balance: u64,
    client_key: Pubkey,
    id: u64,
    bump: u8,
    pay_contractor: bool,
) -> Result<()> {
    // Deposits from other sources cannot be attributed to individual donors.
    // Return excess to the recorded client rather than inflating contractor payout.
    let surplus = balance.checked_sub(agreed).ok_or(EscrowError::InsufficientVaultBalance)?;
    let id_bytes = id.to_le_bytes();
    let bump_bytes = [bump];
    let seeds: &[&[u8]] = &[
        b"token-escrow",
        client_key.as_ref(),
        id_bytes.as_ref(),
        &bump_bytes,
    ];
    let signer = &[seeds];
    if pay_contractor {
        token::transfer_checked(
            CpiContext::new(token_program.clone(), TransferChecked {
                from: vault.clone(),
                mint: mint.clone(),
                to: contractor_token,
                authority: token_escrow.clone(),
            }).with_signer(signer),
            agreed,
            decimals,
        )?;
    }
    let client_amount = if pay_contractor { surplus } else { balance };
    if client_amount > 0 {
        token::transfer_checked(
            CpiContext::new(token_program.clone(), TransferChecked {
                from: vault.clone(),
                mint: mint.clone(),
                to: client_token,
                authority: token_escrow.clone(),
            }).with_signer(signer),
            client_amount,
            decimals,
        )?;
    }
    token::close_account(
        CpiContext::new(token_program, CloseAccount {
            account: vault,
            destination: client,
            authority: token_escrow,
        }).with_signer(signer),
    )?;
    Ok(())
}

fn validate_arbitrator(arbitrator: Pubkey, client: Pubkey, contractor: Pubkey) -> Result<()> {
    require!(arbitrator != Pubkey::default() && arbitrator != client && arbitrator != contractor, EscrowError::InvalidArbitrator);
    Ok(())
}

fn validate_terms(amount: u64, deadline: i64, now: i64) -> Result<()> {
    require!(amount > 0, EscrowError::ZeroAmount);
    require!(deadline > now, EscrowError::InvalidDeadline);
    let duration = deadline
        .checked_sub(now)
        .ok_or(EscrowError::ExcessiveDuration)?;
    require!(
        duration <= MAX_ESCROW_DURATION_SECONDS,
        EscrowError::ExcessiveDuration
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_valid_milestone() {
        assert!(validate_terms(1_000_000, 1_600, 1_000).is_ok());
    }

    #[test]
    fn rejects_zero_value() {
        assert!(validate_terms(0, 1_600, 1_000).is_err());
    }

    #[test]
    fn rejects_expired_or_equal_deadline() {
        assert!(validate_terms(10, 1_000, 1_000).is_err());
        assert!(validate_terms(10, 900, 1_000).is_err());
    }

    #[test]
    fn rejects_excessively_long_duration() {
        assert!(validate_terms(10, 1_000 + MAX_ESCROW_DURATION_SECONDS + 1, 1_000).is_err());
    }

    #[test]
    fn permits_exactly_one_year() {
        assert!(validate_terms(10, 1_000 + MAX_ESCROW_DURATION_SECONDS, 1_000).is_ok());
    }
}
