use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_application::state::QueryRunner;
use lightning_interfaces::types::{
    Epoch,
    ExecuteTransactionError,
    Metadata,
    ProofOfConsensus,
    Tokens,
    TransactionReceipt,
    TransactionRequest,
    UpdateMethod,
    Value,
};
use lightning_interfaces::{
    ApplicationInterface,
    ForwarderInterface,
    KeystoreInterface,
    NodeComponents,
    SyncQueryRunnerInterface,
};
use lightning_notifier::Notifier;
use lightning_utils::transaction::{TransactionClient, TransactionSigner};

use super::TestFullNode;
use crate::e2e::node::TestNetworkNode;

impl<C: NodeComponents> TestFullNode<C>
where
    C::ApplicationInterface: ApplicationInterface<C, SyncExecutor = QueryRunner>,
    C: NodeComponents<NotifierInterface = Notifier<C>>,
{
    pub async fn transaction_client(&self, signer: TransactionSigner) -> TransactionClient<C> {
        TransactionClient::<C>::new(
            self.app_query().clone(),
            self.notifier().clone(),
            self.forwarder().mempool_socket(),
            signer,
        )
        .await
    }

    pub fn get_epoch(&self) -> Epoch {
        match self.app_query().get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => unreachable!("invalid epoch in metadata"),
        }
    }

    pub fn get_stake(&self) -> HpUfixed<18> {
        self.app_query()
            .get_node_info(&self.index(), |node| node.stake.staked)
            .ok_or(anyhow::anyhow!("own node not found"))
            .unwrap_or_default()
    }

    pub fn get_nonce(&self) -> u64 {
        self.app_query()
            .get_node_info(&self.index(), |node| node.nonce)
            .ok_or(anyhow::anyhow!("own node not found"))
            .unwrap_or_default()
    }

    pub fn get_owner_nonce(&self) -> u64 {
        self.app_query()
            .get_account_info(&self.owner_secret_key.to_pk().into(), |a| a.nonce)
            .unwrap_or_default()
    }

    pub fn get_account_nonce(&self, account: EthAddress) -> u64 {
        self.app_query()
            .get_account_info(&account, |a| a.nonce)
            .unwrap_or_default()
    }

    pub fn get_stables_balance(&self, account: EthAddress) -> HpUfixed<6> {
        match self
            .app_query()
            .get_account_info(&account, |a| a.stables_balance)
        {
            Some(balance) => balance,
            None => HpUfixed::<6>::zero(),
        }
    }

    pub fn get_flk_balance(&self, account: EthAddress) -> HpUfixed<18> {
        match self
            .app_query()
            .get_account_info(&account, |a| a.flk_balance)
        {
            Some(balance) => balance,
            None => HpUfixed::<18>::zero(),
        }
    }

    pub async fn deposit_and_stake(
        &self,
        amount: HpUfixed<18>,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), ExecuteTransactionError> {
        let client = self
            .transaction_client(TransactionSigner::AccountOwner(account.clone()))
            .await;
        // Deposit FLK tokens.
        client
            .execute_transaction_and_wait_for_receipt(
                UpdateMethod::Deposit {
                    proof: ProofOfConsensus {},
                    token: Tokens::FLK,
                    amount: amount.clone(),
                },
                None,
            )
            .await?;

        // Stake FLK tokens.
        client
            .execute_transaction_and_wait_for_receipt(
                UpdateMethod::Stake {
                    amount: amount.clone(),
                    node_public_key: self.keystore().get_ed25519_pk(),
                    consensus_key: Some(self.keystore().get_bls_pk()),
                    node_domain: None,
                    worker_public_key: None,
                    worker_domain: None,
                    ports: None,
                },
                None,
            )
            .await?;

        Ok(())
    }

    pub async fn stake_lock(
        &self,
        locked_for: u64,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), ExecuteTransactionError> {
        let client = self
            .transaction_client(TransactionSigner::AccountOwner(account.clone()))
            .await;

        client
            .execute_transaction_and_wait_for_receipt(
                UpdateMethod::StakeLock {
                    node: self.keystore().get_ed25519_pk(),
                    locked_for,
                },
                None,
            )
            .await?;

        Ok(())
    }

    pub async fn unstake(
        &self,
        amount: HpUfixed<18>,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), ExecuteTransactionError> {
        let client = self
            .transaction_client(TransactionSigner::AccountOwner(account.clone()))
            .await;

        client
            .execute_transaction_and_wait_for_receipt(
                UpdateMethod::Unstake {
                    amount: amount.clone(),
                    node: self.keystore().get_ed25519_pk(),
                },
                None,
            )
            .await?;

        Ok(())
    }

    pub async fn execute_transaction_from_owner(
        &self,
        method: UpdateMethod,
    ) -> Result<(TransactionRequest, TransactionReceipt), ExecuteTransactionError> {
        let client = self.transaction_client(self.get_owner_signer()).await;
        let resp = client
            .execute_transaction_and_wait_for_receipt(method, None)
            .await?;

        Ok(resp.as_receipt())
    }
}
