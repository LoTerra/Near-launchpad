use near_contract_standards::non_fungible_token::metadata::TokenMetadata;
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::{env, require, AccountId, Gas};

const MINT_STORAGE_COST: u128 = 5870000000000000000000;
const DEFAULT_GAS: u64 = 5_000_000_000_000;
pub(crate) fn promise_mint_pack(
    nft_pack_contract: AccountId,
    token_id: String,
    receiver_id: AccountId,
    token_metadata: TokenMetadata,
    mint_limit: u16,
    current_account: AccountId,
    storage_deposit: U128,
) -> u128 {
    let storage_mint = u128::from(mint_limit) * MINT_STORAGE_COST;
    require!(
        storage_deposit.0 >= storage_mint,
        format!(
            "Required minimum storage deposit of {} Yocto Near",
            storage_mint
        )
    );

    let promise_id = env::promise_batch_create(&nft_pack_contract);
    let mut n = 0;
    while n < mint_limit {
        if mint_limit - n == 1 {
            env::promise_batch_action_function_call(
                promise_id,
                "nft_mint",
                json!({
                "token_id": token_id,
                "receiver_id": receiver_id.clone(),
                "token_metadata": token_metadata,
                    "refund_id": Some(receiver_id.clone())
            })
                    .to_string()
                    .as_bytes(),
                storage_mint,
                Gas::from(DEFAULT_GAS),
            );
        }else {
            env::promise_batch_action_function_call(
                promise_id,
                "nft_mint",
                json!({
                "token_id": token_id,
                "receiver_id": receiver_id,
                "token_metadata": token_metadata
            })
                    .to_string()
                    .as_bytes(),
                storage_mint,
                Gas::from(DEFAULT_GAS),
            );
        }

        n += 1;
    }

    let callback_promise_id = env::promise_batch_then(
        promise_id,       // postpone until a DataReceipt associated with promise_id is received
        &current_account, // the recipient of this ActionReceipt (&self)
    );

    env::promise_batch_action_function_call(
        callback_promise_id,    // associate the function call with callback_promise_id
        "mint_result",          // the function call will be a callback function
        &[],                    // method arguments
        0,                      // amount of yoctoNEAR to attach
        Gas::from(DEFAULT_GAS), // gas to attach
    );

    // // TODO: Mint the NFT pack and send it to the sender
    // let promise0 = env::promise_create(
    //     nft_pack_contract,
    //     "nft_mint",
    //     json!({
    //         "token_id": token_id,
    //         "receiver_id": receiver_id,
    //         "token_metadata": token_metadata
    //     })
    //     .to_string()
    //     .as_bytes(),
    //     U128::from(MINT_STORAGE_COST).0,
    //     default_gas,
    // );
    // let promise1 = env::promise_then(
    //     promise0,
    //     env::current_account_id(),
    //     "mint_result",
    //     &[],
    //     0,
    //     default_gas,
    // );
    env::promise_return(callback_promise_id);
    storage_mint
    //env::promise_return(promise1)
}
