use solana_program::{
    account_info::next_account_info,
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_program::program::invoke;
use solana_program::sysvar::clock::Clock;
use solana_program::sysvar::Sysvar;
use pyth_sdk_solana::state::{SolanaPriceAccount, Price, PriceFeed};

const STALENESS_THRESHOLD : u64 = 60; // staleness threshold in seconds

// Define the program entrypoint function
entrypoint!(process_instruction);

// Define the entry point function for the program
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Get the accounts passed to the program
    let accounts_iter = &mut accounts.iter();
    let from_account = next_account_info(accounts_iter)?;
    let to_account = next_account_info(accounts_iter)?;
    let pyth_sol_usd_account = next_account_info(accounts_iter)?;

    let price_feed: PriceFeed = SolanaPriceAccount::account_info_to_feed(pyth_sol_usd_account).unwrap();
    let current_timestamp = Clock::get()?.unix_timestamp;
    let current_price: Price = price_feed.get_price_no_older_than(current_timestamp, STALENESS_THRESHOLD).unwrap();

    // Parse the instruction data to get the amount of USD to pay
    let amount_usd = instruction_data.get(0..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ProgramError::InvalidInstructionData)?;

    // calculate the amount of sol to withdraw
    let amount_to_withdraw = calculate_sol_amount(amount_usd, current_price);

    // Check if the caller has sufficient balance to withdraw the specified amount
    if from_account.lamports() < amount_to_withdraw {
        return Err(ProgramError::InsufficientFunds);
    }

    // Transfer the specified amount of SOL from the caller's account to the target account
    invoke(
        &solana_program::system_instruction::transfer(
            from_account.key,
            to_account.key,
            amount_to_withdraw,
        ),
        &[from_account.clone(), to_account.clone()],
    )?;

    msg!("{} SOL withdrawn from caller's account", amount_to_withdraw);

    Ok(())
}

fn calculate_sol_amount(usd_qty: u64, price: Price) -> u64 {
    let fee_lamports = (10u64)
        .checked_pow((10 - price.expo) as u32).unwrap()
        .checked_mul(usd_qty).unwrap()
        .checked_div(price.price as u64).unwrap();
    return  fee_lamports as u64
}