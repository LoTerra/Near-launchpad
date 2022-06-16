use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{AccountId, env, log, near_bindgen, require, Timestamp};
use near_sdk::json_types::U128;


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Launchpad {
    // See more data types at https://doc.rust-lang.org/book/ch03-02-data-types.html
    whitelist: Vec<WhitelistState>,
    minting_price: u128,
    admin: AccountId,
    usdc_account_id: AccountId,
    usdt_account_id: AccountId,
    dai_account_id: AccountId
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct WhitelistState {
    // See more data types at https://doc.rust-lang.org/book/ch03-02-data-types.html
    account_id: String,
    restricted: bool,
    minting_start: Timestamp,
    minting_price: U128
}


#[near_bindgen]
impl Launchpad {

    #[init]
    pub fn new(minting_price: u128, usdc_account_id: AccountId, usdt_account_id: AccountId, dai_account_id: AccountId) -> Self {
        log!("Custom counter initialization!");
        Self { whitelist: vec![], minting_price, admin: env::signer_account_id(), usdc_account_id, usdt_account_id, dai_account_id}
    }

    pub fn add_whitelist(&mut self, minting_start: Timestamp, minting_price: U128) {
        require!(env::signer_account_id() == self.admin, "Owner's method");

        // https://doc.rust-lang.org/std/primitive.i8.html#method.wrapping_add
        self.whitelist.push( WhitelistState{
            account_id: env::signer_account_id().to_string(),
            restricted: false,
            minting_start,
            minting_price
        });

        log!(format!("Whitelist user {}", env::signer_account_id()).as_str());

    }

    pub fn get_whitelist(&self) -> &Vec<WhitelistState> {
        let data = &self.whitelist;
        return data;
    }


}


#[cfg(test)]
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::{AccountId, testing_env, VMContext};
    use std::convert::TryInto;

    fn get_context(is_view: bool) -> VMContext {
        VMContextBuilder::new()
            .signer_account_id(AccountId::new_unchecked("bob_near".to_string()))
            .is_view(is_view)
            .build()
    }

    fn default_launchpad_init(admin: String) -> Launchpad{
        Launchpad{ whitelist: vec![], minting_price: 100, admin: AccountId::new_unchecked(admin), usdc_account_id: AccountId::new_unchecked("usdc_near".to_string()), usdt_account_id: AccountId::new_unchecked("usdt_near".to_string()), dai_account_id: AccountId::new_unchecked("dai_near".to_string()) }
    }

    #[test]
    fn my_test() {
        let mut context = get_context(false);
        context.signer_account_id = AccountId::new_unchecked("admin_near".to_string());
        testing_env!(context);

        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.add_whitelist(env::block_timestamp().saturating_add(100), U128::from(1000));
        println!("Ok: {:?}", contract.get_whitelist());
    }

    #[test]
    #[should_panic]
    fn should_fail(){
        let context = get_context(false);
        testing_env!(context);
        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.add_whitelist(env::block_timestamp().saturating_add(100), U128::from(10));
    }
}