use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::{AccountId, env, log, near_bindgen, require, Timestamp};
use near_sdk::json_types::U128;
use near_contract_standards;


#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct Launchpad {
    // See more data types at https://doc.rust-lang.org/book/ch03-02-data-types.html
    whitelist: Vec<WhitelistState>,
    minting_price: u128,
    admin: AccountId,
    usdc_account_id: AccountId,
    usdt_account_id: AccountId,
    dai_account_id: AccountId,
    private_sale_start: u64,
    public_sale_start: u64
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
    pub fn new(minting_price: u128, usdc_account_id: AccountId, usdt_account_id: AccountId, dai_account_id: AccountId, private_sale_start: u64, public_sale_start: u64) -> Self {
        log!("Custom counter initialization!");
        require!(private_sale_start < public_sale_start, "The private sale should start before the public sale");
        Self { whitelist: vec![], minting_price, admin: env::signer_account_id(), usdc_account_id, usdt_account_id, dai_account_id, private_sale_start , public_sale_start }
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

    pub fn callback_buy(&self, sender: AccountId, amount: U128) {
        // Authorized only by supported FT contract
        require!(self.dai_account_id == env::predecessor_account_id() || self.usdc_account_id == env::predecessor_account_id() || self.usdt_account_id == env::predecessor_account_id(), "Only allowed NF contracts can call this message");
        // Verify the amount sent match with minting cost
        require!(amount == self.minting_price, format!("Wrong amount sent, minting price {} DAI/USDC/USDT", self.minting_price));

        // TODO: Verify if minting time have started
        match env::block_timestamp() {
            Some(time) if self.public_sale_start > time => {},
            Some(time) if self.private_sale_start > time => {}
            _ => panic!()
        }

        if self.private_sale_start < env::block_timestamp() {

        }

        /*
            TODO : If private sale started verify if sender is into whitelist
         */
        // TODO verify minting limit



        // Finally
        // TODO mint an NFT pack and send it to the sender
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

    #[test]
    fn buy() {
        let mut context = get_context(false);
        context.predecessor_account_id = AccountId::new_unchecked("usdc_near".to_string());
        testing_env!(context);

        let mut contract = default_launchpad_init("admin_near".to_string());
        contract.buy();
        //println!("Ok: {:?}", contract.get_whitelist());
    }
}