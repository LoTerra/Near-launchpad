use crate::serde_json::Value;
use near_contract_standards::non_fungible_token::metadata::TokenMetadata;
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::{env, require, AccountId, Gas};

const MINT_STORAGE_COST: u128 = 5870000000000000000000;
const DEFAULT_GAS: u64 = 5_000_000_000_000;
pub(crate) fn promise_mint_pack(
    nft_pack_contract: AccountId,
    nft_pack_supply: u16,
    receiver_id: AccountId,
    token_metadata: TokenMetadata,
    mint_limit: u16,
    current_account: AccountId,
    storage_deposit: U128,
    // amount_cost: U128,
) -> u128 {
    let storage_mint = u128::from(mint_limit) * MINT_STORAGE_COST;
    println!("{}, {}", storage_deposit.0, storage_mint);
    require!(
        storage_deposit.0 >= storage_mint,
        format!(
            "Minimum required for storage deposit of {} Yocto Near",
            storage_mint
        )
    );

    let promise_id = env::promise_batch_create(&nft_pack_contract);

    let mut n = 0;
    while n < mint_limit {
        let token_id = nft_pack_supply
            .checked_sub(n.checked_add(1).unwrap())
            .unwrap()
            .to_string();
        let mut arguments = json!({
            "token_id": token_id,
            "receiver_id": receiver_id,
            "token_metadata": token_metadata
        });

        if mint_limit - n == 1 {
            arguments["refund_id"] = Value::String(receiver_id.clone().to_string());
        }

        env::promise_batch_action_function_call(
            promise_id,
            "nft_mint",
            arguments.to_string().as_bytes(),
            storage_mint,
            Gas::from(DEFAULT_GAS),
        );

        n += 1;
    }

    let callback_promise_id = env::promise_batch_then(
        promise_id,       // postpone until a DataReceipt associated with promise_id is received
        &current_account, // the recipient of this ActionReceipt (&self)
    );
    /*
       Just for info if refund not work just add on argument and uncomment the mint_result params
       message &json!({ "reduce_from_supply": mint_limit, "receiver_id": receiver_id, "from":
       env::predecessor_account_id(), "refund_amount": amount_cost })
    */
    env::promise_batch_action_function_call(
        callback_promise_id, // associate the function call with callback_promise_id
        "mint_result",       // the function call will be a callback function
        &json!({ "reduce_from_supply": mint_limit })
            .to_string()
            .as_bytes(), // method arguments
        0,                   // amount of yoctoNEAR to attach
        Gas::from(DEFAULT_GAS), // gas to attach
    );

    env::promise_return(callback_promise_id);
    storage_mint
}
