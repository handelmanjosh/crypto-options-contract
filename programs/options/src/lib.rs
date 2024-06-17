
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount, transfer, Transfer, mint_to, MintTo, burn, Burn}
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
const POOL_FEE_BASIS_POINTS: u64 = 1;
const LIST_FEE_BASIS_POINTS: u64 = 2;
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
    pub fn create(ctx: Context<Create>, end_time: u64, strike_price: u64, amount: u64, call: bool, resellable: bool) -> Result<()> {
        // transfer underlying from user to token account
        if call {
            // option is a call, user can buy token at strike_price
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
        } else {
            // option is a put, user can sell token for strike_price, 
            let ix = anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.signer.key(),
                &ctx.accounts.program_authority.key(),
                strike_price * amount,
            );
            anchor_lang::solana_program::program::invoke(
                &ix,
                &[
                    ctx.accounts.signer.to_account_info(),
                    ctx.accounts.program_authority.to_account_info(),
                ],
            )?;
        }
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
        ctx.accounts.option_data_account.resellable = resellable;
        ctx.accounts.option_data_account.creator = ctx.accounts.signer.key();
        ctx.accounts.option_data_account.underlying_mint = ctx.accounts.underlying_mint.key();
        Ok(())
    }
    pub fn create_pool(ctx: Context<CreatePool>, base_price: u64, amount: u64) -> Result<()> {
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_option_account.to_account_info(),
                    to: ctx.accounts.program_holder_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                }
            ),
            amount,
        )?;
        ctx.accounts.pool.base_price = base_price;
        ctx.accounts.pool.option_mint = ctx.accounts.option_mint.key();
        ctx.accounts.pool.left = amount;
        ctx.accounts.pool.right = 0;
        Ok(())
    }
    pub fn swap_pool(ctx: Context<SwapPool>, base_price: u64, amount: u64, left_to_right: bool) -> Result<()> {
        let temp = ctx.accounts.pool.right;
        let price: u64 = match temp.checked_div(ctx.accounts.pool.left) {
            None => if ctx.accounts.pool.left == 0 { 
                return Err(CustomError::PoolEmpty.into())
            } else { 
                base_price
            },
            Some(val) => val + base_price,
        };
        if left_to_right {
            // swap left to right
            match ctx.accounts.pool.left.checked_sub(amount) {
                None => return Err(CustomError::PoolEmpty.into()),
                Some(_) => true,
            };
            match ctx.accounts.pool.right.checked_add(amount) {
                None => return Err(CustomError::PoolFull.into()),
                Some(_) => true,
            };
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_holder_account.to_account_info(),
                        to: ctx.accounts.user_option_token_account.to_account_info(),
                        authority: ctx.accounts.program_authority.to_account_info(),
                    },
                    &[&[b"auth", &[ctx.bumps.program_authority]]]
                ),
                amount,
            )?;
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.signer.to_account_info(),
                        to: ctx.accounts.program_authority.to_account_info(),
                    }
                ),
                price * amount,
            )?;
        } else {
            // swap right to left
            match ctx.accounts.pool.right.checked_sub(amount) {
                None => return Err(CustomError::PoolEmpty.into()),
                Some(_) => true,
            };
            match ctx.accounts.pool.left.checked_add(amount) {
                None => return Err(CustomError::PoolEmpty.into()),
                Some(_) => true,
            };
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_option_token_account.to_account_info(),
                        to: ctx.accounts.program_holder_account.to_account_info(),
                        authority: ctx.accounts.program_authority.to_account_info(),
                    },
                    &[&[b"auth", &[ctx.bumps.program_authority]]]
                ),
                amount,
            )?;
            let transferred = amount * price;
            **ctx.accounts.program_authority.try_borrow_mut_lamports()? -= transferred;
            **ctx.accounts.signer.try_borrow_mut_lamports()? += transferred;
        }
        Ok(())
    }
    pub fn close_pool(ctx: Context<ClosePool>, base_price: u64) -> Result<()> {
        let time = Clock::get()?.unix_timestamp as u64;
        let valid = match OptionDataAccount::try_from_slice(&ctx.accounts.option_data_account.data.borrow()).ok() {
            None => true,
            Some(account) => account.end_time < time,
        }; 
        if !valid {
            return Err(CustomError::OptionNotExpired.into())
        }
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
    pub fn close_listing(ctx: Context<CloseListing>, price: u64) -> Result<()> {
        let time = Clock::get()?.unix_timestamp as u64;
        let valid = match OptionDataAccount::try_from_slice(&ctx.accounts.option_data_account.data.borrow()).ok() {
            None => true,
            Some(account) => account.end_time < time,
        }; 
        if !valid || !(ctx.accounts.owner.key() == ctx.accounts.signer.key()) {
            return Err(CustomError::OptionNotExpired.into())
        }
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_holder_account.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info(),
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            ctx.accounts.listing.amount,
        )?;
        Ok(())  
    }
    pub fn exercise(ctx: Context<Exercise>, amount: u64) -> Result<()> {
        let time = Clock::get()?.unix_timestamp as u64;
        if time > ctx.accounts.option_data_account.end_time {
            return Err(CustomError::OptionExpired.into())
        }
        // need to put creator in account
        if ctx.accounts.option_data_account.call {
            let ix = anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.signer.key(),
                &ctx.accounts.creator.key(),
                ctx.accounts.option_data_account.strike_price * amount,
            );
            anchor_lang::solana_program::program::invoke(
                &ix,
                &[
                    ctx.accounts.signer.to_account_info(),
                    ctx.accounts.creator.to_account_info(),
                ],
            )?;
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.underlying_token_account.to_account_info(),
                        to: ctx.accounts.user_underlying_token_account.to_account_info(),
                        authority: ctx.accounts.program_authority.to_account_info()
                    },
                    &[&[b"auth", &[ctx.bumps.program_authority]]]
                ),
                amount
            )?;
        } else {
            transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_underlying_token_account.to_account_info(),
                        to: ctx.accounts.creator_token_account.to_account_info(),
                        authority: ctx.accounts.signer.to_account_info()
                    }
                ),
                amount
            )?;
            // now transfer sol from holder account to user
            let transferred = amount * ctx.accounts.option_data_account.strike_price;
            **ctx.accounts.program_authority.try_borrow_mut_lamports()? -= transferred;
            **ctx.accounts.signer.try_borrow_mut_lamports()? += transferred;
        }
        burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.option_mint.to_account_info(),
                    from: ctx.accounts.user_option_token_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info()
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            amount,
        )?;
        match ctx.accounts.option_data_account.amount_unexercised.checked_sub(amount) {
            None => return Err(CustomError::NotEnoughOptionToken.into()),
            Some(_) => true        
        };
        Ok(())
    }
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        if ctx.accounts.signer.key() != ctx.accounts.option_data_account.creator {
            return Err(CustomError::WrongOwner.into())
        }
        let time = Clock::get()?.unix_timestamp as u64;
        if time < ctx.accounts.option_data_account.end_time {
            return Err(CustomError::OptionNotExpired.into())
        }
        if ctx.accounts.option_data_account.call {
            transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.program_holder_account.to_account_info(),
                        to: ctx.accounts.user_underlying_account.to_account_info(),
                        authority: ctx.accounts.program_authority.to_account_info()
                    },
                    &[&[b"auth", &[ctx.bumps.program_authority]]]
                ),
                ctx.accounts.option_data_account.amount_unexercised
            )?;
        } else {
            let transferred = ctx.accounts.option_data_account.amount_unexercised * ctx.accounts.option_data_account.strike_price;
            **ctx.accounts.program_authority.try_borrow_mut_lamports()? -= transferred;
            **ctx.accounts.signer.try_borrow_mut_lamports()? += transferred;
        }
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
    #[msg("Not enough option token")]
    NotEnoughOptionToken,
    #[msg("Option expired")]
    OptionExpired,
    #[msg("Option not expired")]
    OptionNotExpired,
    #[msg("Invalid Account")]
    InvalidAccount,
    #[msg("Pool empty")]
    PoolEmpty,
    #[msg("Pool full")]
    PoolFull
}
#[account]
pub struct OptionDataAccount {
    creator: Pubkey,
    underlying_mint: Pubkey,
    end_time: u64,
    strike_price: u64,
    amount_unexercised: u64,
    call: bool,
    resellable: bool,
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
        space = 8 + 32 + 32 + 8 + 8 + 8 + 1 + 1,
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
        mut,
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
    /// CHECK: 
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
#[instruction(price: u64)]
pub struct CloseListing<'info> {
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [b"listing", option_mint.key().as_ref(), owner.key().as_ref(), price.to_be_bytes().as_ref()],
        bump,
        close = signer,
    )]
    pub listing: Account<'info, Listing>,
    #[account(
        mut,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = owner.key() == listing.owner @ CustomError::InvalidAccount
    )]
    /// CHECK: 
    pub owner: AccountInfo<'info>,
    /// CHECK: checked in program
    pub option_data_account: AccountInfo<'info>,
    #[account(
        mut,
        constraint = owner.key() == owner_token_account.owner
    )]
    pub owner_token_account: Account<'info, TokenAccount>,
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
    pub option_mint: Account<'info, Mint>,
    #[account(mut)]
    pub user_option_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"option_data_account", option_mint.key().as_ref()],
        bump
    )]
    pub option_data_account: Account<'info, OptionDataAccount>,
    #[account(
        mut,
        seeds = [b"underlying_token", option_data_account.underlying_mint.key().as_ref()],
        bump,
    )]
    pub underlying_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = creator.key() == option_data_account.creator @ CustomError::InvalidAccount
    )]
    /// CHECK: 
    pub creator: AccountInfo<'info>,
    #[account(
        mut,
        constraint = creator.key() == creator_token_account.owner @ CustomError::InvalidAccount
    )]
    pub creator_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_underlying_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [b"option_data_account", option_mint.key().as_ref()],
        bump,
        close = signer,
    )]
    pub option_data_account: Account<'info, OptionDataAccount>,
    #[account(
        mut,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub user_underlying_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK:
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[account]
pub struct Pool {
    option_mint: Pubkey,
    base_price: u64,
    right: u64,
    left: u64,
}
#[derive(Accounts)]
#[instruction(base_price: u64)]
pub struct CreatePool<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(mut)]
    pub user_option_account: Account<'info, TokenAccount>,
    pub option_mint: Account<'info, Mint>,
    #[account(
        init,
        payer = signer,
        seeds = [b"pool", option_mint.key().as_ref(), base_price.to_be_bytes().as_ref()],
        bump,
        space = 32 + 8 + 8 + 8,
    )]
    pub pool: Account<'info, Pool>,
    #[account(
        init_if_needed,
        payer = signer,
        seeds = [b"holder_account", option_mint.key().as_ref()],
        bump,
        token::mint = option_mint,
        token::authority = program_authority,
    )]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(base_price: u64)]
pub struct SwapPool<'info> {
    pub signer: Signer<'info>,
    #[account(mut)]
    pub user_option_token_account: Account<'info, TokenAccount>,
    pub option_mint: Account<'info, Mint>,
    #[account(
        mut,
        seeds = [b"pool", option_mint.key().as_ref(), base_price.to_be_bytes().as_ref()],
        bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(mut)]
    pub program_holder_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(base_price: u64)]
pub struct ClosePool<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub option_mint: Account<'info, Mint>,
    /// CHECK: checked in program
    pub option_data_account: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"pool", option_mint.key().as_ref(), base_price.to_be_bytes().as_ref()],
        bump,
        close = signer,
    )]
    pub pool: Account<'info, Pool>,
}



