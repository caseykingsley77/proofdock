//! ProofDock: one funded SOL escrow per client-defined milestone.
//!
//! The client funds each escrow at creation. The client may release funds to
//! the contractor, or reclaim them after a deadline. A released or refunded
//! escrow is closed; the payer receives its rent deposit back.
//!
//! This is a demonstration project, NOT an audited production contract.

use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer};

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
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        validate_terms(amount, deadline, now)?;
        require!(
            ctx.accounts.client.key() != ctx.accounts.contractor.key(),
            EscrowError::SameParties
        );
        require!(metadata_hash != [0u8; 32], EscrowError::EmptyMetadataHash);

        let escrow = &mut ctx.accounts.escrow;
        escrow.id = id;
        escrow.client = ctx.accounts.client.key();
        escrow.contractor = ctx.accounts.contractor.key();
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

        emit!(EscrowRefunded {
            escrow: ctx.accounts.escrow.key(),
            client: ctx.accounts.client.key(),
            amount: ctx.accounts.escrow.amount,
        });
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CreateEscrow<'info> {
    #[account(mut)]
    pub client: Signer<'info>,

    /// The contractor is a system-owned wallet that receives released SOL.
    pub contractor: SystemAccount<'info>,

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

    #[account(
        mut,
        has_one = client,
        close = client,
        seeds = [b"escrow", client.key().as_ref(), &escrow.id.to_le_bytes()],
        bump = escrow.bump,
    )]
    pub escrow: Account<'info, Escrow>,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub id: u64,
    pub client: Pubkey,
    pub contractor: Pubkey,
    pub amount: u64,
    pub deadline: i64,
    pub metadata_hash: [u8; 32],
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
    #[msg("Escrow has not reached its deadline")]
    NotExpired,
    #[msg("Escrow balance underflow")]
    BalanceUnderflow,
    #[msg("Recipient balance overflow")]
    BalanceOverflow,
}

fn validate_terms(amount: u64, deadline: i64, now: i64) -> Result<()> {
    require!(amount > 0, EscrowError::ZeroAmount);
    require!(deadline > now, EscrowError::InvalidDeadline);
    let duration = deadline
        .checked_sub(now)
        .ok_or(EscrowError::ExcessiveDuration)?;
    require!(duration <= MAX_ESCROW_DURATION_SECONDS, EscrowError::ExcessiveDuration);
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
