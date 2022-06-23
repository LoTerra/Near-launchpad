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
    env, log, near_bindgen, require, serde_json, AccountId, Balance, Gas, PanicOnDefault, Promise,
    PromiseOrValue, Timestamp,
};

const CODE: &[u8] =
    include_bytes!("../../NFT/target/wasm32-unknown-unknown/release/non_fungible_token.wasm");
const DEFAULT_GAS: u64 = 100000000000000;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Launchpad {
    // Create whitelist storage key address => WhitelistState value
    whitelist: UnorderedMap<AccountId, WhitelistState>,
    minting_price: U128,
    admin: AccountId,
    usdc_account_id: AccountId,
    usdt_account_id: AccountId,
    dai_account_id: AccountId,
    private_sale_start: u64,
    public_sale_start: u64,
    switch_off: bool,
    // Create a storage key address => minted_amount value
    minted: LookupMap<AccountId, u64>,
    deposits: LookupMap<AccountId, U128>,
    nft_pack_contract: AccountId,
    // TODO: available mint and decrease on every mint
    nft_pack_supply: u16,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct WhitelistState {
    restricted: bool,
    minting_start: Timestamp,
    minting_price: U128,
    minting_limit: u8,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
enum TokenReceiverMessage {
    /// Reserve an NFT.
    Reserve { nft_amount: u64 },
}

#[near_bindgen]
impl Launchpad {
    #[init]
    pub fn new(
        minting_price: U128,
        usdc_account_id: AccountId,
        usdt_account_id: AccountId,
        dai_account_id: AccountId,
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
            AccountId::new_unchecked(format!("nft_pack8.{}", env::current_account_id()));
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
            .transfer(50_000_000_000_000_000_000_000_000)
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
            dai_account_id,
            private_sale_start,
            public_sale_start,
            switch_off: false,
            minted: LookupMap::new(b"m"),
            deposits: LookupMap::new(b"d"),
            nft_pack_contract: subaccount_id,
            nft_pack_supply,
        }
    }

    pub fn add_whitelist(
        &mut self,
        address: String,
        minting_start: Timestamp,
        minting_price: U128,
        minting_limit: u8,
    ) {
        require!(env::signer_account_id() == self.admin, "Owner's method");
        let account_id = AccountId::new_unchecked(address.clone());
        require!(
            self.whitelist.get(&account_id).is_none(),
            "Address already exist"
        );

        self.whitelist.insert(
            &account_id,
            &WhitelistState {
                restricted: false,
                minting_start,
                minting_price,
                minting_limit,
            },
        );

        log!(format!("Whitelist user {}", address));
    }

    #[payable]
    pub fn storage_deposit(&mut self, account: Option<AccountId>) {
        log!("Deposited {}YoctoNear", env::attached_deposit());
        let account_id = account.unwrap_or(env::signer_account_id());
        let deposit = U128::from(env::attached_deposit());

        if self.deposits.contains_key(&account_id.clone()) {
            let balance = self.deposits.get(&account_id.clone()).unwrap();
            self.deposits.insert(
                &account_id,
                &U128::from(deposit.0.saturating_add(balance.0)),
            );
            log!("Balance {}YoctoNear", deposit.0.saturating_add(balance.0));
        } else {
            self.deposits.insert(&account_id, &deposit);
        }
    }
    pub fn storage_withdraw_all(&mut self) -> Promise {
        let account = env::signer_account_id();
        require!(
            self.deposits.contains_key(&account.clone()),
            "No account found"
        );
        let balance = self.deposits.get(&account.clone()).unwrap();
        require!(balance > U128::from(0), "Empty balance");
        self.deposits.remove(&account.clone().into());
        log!(
            "Withdraw all storage ({}YoctoNear to {})",
            balance.0,
            account
        );

        Promise::new(account).transfer(balance.0)
    }

    /*
       TODO: Add update whitelist only admin allowed
    */

    /*
       TODO: Allow admin to withdraw collected funds out of the launchpad contract
    */

    /*
       TODO: Allow users to check their deposited amount
    */

    pub fn get_whitelist(&self, from_index: u64, limit: u64) -> Vec<(AccountId, WhitelistState)> {
        let keys = self.whitelist.keys_as_vector();
        let values = self.whitelist.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, self.whitelist.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap()))
            .collect()
    }

    /*
       TODO: Query get minting info
    */

    #[private]
    pub fn mint_result(&mut self) {
        //require!(env::promise_result() == 1);
        require!(env::promise_results_count() == 1);

        let balance = env::attached_deposit();

        log!("GOOD refunded amount: {:?}", balance);
        self.nft_pack_supply = self.nft_pack_supply.wrapping_sub(1);
    }
}
/*
   TODO: Allows multiple mint at same time with a loop
*/
#[near_bindgen]
impl FungibleTokenReceiver for Launchpad {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        require!(
            self.dai_account_id == env::predecessor_account_id()
                || self.usdc_account_id == env::predecessor_account_id()
                || self.usdt_account_id == env::predecessor_account_id(),
            "Only allowed NF contracts can call this message"
        );
        /*
           TODO: USDC & USDT are 6 decimals but DAI are 18 decimals. We need to do extra checks
        */

        // Verify the amount sent match with minting cost
        require!(
            amount == self.minting_price,
            format!(
                "Wrong amount sent, minting price {:?} DAI/USDC/USDT",
                self.minting_price
            )
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
        let token_id = self.nft_pack_supply.to_string();
        let receiver_id = sender_id.clone();

        if msg.is_empty() {
            log!("Missing msg in ft_transfer_call");
            PromiseOrValue::Value(amount)
        } else {
            let message = serde_json::from_str::<TokenReceiverMessage>(&msg)
                .expect("Illegal msg in ft_transfer_call");
            // Mint info end
            match message {
                TokenReceiverMessage::Reserve { nft_amount } => {
                    match env::block_timestamp() {
                        time if time > self.public_sale_start => {
                            // Save the Sender to minted storage and increment the amount already minted
                            if self.minted.contains_key(&sender_id.clone().into()) {
                                let amount_minted =
                                    self.minted.get(&sender_id.clone().into()).unwrap();
                                self.minted
                                    .insert(&sender_id.into(), &amount_minted.saturating_add(1));
                            } else {
                                self.minted.insert(&sender_id.into(), &1);
                            }

                            // TODO: Mint the NFT pack and send it to the sender
                            promise_mint_pack(
                                self.nft_pack_contract.clone(),
                                Gas::from(DEFAULT_GAS),
                                token_id,
                                receiver_id,
                                token_metadata,
                            );

                            // let promise0 = env::promise_create(
                            //     self.nft_pack_contract.clone(),
                            //     "nft_mint",
                            //     json!({
                            //         "token_id": token_id,
                            //         "receiver_id": receiver_id,
                            //         "token_metadata": token_metadata,
                            //         "refund_id": receiver_id
                            //     })
                            //     .to_string()
                            //     .as_bytes(),
                            //     U128::from(9870000000000000000000).0,
                            //     Gas::from(DEFAULT_GAS),
                            // );
                            //
                            // let promise1 = env::promise_then(
                            //     promise0,
                            //     env::current_account_id(),
                            //     "mint_result",
                            //     &[],
                            //     0,
                            //     Gas::from(DEFAULT_GAS),
                            // );
                            // env::promise_return(promise1);

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
                            require!(!whitelist_user.restricted, "Address have been restricted");
                            if self.minted.contains_key(&sender_id.clone().into()) {
                                let amount_minted =
                                    self.minted.get(&sender_id.clone().into()).unwrap();
                                require!(
                                    u64::from(whitelist_user.minting_limit) < amount_minted,
                                    "Minting limit reached"
                                );
                                self.minted
                                    .insert(&sender_id.into(), &amount_minted.saturating_add(1));
                            } else {
                                self.minted.insert(&sender_id.into(), &1);
                            }
                            // TODO: Mint the NFT pack and send it to the sender
                            promise_mint_pack(
                                self.nft_pack_contract.clone(),
                                Gas::from(DEFAULT_GAS),
                                token_id,
                                receiver_id,
                                token_metadata,
                            );
                            // let promise0 = env::promise_create(
                            //     self.nft_pack_contract.clone(),
                            //     "nft_mint",
                            //     json!({
                            //         "token_id": token_id,
                            //         "receiver_id": receiver_id,
                            //         "token_metadata": token_metadata
                            //     })
                            //     .to_string()
                            //     .as_bytes(),
                            //     0,
                            //     Gas::from(DEFAULT_GAS),
                            // );
                            // let promise1 = env::promise_then(
                            //     promise0,
                            //     env::current_account_id(),
                            //     "mint_result",
                            //     &[],
                            //     0,
                            //     Gas::from(DEFAULT_GAS),
                            // );
                            // env::promise_return(promise1);
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
        // match msg.as_str() {
        //     "reserve_mint" => {
        //         // Verify if minting time have started otherwise refund
        //         match env::block_timestamp() {
        //             time if time > self.public_sale_start => {
        //                 // Save the Sender to minted storage and increment the amount already minted
        //                 if self.minted.contains_key(&sender_id.clone().into()) {
        //                     let amount_minted = self.minted.get(&sender_id.clone().into()).unwrap();
        //                     self.minted
        //                         .insert(&sender_id.into(), &amount_minted.saturating_add(1));
        //                 } else {
        //                     self.minted.insert(&sender_id.into(), &1);
        //                 }
        //                 // TODO: Mint the NFT pack and send it to the sender
        //                 let promise0 = env::promise_create(
        //                     self.nft_pack_contract.clone(),
        //                     "nft_mint",
        //                     json!({
        //                         "token_id": token_id,
        //                         "receiver_id": receiver_id,
        //                         "token_metadata": token_metadata
        //                     })
        //                     .to_string()
        //                     .as_bytes(),
        //                     0,
        //                     Gas::from(DEFAULT_GAS),
        //                 );
        //                 let promise1 = env::promise_then(
        //                     promise0,
        //                     env::current_account_id(),
        //                     "mint_result",
        //                     &[],
        //                     0,
        //                     Gas::from(DEFAULT_GAS),
        //                 );
        //                 env::promise_return(promise1);
        //
        //                 PromiseOrValue::Value(U128::from(0))
        //             }
        //             time if time > self.private_sale_start => {
        //                 // Verify the sender is in the whitelist
        //                 require!(
        //                     self.whitelist.get(&sender_id).is_some(),
        //                     "The address is not in the whitelist"
        //                 );
        //                 // Verify the sender have not reached the minting limit
        //                 // Save the Sender to minted storage and increment the amount already minted
        //                 let whitelist_user = self.whitelist.get(&sender_id).unwrap();
        //                 require!(!whitelist_user.restricted, "Address have been restricted");
        //                 if self.minted.contains_key(&sender_id.clone().into()) {
        //                     let amount_minted = self.minted.get(&sender_id.clone().into()).unwrap();
        //                     require!(
        //                         u64::from(whitelist_user.minting_limit) < amount_minted,
        //                         "Minting limit reached"
        //                     );
        //                     self.minted
        //                         .insert(&sender_id.into(), &amount_minted.saturating_add(1));
        //                 } else {
        //                     self.minted.insert(&sender_id.into(), &1);
        //                 }
        //                 // TODO: Mint the NFT pack and send it to the sender
        //                 let promise0 = env::promise_create(
        //                     self.nft_pack_contract.clone(),
        //                     "nft_mint",
        //                     json!({
        //                         "token_id": token_id,
        //                         "receiver_id": receiver_id,
        //                         "token_metadata": token_metadata
        //                     })
        //                     .to_string()
        //                     .as_bytes(),
        //                     0,
        //                     Gas::from(DEFAULT_GAS),
        //                 );
        //                 let promise1 = env::promise_then(
        //                     promise0,
        //                     env::current_account_id(),
        //                     "mint_result",
        //                     &[],
        //                     0,
        //                     Gas::from(DEFAULT_GAS),
        //                 );
        //                 env::promise_return(promise1);
        //                 PromiseOrValue::Value(U128::from(0))
        //             }
        //             _ => {
        //                 log!("Sale have not started yet");
        //                 PromiseOrValue::Value(amount)
        //             }
        //         }
        //     }
        //     _ => {
        //         log!("Invalid instruction for launchpad call");
        //         PromiseOrValue::Value(amount)
        //     }
        // }
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
            .signer_account_id(AccountId::new_unchecked("bob_near".to_string()))
            .is_view(is_view)
            .build()
    }

    fn default_launchpad_init(admin: String) -> Promise {
        self::Launchpad::new(
            U128::from(100),
            AccountId::new_unchecked("usdc_near".to_string()),
            AccountId::new_unchecked("usdt_near".to_string()),
            AccountId::new_unchecked("dai_near".to_string()),
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

        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.add_whitelist(
            "alice_near".to_string(),
            env::block_timestamp().saturating_add(100),
            U128::from(1000),
            5,
        );
        println!("Ok: {:?}", contract.get_whitelist(0, 10));
    }

    #[test]
    #[should_panic]
    fn should_fail() {
        let context = get_context(false);
        testing_env!(context);
        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.add_whitelist(
            "alice_near".to_string(),
            env::block_timestamp().saturating_add(100),
            U128::from(10),
            5,
        );
    }

    // #[test]
    // fn buy() {
    //     let mut context = get_context(false);
    //     context.predecessor_account_id = AccountId::new_unchecked("usdc_near".to_string());
    //     testing_env!(context);
    //
    //     let mut contract = default_launchpad_init("admin_near".to_string());
    //     contract.callback_buy(AccountId::new_unchecked("usdc_near".to_string()), U128(100));
    //     //println!("Ok: {:?}", contract.get_whitelist());
    // }
}
