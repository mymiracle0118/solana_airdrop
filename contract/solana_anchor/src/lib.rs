pub mod utils;
use borsh::{BorshDeserialize,BorshSerialize};
use {
    crate::utils::*,
    anchor_lang::{
        prelude::*,
        AnchorDeserialize,
        AnchorSerialize,
        Key,
        solana_program::{
            program_pack::Pack,
            sysvar::{clock::Clock},
            msg
        }      
    },
    spl_token::state,
    metaplex_token_metadata::{
        state::{
            MAX_SYMBOL_LENGTH,
        }
    }
};
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod solana_anchor {
    use super::*;

    pub fn init_pool(
        ctx : Context<InitPool>,
        _bump : u8,
        _schedule : Vec<Schedule>,
        _period : u64,
        _stake_collection : String,
        ) -> ProgramResult {
        msg!("Init Pool");
        let pool = &mut ctx.accounts.pool;
        let reward_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.reward_account.data.borrow())?;
        if reward_account.owner != pool.key() {
            msg!("Reward token account's owner need to be pool");
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if reward_account.mint != *ctx.accounts.reward_mint.key {
            msg!("Reward token account's mint need to be reward_mint");
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if _period == 0 {
            msg!("Period need to be bigger than zero");
            return Err(PoolError::InvalidPeriod.into());
        }
        pool.owner = *ctx.accounts.owner.key;
        pool.rand = *ctx.accounts.rand.key;
        pool.reward_mint = *ctx.accounts.reward_mint.key;
        pool.reward_account = *ctx.accounts.reward_account.key;
        pool.schedule = _schedule;
        pool.period = _period;
        pool.stake_collection = _stake_collection;
        pool.bump = _bump;
        Ok(())
    }

    pub fn init_nft_data(
        ctx : Context<InitNftData>,
        _bump : u8
        ) -> ProgramResult {
        let nft_data = &mut ctx.accounts.nft_data;
        nft_data.nft_mint = *ctx.accounts.nft_mint.key;
        nft_data.last_airdrop_time = 0;
        nft_data.bump = _bump;
        Ok(())
    }

    pub fn airdrop(
        ctx : Context<Airdrop>,
        ) -> ProgramResult {
        let pool = &mut ctx.accounts.pool;
        let nft_data = &mut ctx.accounts.nft_data;
        let clock = (Clock::from_account_info(&ctx.accounts.clock)?).unix_timestamp as u64;
        let nft_mint : state::Mint = state::Mint::unpack_from_slice(&ctx.accounts.nft_mint.data.borrow())?;
        let nft_account : state::Account = state::Account::unpack_from_slice(&ctx.accounts.nft_account.data.borrow())?;
        let nft_metadata : metaplex_token_metadata::state::Metadata =  metaplex_token_metadata::state::Metadata::from_account_info(&ctx.accounts.nft_metadata)?; 
        
        if nft_mint.decimals != 0 || nft_mint.supply != 1 {
            msg!("This mint is not proper nft");
            return Err(PoolError::InvalidTokenMint.into());
        }
        if nft_account.mint != *ctx.accounts.nft_mint.key {
            msg!("Not match mint address");
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_account.owner != *ctx.accounts.owner.key || nft_account.amount != 1 {
            msg!("owner or amount is invalid");
            return Err(PoolError::InvalidTokenAccount.into());
        }
        if nft_metadata.mint != *ctx.accounts.nft_mint.key {
            msg!("mint of metadata is not matched");
            return Err(PoolError::InvalidMetadata.into());
        }
        if (&nft_metadata.data.symbol).eq(&pool.stake_collection) {
            msg!("Not match collection symbol");
            return Err(PoolError::InvalidMetadata.into());
        }
        if *ctx.accounts.token_from.key != pool.reward_account {
            msg!("token_from need to be reward_account of pool");
            return Err(PoolError::InvalidTokenAccount.into())
        }
        let mut confirmed = false;
        let mut amount = 0;

        for s in pool.schedule.iter(){
            if s.airdrop_time < clock && clock < s.airdrop_time+pool.period && nft_data.last_airdrop_time < s.airdrop_time{
                confirmed = true;
                amount = s.airdrop_amount;
                break;
            }
        }

        if !confirmed {
            return Err(PoolError::InvalidTime.into());
        }

        let pool_seeds = &[pool.rand.as_ref(),&[pool.bump]];        
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.token_from.clone(),
                destination : ctx.accounts.token_to.clone(),
                authority : pool.to_account_info().clone(),
                authority_signer_seeds : pool_seeds,
                token_program : ctx.accounts.token_program.clone(),
                amount : amount,
            }
        )?;

        nft_data.last_airdrop_time = clock;

        Ok(())        
    }

    pub fn redeem_token(
        ctx : Context<RedeemToken>,
        _amount : u64
        )->ProgramResult{
        let pool = &mut ctx.accounts.pool;
        if *ctx.accounts.token_from.key != pool.reward_account {
            msg!("token_from need to be reward_account of pool");
            return Err(PoolError::InvalidTokenAccount.into())
        }
        let pool_seeds = &[pool.rand.as_ref(),&[pool.bump]];        
        spl_token_transfer(
            TokenTransferParams{
                source : ctx.accounts.token_from.clone(),
                destination : ctx.accounts.token_to.clone(),
                authority : pool.to_account_info().clone(),
                authority_signer_seeds : pool_seeds,
                token_program : ctx.accounts.token_program.clone(),
                amount : _amount,
            }
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct RedeemToken<'info>{
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(mut,has_one=owner)]
    pool : ProgramAccount<'info,Pool>,

    #[account(mut,owner=spl_token::id())]
    token_from : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    token_to : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,    
}

#[derive(Accounts)]
pub struct Airdrop<'info>{
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(mut)]
    pool : ProgramAccount<'info,Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    nft_metadata : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    nft_account : AccountInfo<'info>,

    #[account(mut,seeds=[(*nft_mint.key).as_ref(), pool.key().as_ref()], bump=nft_data.bump)]
    nft_data : ProgramAccount<'info, NftData>,

    #[account(mut,owner=spl_token::id())]
    token_from : AccountInfo<'info>,

    #[account(mut,owner=spl_token::id())]
    token_to : AccountInfo<'info>,

    #[account(address=spl_token::id())]
    token_program : AccountInfo<'info>,

    clock : AccountInfo<'info>,    
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct InitNftData<'info>{
    #[account(mut)]
    payer : AccountInfo<'info>,

    pool : ProgramAccount<'info, Pool>,

    #[account(owner=spl_token::id())]
    nft_mint : AccountInfo<'info>,

    #[account(init, seeds=[(*nft_mint.key).as_ref(), pool.key().as_ref()], bump=_bump, payer=payer, space=8+NFT_DATA_SIZE)]
    nft_data : ProgramAccount<'info,NftData>,

    system_program : Program<'info,System>,
}

#[derive(Accounts)]
#[instruction(_bump : u8)]
pub struct InitPool<'info> {
    #[account(mut, signer)]
    owner : AccountInfo<'info>,

    #[account(init, seeds=[(*rand.key).as_ref()], bump=_bump, payer=owner, space=8+POOL_SIZE)]
    pool : ProgramAccount<'info, Pool>,

    rand : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    reward_mint : AccountInfo<'info>,

    #[account(owner=spl_token::id())]
    reward_account : AccountInfo<'info>,

    system_program : Program<'info,System>,
}

pub const MAX_NUM : usize = 10;
pub const POOL_SIZE : usize = 32 + 32 + 32 + 32 + 4 + MAX_NUM * 16 + 4 + MAX_SYMBOL_LENGTH + 1;
pub const NFT_DATA_SIZE : usize = 32 + 8 + 1;

#[account]
#[derive(Default)]
pub struct Pool {
    pub owner : Pubkey,
    pub rand : Pubkey,
    pub reward_mint : Pubkey,
    pub reward_account : Pubkey,
    pub schedule : Vec<Schedule>,
    pub period : u64,
    pub stake_collection : String,
    pub bump : u8,
}

#[derive(AnchorSerialize,AnchorDeserialize,Clone,Copy)]
pub struct Schedule{
    pub airdrop_time : u64,
    pub airdrop_amount : u64,
}

#[account]
pub struct NftData{
    pub nft_mint : Pubkey,
    pub last_airdrop_time : u64,
    pub bump : u8,
}


#[error]
pub enum PoolError {
    #[msg("Token mint to failed")]
    TokenMintToFailed,

    #[msg("Token set authority failed")]
    TokenSetAuthorityFailed,

    #[msg("Token transfer failed")]
    TokenTransferFailed,

    #[msg("Invalid token account")]
    InvalidTokenAccount,

    #[msg("Invalid token mint")]
    InvalidTokenMint,

    #[msg("Invalid metadata")]
    InvalidMetadata,

    #[msg("Invalid period")]
    InvalidPeriod,

    #[msg("Invalid Time")]
    InvalidTime,
}