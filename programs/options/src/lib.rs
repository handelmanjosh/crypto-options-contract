
use std::collections::HashMap;

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};

declare_id!("5eZCdZaW9kZFRZgC5kaHXQ1Tj4dAG6ythhcpS6hrp2fQ");

#[program]
pub mod options {
    use super::*;
    
    pub fn create(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
    pub fn delete(ctx: Context<Delete>) -> Result<()> {
        Ok(())
    }
    pub fn mint(ctx: Context<Buy>) -> Result<()> {
        Ok(())
    }
    pub fn exercise(ctx: Context<Exercise>) -> Result<()> {
        Ok(())
    }   
    pub fn crank(ctx: Context<Crank>) -> Result<()> {
        Ok(())
    }
}

#[account]
pub struct OptionAccount {
    pub options: HashMap<Pubkey, IndividualOption>,
}
#[derive(BorshSerialize, BorshDeserialize, BorshSchema, Clone)]
pub struct IndividualOption {
    pub creator: Pubkey,
    pub amount: u64,
    pub expiry: u64,
    pub token_price: u64,
    pub sol_price: u64,
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(mut)]
    pub principal_token_account: Account<'info, TokenAccount>,
    pub token_mint: Account<'info, Mint>,
    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"holder", token_mint.key().as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = program_authority
    )]
    pub holding_token_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"option", token_mint.key().as_ref()],
        bump,
        space = 8,
    )]
    pub option: Box<Account<'info, OptionAccount>>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct Delete {

}
#[derive(Accounts)]
pub struct Buy {

}
#[derive(Accounts)]
pub struct Exercise {

}
#[derive(Accounts)]
pub struct Crank {

}

