
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, transfer, Transfer, mint_to, MintTo}
};

declare_id!("BfrkttNPsNutRR3PKtsh8N2cN3EhkqXJWRwG5RSMU8AK");
/* 
    definitions:
    principal token : token used to buy options
    option token : token representing an option
    underlying token : token that the option is for.

    pairing of option token : principal token
    option token is tradeable for underlying token
*/
const OPTION_MINT_DECIMALS: u8 = 6;
#[program]
pub mod options {
    use super::*;
    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
    pub fn create_holder_account(_ctx: Context<CreateHolderAccount>) -> Result<()> {
        Ok(())
    }
    // should initialize an option token, set its data, and mint it to the user
    // if option token already exists, should mint it to the user
    // should take underlying token from the user and hold as collateral. 
    pub fn create(ctx: Context<Create>, end_time: u64, strike_price: u64, amount: u64, call: bool) -> Result<()> {
        // transfer underlying from user to token account
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_underlying_token_account.to_account_info(),
                    to: ctx.accounts.underlying_token_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                }
            ),
            amount,
        )?;
        // mint option token to user
        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.option_mint.to_account_info(),
                    to: ctx.accounts.user_option_token_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            amount,
        )?;
        ctx.accounts.option_data_account.end_time = end_time;
        ctx.accounts.option_data_account.strike_price = strike_price;
        ctx.accounts.option_data_account.amount_unexercised = amount;
        ctx.accounts.option_data_account.call = call;
        ctx.accounts.option_data_account.creator = ctx.accounts.signer.key();
        ctx.accounts.option_data_account.underlying_mint = ctx.accounts.underlying_mint.key();
        Ok(())
    }
    pub fn list(ctx: Context<List>, amount: u64, price: u64) -> Result<()> {
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_option_token_account.to_account_info(),
                    to: ctx.accounts.program_holder_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                }
            ),
            amount,
        )?;
        ctx.accounts.list_account.amount += amount;
        ctx.accounts.list_account.price = price;
        ctx.accounts.list_account.owner = ctx.accounts.signer.key();
        ctx.accounts.list_account.underlying_mint = ctx.accounts.option_data_account.underlying_mint.key();
        ctx.accounts.list_account.option_mint = ctx.accounts.option_mint.key();
        Ok(())
    }
    pub fn buy(ctx: Context<Buy>, _price: u64, amount: u64) -> Result<()> {
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.signer.key(),
            &ctx.accounts.owner.key(),
            ctx.accounts.listing.price * amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.owner.to_account_info(),
            ],
        )?;
        match ctx.accounts.listing.amount.checked_sub(amount) {
            None => return Err(CustomError::ListingEmpty.into()),
            Some(_) => true,
        };
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_holder_account.to_account_info(),
                    to: ctx.accounts.user_holder_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            amount,
        )?;
        Ok(())
    }
    pub fn exercise(ctx: Context<Exercise>) -> Result<()> {
        Ok(())
    }
}
#[error_code]
pub enum CustomError {
    #[msg("Strike price not reached")]
    StrikePriceNotReached,
    #[msg("Token price not found")]
    TokenPriceNotFound,
    #[msg("Listing empty")]
    ListingEmpty,
    #[msg("Wrong owner")]
    WrongOwner,
}
#[account]
pub struct OptionDataAccount {
    creator: Pubkey,
    underlying_mint: Pubkey,
    end_time: u64,
    strike_price: u64,
    amount_unexercised: u64,
    call: bool,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        seeds = [b"auth"],
        bump,
        payer = signer,
        space = 8
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub underlying_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_underlying_token_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"underlying_token", underlying_mint.key().as_ref()],
        bump,
        token::authority = program_authority,
        token::mint = underlying_mint
    )]
    pub underlying_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = signer,
        mint::authority = program_authority,
        mint::decimals = OPTION_MINT_DECIMALS,
    )]
    pub option_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = signer,
        associated_token::mint = option_mint,
        associated_token::authority = signer,
    )]
    pub user_option_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        seeds = [b"option_data_account", option_mint.key().as_ref()],
        bump,
        payer = signer,
        space = 8 + 32 + 32 + 8 + 8 + 8 + 1,
    )]
    pub option_data_account: Account<'info, OptionDataAccount>,
    #[account(
        seeds = [b"auth"],
        bump
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
#[account]
pub struct Listing {
    underlying_mint: Pubkey,
    option_mint: Pubkey,
    owner: Pubkey,
    amount: u64,
    price: u64,
}
#[derive(Accounts)]
pub struct CreateHolderAccount<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = signer,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
        token::authority = program_authority,
        token::mint = option_mint
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
#[instruction(amount: u64, price: u64)]
pub struct List<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_option_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"option_data_account", option_mint.key().as_ref()],
        bump
    )]
    pub option_data_account: Account<'info, OptionDataAccount>,
    #[account(
        mut,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"listing", option_mint.key().as_ref(), signer.key().as_ref(), price.to_be_bytes().as_ref()],
        bump,
        space = 8 + 32 + 32 + 32 + 8 + 8
    )]
    pub list_account: Account<'info, Listing>,
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
#[instruction(price: u64)]
pub struct Buy<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    #[account(mut)]
    pub owner: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"listing", option_mint.key().as_ref(), owner.key().as_ref(), price.to_be_bytes().as_ref()],
        bump,
    )]
    pub listing: Account<'info, Listing>,
    #[account(
        mut,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_holder_account: Account<'info, TokenAccount>,
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
pub struct Exercise<'info> {
    pub signer: Signer<'info>,
}


