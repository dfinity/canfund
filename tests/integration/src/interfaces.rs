use candid::{CandidType, Principal};
use ic_ledger_types::{AccountBalanceArgs, AccountIdentifier, Memo, Subaccount, Tokens, TransferArgs, TransferError, MAINNET_LEDGER_CANISTER_ID};
use pocket_ic::{update_candid_as, PocketIc};
use std::collections::{HashMap, HashSet};

#[derive(CandidType)]
pub enum NnsLedgerCanisterPayload {
    Init(NnsLedgerCanisterInitPayload),
}

#[derive(CandidType)]
pub struct NnsLedgerCanisterInitPayload {
    pub minting_account: String,
    pub initial_values: HashMap<String, Tokens>,
    pub send_whitelist: HashSet<Principal>,
    pub transfer_fee: Option<Tokens>,
    pub token_symbol: Option<String>,
    pub token_name: Option<String>,
}

pub const ICP: u64 = 100_000_000; // in e8s
pub const ICP_FEE: u64 = 10_000; // in e8s

pub fn get_icp_account_balance(env: &PocketIc, account_id: AccountIdentifier) -> u64 {
    let ledger_canister_id = MAINNET_LEDGER_CANISTER_ID;
    let account_balance_args = AccountBalanceArgs {
        account: account_id,
    };
    let res: (Tokens,) = update_candid_as(
        env,
        ledger_canister_id,
        Principal::anonymous(),
        "account_balance",
        (account_balance_args,),
    )
    .unwrap();
    res.0.e8s()
}

pub fn send_icp_to_account(
    env: &PocketIc,
    sender_id: Principal,
    beneficiary_account: AccountIdentifier,
    e8s: u64,
    memo: u64,
    from_subaccount: Option<Subaccount>,
) -> Result<u64, TransferError> {
    let ledger_canister_id = MAINNET_LEDGER_CANISTER_ID;
    let transfer_args = TransferArgs {
        memo: Memo(memo),
        amount: Tokens::from_e8s(e8s),
        fee: Tokens::from_e8s(ICP_FEE),
        from_subaccount,
        to: beneficiary_account,
        created_at_time: None,
    };
    let res: (Result<u64, TransferError>,) = update_candid_as(
        env,
        ledger_canister_id,
        sender_id,
        "transfer",
        (transfer_args,),
    )
    .unwrap();
    res.0
}
