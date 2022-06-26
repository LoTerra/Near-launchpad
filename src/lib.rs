mod helpers;

use crate::helpers::promise_mint_pack;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_contract_standards::non_fungible_token::metadata::{NFTContractMetadata, TokenMetadata};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::serde_json::json;
use near_sdk::{
    assert_one_yocto, env, log, near_bindgen, require, serde_json, AccountId, Gas, PanicOnDefault,
    Promise, PromiseOrValue, PromiseResult, Timestamp,
};

const CODE: &[u8] =
    include_bytes!("../../NFT/target/wasm32-unknown-unknown/release/non_fungible_token.wasm");
const DEFAULT_GAS: u64 = 5_000_000_000_000;
const MINT_STORAGE_COST: u128 = 5870000000000000000000;
/*
   IMPORTANT: Reduce amount for mainnet
*/
const MIN_DEPOSIT_CREATING_ACCOUNT: u128 = 5_000_000_000_000_000_000_000_000;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Minter {
    whitelist: UnorderedMap<AccountId, WhitelistState>, // Create whitelist storage key address => WhitelistState value
    minting_price: U128,
    admin: AccountId,
    usdc_account_id: AccountId,
    usdt_account_id: AccountId,
    // dai_account_id: AccountId,
    private_sale_start: u64,
    public_sale_start: u64,
    switch_off: bool,
    minted: LookupMap<AccountId, u16>, // Create a storage key address => minted_amount value
    storage_deposits: LookupMap<AccountId, U128>,
    nft_pack_contract: AccountId,
    nft_pack_supply: u16, // Available mint and decrease on every mint
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, PartialEq)]
#[serde(crate = "near_sdk::serde")]
pub struct WhitelistState {
    minting_start: Timestamp,
    minting_price: U128,
    minting_limit: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
enum TokenReceiverMessage {
    /// Mint an NFT, amount will be used to verify the deposit.
    Mint { mint_amount: u16 },
}

#[near_bindgen]
impl Minter {
    /// Instantiate the contract
    #[init]
    pub fn new(
        minting_price: U128,
        usdc_account_id: AccountId,
        usdt_account_id: AccountId,
        //dai_account_id: AccountId,
        private_sale_start: u64,
        public_sale_start: u64,
        nft_pack_supply: u16,
    ) -> Self {
        log!(format!("creator: {}", env::signer_account_id()));
        require!(
            private_sale_start < public_sale_start,
            "The private sale should start before the public sale"
        );
        /*
            Allows our contract to deploy the NFT pack contract as admin more info for
            dev help https://www.near-sdk.io/promises/deploy-contract
        */
        let subaccount_id =
            AccountId::new_unchecked(format!("nft_pack9.{}", env::current_account_id()));
        let current_accout = env::current_account_id();

        let metadata = NFTContractMetadata {
            spec: "nft-1.0.0".to_string(),
            name: "test".to_string(),
            symbol: "PACK".to_string(),
            icon: None,
            base_uri: None,
            reference: None,
            reference_hash: None,
        };

        Promise::new(subaccount_id.clone())
            .create_account()
            .transfer(MIN_DEPOSIT_CREATING_ACCOUNT)
            .add_full_access_key(env::signer_account_pk())
            .deploy_contract(CODE.to_vec())
            .function_call(
                "new".to_string(),
                json!({
                    "owner_id":  current_accout,
                    "metadata": metadata
                })
                .to_string()
                .as_bytes()
                .to_vec(),
                0,
                Gas::from(DEFAULT_GAS),
            );

        Self {
            whitelist: UnorderedMap::new(b"s"),
            minting_price,
            admin: env::signer_account_id(),
            usdc_account_id,
            usdt_account_id,
            // dai_account_id,
            private_sale_start,
            public_sale_start,
            // Not used should we use a switch off??
            switch_off: false,
            minted: LookupMap::new(b"m"),
            storage_deposits: LookupMap::new(b"d"),
            nft_pack_contract: subaccount_id,
            nft_pack_supply,
        }
    }

    /// Admin add account id to whitelist
    pub fn add_whitelist(
        &mut self,
        account_id: AccountId,
        minting_start: Timestamp,
        minting_price: U128,
        minting_limit: u8,
    ) {
        require!(env::signer_account_id() == self.admin, "Owner's method");
        require!(
            env::is_valid_account_id(&account_id.as_bytes()),
            "Not a valid account id"
        );
        require!(
            self.whitelist.get(&account_id.clone().into()).is_none(),
            "Account already exist"
        );

        self.whitelist.insert(
            &account_id.clone().into(),
            &WhitelistState {
                minting_start,
                minting_price,
                minting_limit,
            },
        );

        log!(format!("Whitelist account {}", account_id));
    }

    /// Admin delete account from whitelist
    pub fn delete_whitelist(&mut self, account_id: AccountId) {
        require!(env::signer_account_id() == self.admin, "Owner's method");
        require!(
            env::is_valid_account_id(&account_id.as_bytes()),
            "Not a valid account id"
        );
        require!(
            self.whitelist.get(&account_id.clone().into()).is_some(),
            "Account not found"
        );

        self.whitelist.remove(&account_id.clone().into());
        log!(format!("Delete whitelist account {}", account_id));
    }

    /// Near deposit storage, used as fee for minting NFT
    #[payable]
    pub fn storage_deposit(&mut self, account: Option<AccountId>) {
        require!(
            env::attached_deposit() >= MINT_STORAGE_COST,
            format!("Requires minimum deposit of {}", MINT_STORAGE_COST)
        );
        log!("Deposited {}YoctoNear", env::attached_deposit());
        let account_id = account.unwrap_or(env::signer_account_id());
        let deposit = U128::from(env::attached_deposit());

        if self
            .storage_deposits
            .contains_key(&account_id.clone().into())
        {
            let balance = self
                .storage_deposits
                .get(&account_id.clone().into())
                .unwrap_or(U128::from(0));
            self.storage_deposits.insert(
                &account_id.clone().into(),
                &U128::from(deposit.0.checked_add(balance.0).unwrap()),
            );

            log!(
                "Balance {}YoctoNear",
                deposit.0.checked_add(balance.0).unwrap()
            );
        } else {
            self.storage_deposits.insert(&account_id.into(), &deposit);
        }
    }

    /// Withdraw all storage deposited
    #[payable]
    pub fn storage_withdraw_all(&mut self) -> Promise {
        assert_one_yocto();
        let account = env::signer_account_id();
        require!(
            self.storage_deposits.contains_key(&account.clone().into()),
            "No account found"
        );
        let balance = self
            .storage_deposits
            .get(&account.clone().into())
            .unwrap_or(U128::from(0));
        require!(balance > U128::from(0), "Empty balance");
        self.storage_deposits.remove(&account.clone().into());
        log!(
            "Withdraw all storage ({}YoctoNear to {})",
            balance.0,
            account
        );

        Promise::new(account).transfer(balance.0)
    }

    /*
       Allow admin to withdraw collected funds out of the Minter contract
    */
    /// Admin can withdraw collected funds
    pub fn admin_collect(self, from: AccountId, amount: U128) -> Promise {
        let signer_account_id = env::signer_account_id();
        require!(signer_account_id == self.admin, "Owner's method");

        Promise::new(from).function_call(
            "ft_transfer".to_string(),
            json!({
            "receiver_id": signer_account_id,
            "amount": amount
            })
            .to_string()
            .as_bytes()
            .to_vec(),
            0,
            Gas::from(DEFAULT_GAS),
        )
    }

    /// Queries
    /// Get storage balance from account id
    // Allow users to check their deposited amount
    pub fn get_storage_balance_of(self, account: AccountId) -> U128 {
        require!(
            self.storage_deposits.contains_key(&account.clone().into()),
            "No account found"
        );

        self.storage_deposits
            .get(&account.clone().into())
            .unwrap_or(U128::from(0))
    }

    /// Query get whitelist by pagination from index + limit
    pub fn get_whitelist(&self, from_index: u64, limit: u64) -> Vec<(AccountId, WhitelistState)> {
        let keys = self.whitelist.keys_as_vector();
        let values = self.whitelist.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, self.whitelist.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap()))
            .collect()
    }

    /// Get minting info from account id
    pub fn get_minting_of(self, account: AccountId) -> u16 {
        require!(
            self.minted.contains_key(&account.clone().into()),
            "No account found"
        );

        self.minted.get(&account.clone().into()).unwrap_or_default()
    }

    #[private]
    pub fn mint_result(
        &mut self,
        reduce_from_supply: u16,
        // _receiver_id: AccountId,
        // _from: AccountId,
        // _refund_amount: U128,
    ) {
        require!(env::promise_results_count() == 1);
        match env::promise_result(0) {
            PromiseResult::NotReady => unreachable!(),
            PromiseResult::Successful(_) => {
                log!(format!("Successfully minted {} pack", reduce_from_supply));
                self.nft_pack_supply = self
                    .nft_pack_supply
                    .checked_sub(reduce_from_supply)
                    .unwrap();
            }
            PromiseResult::Failed => {
                /*
                   Highlighting we probably don't need to refund
                   FT contract is already waiting a PromiseOrValue with a refund if fail
                */
                // log!("Refund because failed to mint NFT pack");
                // // Probably we need to refund the user here
                // Promise::new(from).function_call(
                //     "ft_transfer_call".to_string(),
                //     json!({
                //         "receiver_id": receiver_id,
                //         "amount": refund_amount
                //     })
                //     .to_string()
                //     .as_bytes()
                //     .to_vec(),
                //     0,
                //     Gas::from(DEFAULT_GAS),
                // );

                // OR panic
                env::panic_str("Minting failed")
            }
        }
    }
}

#[near_bindgen]
impl FungibleTokenReceiver for Minter {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        require!(
            // self.dai_account_id == env::predecessor_account_id()
            //     ||
            self.usdc_account_id == env::predecessor_account_id()
                || self.usdt_account_id == env::predecessor_account_id(),
            "Only allowed NF contracts can call this message"
        );

        log!(
            "in {} tokens from @{} ft_on_transfer, msg = {}",
            amount.0,
            sender_id.as_ref(),
            msg
        );

        // Mint info start
        let token_metadata = TokenMetadata {
            title: None,
            description: None,
            media: None,
            media_hash: None,
            copies: None,
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        };

        let receiver_id = sender_id.clone();

        if msg.is_empty() {
            log!("Missing msg in ft_transfer_call");
            PromiseOrValue::Value(amount)
        } else {
            let message = serde_json::from_str::<TokenReceiverMessage>(&msg)
                .expect("Illegal msg in ft_transfer_call");
            // Mint info end
            match message {
                TokenReceiverMessage::Mint { mint_amount } => {
                    require!(mint_amount > 0);
                    /*
                       INFO: USDC & USDT are 6 decimals but DAI are 18 decimals.
                    */
                    // Verify the amount sent match with minting cost
                    require!(
                        amount == U128::from(self.minting_price.0 * u128::from(mint_amount)),
                        format!(
                            "Wrong amount sent, minting price {:?} DAI/USDC/USDT",
                            self.minting_price.0 * u128::from(mint_amount)
                        )
                    );

                    require!(
                        self.nft_pack_supply >= mint_amount,
                        format!(
                            "Supply limit reached. Left {} NFT pack",
                            self.nft_pack_supply
                        )
                    );
                    let storage_deposit = self.storage_deposits.get(&sender_id.clone());
                    require!(
                        storage_deposit.is_some(),
                        "Action required deposit Near for storage"
                    );

                    match env::block_timestamp() {
                        time if time > self.public_sale_start => {
                            // Save the Sender to minted storage and increment the amount already minted
                            if self.minted.contains_key(&sender_id.clone().into()) {
                                let amount_minted =
                                    self.minted.get(&sender_id.clone().into()).unwrap();
                                self.minted.insert(
                                    &sender_id.clone().into(),
                                    &amount_minted.checked_add(mint_amount).unwrap(),
                                );
                            } else {
                                self.minted.insert(&sender_id.clone().into(), &mint_amount);
                            }

                            // Mint the NFT pack and send it to the sender
                            let used_storage_deposit = promise_mint_pack(
                                self.nft_pack_contract.clone(),
                                self.nft_pack_supply,
                                receiver_id,
                                token_metadata,
                                mint_amount,
                                env::current_account_id(),
                                storage_deposit.unwrap_or(U128::from(0)),
                                // amount,
                            );

                            self.storage_deposits.insert(
                                &sender_id.clone().into(),
                                &U128::from(
                                    storage_deposit
                                        .unwrap_or(U128::from(0))
                                        .0
                                        .checked_sub(used_storage_deposit)
                                        .unwrap(),
                                ),
                            );

                            PromiseOrValue::Value(U128::from(0))
                        }
                        time if time > self.private_sale_start => {
                            // Verify the sender is in the whitelist
                            require!(
                                self.whitelist.get(&sender_id).is_some(),
                                "The address is not in the whitelist"
                            );
                            // Verify the sender have not reached the minting limit
                            // Save the Sender to minted storage and increment the amount already minted
                            let whitelist_user = self.whitelist.get(&sender_id).unwrap();
                            if self.minted.contains_key(&sender_id.clone().into()) {
                                let amount_minted =
                                    self.minted.get(&sender_id.clone().into()).unwrap();
                                require!(
                                    u16::from(whitelist_user.minting_limit)
                                        <= amount_minted.checked_add(mint_amount).unwrap(),
                                    "Out of mint"
                                );
                                self.minted.insert(
                                    &sender_id.clone().into(),
                                    &amount_minted.checked_add(mint_amount).unwrap(),
                                );
                            } else {
                                require!(
                                    u16::from(whitelist_user.minting_limit) <= mint_amount,
                                    format!(
                                        "Whitelisted account only allowed to mint {} NFTs pack",
                                        whitelist_user.minting_limit
                                    )
                                );
                                self.minted.insert(&sender_id.clone().into(), &mint_amount);
                            }
                            // Mint the NFT pack and send it to the sender
                            let used_storage_deposit = promise_mint_pack(
                                self.nft_pack_contract.clone(),
                                self.nft_pack_supply,
                                receiver_id,
                                token_metadata,
                                mint_amount,
                                env::current_account_id(),
                                storage_deposit.unwrap_or(U128::from(0)),
                                // amount,
                            );
                            self.storage_deposits.insert(
                                &sender_id.clone().into(),
                                &U128::from(
                                    storage_deposit
                                        .unwrap_or(U128::from(0))
                                        .0
                                        .checked_sub(used_storage_deposit)
                                        .unwrap(),
                                ),
                            );

                            PromiseOrValue::Value(U128::from(0))
                        }
                        _ => {
                            log!("Sale have not started yet");
                            PromiseOrValue::Value(amount)
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{testing_env, AccountId, VMContext};

    fn get_context(is_view: bool) -> VMContext {
        VMContextBuilder::new()
            .signer_account_id(AccountId::new_unchecked("admin_near".to_string()))
            .is_view(is_view)
            .build()
    }

    fn default_minter_init() -> Minter {
        self::Minter::new(
            U128::from(100),
            AccountId::new_unchecked("usdc_near".to_string()),
            AccountId::new_unchecked("usdt_near".to_string()),
            //AccountId::new_unchecked("dai_near".to_string()),
            100,
            200,
            5000,
        )
    }

    #[test]
    fn try_whitelist() {
        let mut context = get_context(false);
        context.signer_account_id = AccountId::new_unchecked("admin_near".to_string());
        testing_env!(context);

        let mut contract = default_minter_init();
        contract.add_whitelist(
            AccountId::new_unchecked("alice_near".to_string()),
            env::block_timestamp().checked_add(100).unwrap(),
            U128::from(1000),
            5,
        );

        assert_eq!(
            contract.get_whitelist(0, 10),
            vec![(
                AccountId::new_unchecked("alice_near".to_string()),
                WhitelistState {
                    minting_start: 100,
                    minting_price: U128::from(1000),
                    minting_limit: 5
                }
            )]
        );
    }

    #[test]
    #[should_panic]
    fn try_whitelist_not_authorized() {
        let mut context = get_context(false);
        let mut contract = default_minter_init();
        context.signer_account_id = AccountId::new_unchecked("alice_near".to_string());
        testing_env!(context);

        contract.add_whitelist(
            AccountId::new_unchecked("alice_near".to_string()),
            env::block_timestamp().checked_add(100).unwrap(),
            U128::from(10),
            5,
        )
    }
}
