use crate::types::{WithdrawArgs, WithdrawError};
use async_trait::async_trait;
use candid::Principal;
use ic_cdk::api::call::CallResult;
use ic_ledger_types::{transfer, TransferArgs, TransferResult};
use icrc_ledger_types::icrc1::transfer::BlockIndex;

#[async_trait]
pub trait LedgerCanister: Send + Sync {
    async fn transfer(&self, args: TransferArgs) -> CallResult<TransferResult>;
}

pub struct IcLedgerCanister {
    canister_id: Principal,
}

impl IcLedgerCanister {
    pub fn new(canister_id: Principal) -> Self {
        Self { canister_id }
    }
}

#[async_trait]
impl LedgerCanister for IcLedgerCanister {
    async fn transfer(&self, args: TransferArgs) -> CallResult<TransferResult> {
        transfer(self.canister_id, args).await
    }
}

#[async_trait]
pub trait WithdrawableLedgerCanister: Send + Sync {
    async fn withdraw(&self, args: WithdrawArgs) -> CallResult<Result<BlockIndex, WithdrawError>>;
}

pub struct CyclesLedgerCanister {
    canister_id: Principal,
}

impl CyclesLedgerCanister {
    pub fn new(canister_id: Principal) -> Self {
        Self { canister_id }
    }
}

#[async_trait]
impl WithdrawableLedgerCanister for CyclesLedgerCanister {
    async fn withdraw(&self, args: WithdrawArgs) -> CallResult<Result<BlockIndex, WithdrawError>> {
        let (result,) = ic_cdk::call::<(WithdrawArgs,), (Result<BlockIndex, WithdrawError>,)>(
            self.canister_id,
            "withdraw",
            (args,),
        )
        .await?;

        Ok(result)
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use super::*;
    use async_trait::async_trait;
    use tokio::sync::RwLock;

    #[derive(Default)]
    pub struct TestLedgerCanister {
        pub transfer_called_with: Arc<RwLock<Vec<TransferArgs>>>,
        pub returns_with: Option<CallResult<TransferResult>>,
    }
    #[async_trait]
    impl LedgerCanister for TestLedgerCanister {
        async fn transfer(&self, args: TransferArgs) -> CallResult<TransferResult> {
            let mut locked = self.transfer_called_with.write().await;
            locked.push(args);

            if let Some(value) = &self.returns_with {
                return value.clone();
            }

            Ok(Ok(0))
        }
    }

    #[derive(Default)]
    pub struct TestCyclesLedgerCanister {
        pub transfer_called_with: Arc<RwLock<Vec<WithdrawArgs>>>,
        pub returns_with: Option<CallResult<Result<BlockIndex, WithdrawError>>>,
    }
    #[async_trait]
    impl WithdrawableLedgerCanister for TestCyclesLedgerCanister {
        async fn withdraw(
            &self,
            args: WithdrawArgs,
        ) -> CallResult<Result<BlockIndex, WithdrawError>> {
            let mut locked = self.transfer_called_with.write().await;
            locked.push(args);

            if let Some(value) = &self.returns_with {
                return value.clone();
            }

            Ok(Ok(0_u64.into()))
        }
    }
}
