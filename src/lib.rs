use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap};
use near_sdk::json_types::U128;
use near_sdk::{
    env, log, near_bindgen, require, AccountId, Balance, Gas, PromiseOrValue, Timestamp,
};
use std::borrow::Borrow;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
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
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct WhitelistState {
    restricted: bool,
    minting_start: Timestamp,
    minting_price: U128,
    minting_limit: u8,
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
    ) -> Self {
        log!("Custom counter initialization!");
        require!(
            private_sale_start < public_sale_start,
            "The private sale should start before the public sale"
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
        require!(self.whitelist.get(&account_id).is_none(), "Address already exist");

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

    /*
        TODO: Add update whitelist only admin allowed
     */

    /*
        TODO: Allow admin to withdraw collected funds out of the launchpad contract
     */

    pub fn get_whitelist(&self, from_index: u64, limit: u64) -> Vec<(AccountId, WhitelistState)> {
        let keys = self.whitelist.keys_as_vector();
        let values = self.whitelist.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, self.whitelist.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap()))
            .collect()
    }
}

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

        match msg.as_str() {
            "buy_ticket" => {
                // Verify if minting time have started otherwise refund
                match env::block_timestamp() {
                    time if time > self.public_sale_start => {
                        // Save the Sender to minted storage and increment the amount already minted
                        if self.minted.contains_key(&sender_id.clone().into()) {
                            let amount_minted = self.minted.get(&sender_id.clone().into()).unwrap();
                            self.minted
                                .insert(&sender_id.into(), &amount_minted.saturating_add(1));
                        } else {
                            self.minted.insert(&sender_id.into(), &1);
                        }
                        // TODO: Mint the NFT pack and send it to the sender

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
                            let amount_minted = self.minted.get(&sender_id.clone().into()).unwrap();
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

                        PromiseOrValue::Value(U128::from(0))
                    }
                    _ => {
                        log!("Sale have not started yet");
                        PromiseOrValue::Value(amount)
                    }
                }
            }
            _ => {
                log!("Invalid instruction for launchpad call");
                PromiseOrValue::Value(amount)
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
            .signer_account_id(AccountId::new_unchecked("bob_near".to_string()))
            .is_view(is_view)
            .build()
    }

    fn default_launchpad_init(admin: String) -> Launchpad {
        self::Launchpad::new(
            U128::from(100),
            AccountId::new_unchecked("usdc_near".to_string()),
            AccountId::new_unchecked("usdt_near".to_string()),
            AccountId::new_unchecked("dai_near".to_string()),
            100,
            200,
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
