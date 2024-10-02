use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    Epoch,
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
    SyncQueryRunnerInterface,
};
use lightning_utils::transaction::{TransactionClient, TransactionClientError, TransactionSigner};

use super::{TestNode, TestNodeComponents};

impl TestNode {
    pub async fn app_client(
        &self,
        signer: TransactionSigner,
    ) -> TransactionClient<TestNodeComponents> {
        TransactionClient::<TestNodeComponents>::new(
            self.app_query.clone(),
            self.notifier.clone(),
            self.forwarder.mempool_socket(),
            signer,
        )
        .await
    }

    pub fn get_epoch(&self) -> Epoch {
        match self.app_query.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => unreachable!("invalid epoch in metadata"),
        }
    }

    pub fn get_protocol_fund_address(&self) -> EthAddress {
        match self.app_query.get_metadata(&Metadata::ProtocolFundAddress) {
            Some(Value::AccountPublicKey(s)) => s,
            None => unreachable!("missing protocol fund address in metadata"),
            _ => unreachable!("invalid protocol fund address in metadata"),
        }
    }

    pub fn get_total_supply(&self) -> HpUfixed<18> {
        match self.app_query.get_metadata(&Metadata::TotalSupply) {
            Some(Value::HpUfixed(s)) => s,
            None => panic!("missing total supply in metadata"),
            _ => unreachable!("invalid total supply in metadata"),
        }
    }

    pub fn get_supply_year_start(&self) -> HpUfixed<18> {
        match self.app_query.get_metadata(&Metadata::SupplyYearStart) {
            Some(Value::HpUfixed(s)) => s,
            None => panic!("missing supply year start in metadata"),
            _ => unreachable!("invalid supply year start in metadata"),
        }
    }

    pub fn get_stake(&self) -> HpUfixed<18> {
        self.app
            .sync_query()
            .get_node_info(&self.index(), |node| node.stake.staked)
            .ok_or(anyhow::anyhow!("own node not found"))
            .unwrap_or_default()
    }

    pub fn get_nonce(&self) -> u64 {
        self.app
            .sync_query()
            .get_node_info(&self.index(), |node| node.nonce)
            .ok_or(anyhow::anyhow!("own node not found"))
            .unwrap_or_default()
    }

    pub fn get_owner_nonce(&self) -> u64 {
        self.app_query
            .get_account_info(&self.owner_secret_key.to_pk().into(), |a| a.nonce)
            .unwrap_or_default()
    }

    pub fn get_account_nonce(&self, account: EthAddress) -> u64 {
        self.app_query
            .get_account_info(&account, |a| a.nonce)
            .unwrap_or_default()
    }

    pub fn get_stables_balance(&self, account: EthAddress) -> HpUfixed<6> {
        match self
            .app
            .sync_query()
            .get_account_info(&account, |a| a.stables_balance)
        {
            Some(balance) => balance,
            None => HpUfixed::<6>::zero(),
        }
    }

    pub fn get_flk_balance(&self, account: EthAddress) -> HpUfixed<18> {
        match self.app_query.get_account_info(&account, |a| a.flk_balance) {
            Some(balance) => balance,
            None => HpUfixed::<18>::zero(),
        }
    }

    pub async fn deposit_and_stake(
        &self,
        amount: HpUfixed<18>,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), TransactionClientError> {
        let client = self
            .app_client(TransactionSigner::AccountOwner(account.clone()))
            .await;
        // Deposit FLK tokens.
        client
            .execute_transaction(UpdateMethod::Deposit {
                proof: ProofOfConsensus {},
                token: Tokens::FLK,
                amount: amount.clone(),
            })
            .await?;

        // Stake FLK tokens.
        client
            .execute_transaction(UpdateMethod::Stake {
                amount: amount.clone(),
                node_public_key: self.keystore.get_ed25519_pk(),
                consensus_key: Some(self.keystore.get_bls_pk()),
                node_domain: None,
                worker_public_key: None,
                worker_domain: None,
                ports: None,
            })
            .await?;

        Ok(())
    }

    pub async fn stake_lock(
        &self,
        locked_for: u64,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), TransactionClientError> {
        let client = self
            .app_client(TransactionSigner::AccountOwner(account.clone()))
            .await;

        client
            .execute_transaction(UpdateMethod::StakeLock {
                node: self.keystore.get_ed25519_pk(),
                locked_for,
            })
            .await?;

        Ok(())
    }

    pub async fn unstake(
        &self,
        amount: HpUfixed<18>,
        account: &AccountOwnerSecretKey,
    ) -> Result<(), TransactionClientError> {
        let client = self
            .app_client(TransactionSigner::AccountOwner(account.clone()))
            .await;

        client
            .execute_transaction(UpdateMethod::Unstake {
                amount: amount.clone(),
                node: self.keystore.get_ed25519_pk(),
            })
            .await?;

        Ok(())
    }

    pub async fn execute_transaction_from_node(
        &self,
        method: UpdateMethod,
    ) -> Result<(TransactionRequest, TransactionReceipt), TransactionClientError> {
        let client = self.app_client(self.get_node_signer()).await;
        client.execute_transaction(method).await
    }

    pub async fn execute_transaction_from_owner(
        &self,
        method: UpdateMethod,
    ) -> Result<(TransactionRequest, TransactionReceipt), TransactionClientError> {
        let client = self.app_client(self.get_owner_signer()).await;
        client.execute_transaction(method).await
    }
}
