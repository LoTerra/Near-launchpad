use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::UnorderedMap;
use near_sdk::json_types::U128;
use near_sdk::{env, log, near_bindgen, require, AccountId, Timestamp, PromiseOrValue, Balance};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Launchpad {
    // See more data types at https://doc.rust-lang.org/book/ch03-02-data-types.html
    whitelist: UnorderedMap<AccountId, WhitelistState>,
    minting_price: U128,
    admin: AccountId,
    usdc_account_id: AccountId,
    usdt_account_id: AccountId,
    dai_account_id: AccountId,
    private_sale_start: u64,
    public_sale_start: u64,
    switch_off: bool
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct WhitelistState {
    // See more data types at https://doc.rust-lang.org/book/ch03-02-data-types.html
    restricted: bool,
    minting_start: Timestamp,
    minting_price: U128,
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
            switch_off: false
        }
    }

    pub fn add_whitelist(&mut self, address: String, minting_start: Timestamp, minting_price: U128) {
        require!(env::signer_account_id() == self.admin, "Owner's method");

        // https://doc.rust-lang.org/std/primitive.i8.html#method.wrapping_add
        self.whitelist.insert(&AccountId::new_unchecked(address.clone()), &WhitelistState {
            restricted: false,
            minting_start,
            minting_price
        });

        log!(format!("Whitelist user {}", address));
    }

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
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
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

        match msg.as_str() {
            "buy-pack" => PromiseOrValue::Value(U128::from(0)),
            _ => {
                let prepaid_gas = env::prepaid_gas();
                let account_id = env::current_account_id();
                ext_self::value_please(
                    msg,
                    account_id,
                    NO_DEPOSIT,
                    prepaid_gas - GAS_FOR_FT_ON_TRANSFER,
                )
                    .into()
            }
        }

        // TODO: Verify if minting time have started
        // match env::block_timestamp() {
        //     Some(time) if self.public_sale_start > time => {}
        //     Some(time) if self.private_sale_start > time => {}
        //     _ => panic!(),
        // }

        //if self.private_sale_start < env::block_timestamp() {}

        /*
           TODO : If private sale started verify if sender is into whitelist
        */
        // TODO verify minting limit

        // Finally
        // TODO mint an NFT pack and send it to the sender
    }
}

#[near_bindgen]
impl ValueReturnTrait for Launchpad {
    fn value_please(&self, amount_to_return: String) -> PromiseOrValue<U128> {
        log!("in value_please, amount_to_return = {}", amount_to_return);
        let amount: Balance = amount_to_return.parse().expect("Not an integer");
        PromiseOrValue::Value(amount.into())
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
        contract.add_whitelist("alice_near".to_string(), env::block_timestamp().saturating_add(100), U128::from(1000));
        println!("Ok: {:?}", contract.get_whitelist(0, 10));
    }

    #[test]
    #[should_panic]
    fn should_fail() {
        let context = get_context(false);
        testing_env!(context);
        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.add_whitelist("alice_near".to_string(), env::block_timestamp().saturating_add(100), U128::from(10));
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
