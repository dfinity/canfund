use candid::{CandidType, Deserialize, Nat, Principal};
use ic_cdk::api::call::RejectionCode;
use icrc_ledger_types::icrc1::account::Subaccount;
use icrc_ledger_types::icrc1::transfer::BlockIndex;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HeaderField(pub String, pub String);

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<HeaderField>,
    #[serde(with = "serde_bytes")]
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    #[serde(with = "serde_bytes")]
    pub body: Vec<u8>,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WithdrawArgs {
    #[serde(default)]
    pub from_subaccount: Option<Subaccount>,
    pub to: Principal,
    #[serde(default)]
    pub created_at_time: Option<u64>,
    pub amount: Nat,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum WithdrawError {
    BadFee {
        expected_fee: Nat,
    },
    InsufficientFunds {
        balance: Nat,
    },
    TooOld,
    CreatedInFuture {
        ledger_time: u64,
    },
    TemporarilyUnavailable,
    Duplicate {
        duplicate_of: BlockIndex,
    },
    FailedToWithdraw {
        fee_block: Option<Nat>,
        rejection_code: RejectionCode,
        rejection_reason: String,
    },
    GenericError {
        error_code: Nat,
        message: String,
    },
    InvalidReceiver {
        receiver: Principal,
    },
}
