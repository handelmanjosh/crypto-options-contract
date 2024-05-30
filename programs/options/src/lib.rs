use anchor_lang::prelude::*;

declare_id!("5eZCdZaW9kZFRZgC5kaHXQ1Tj4dAG6ythhcpS6hrp2fQ");

#[program]
pub mod options {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
