use near_contract_standards::non_fungible_token::metadata::TokenMetadata;
use near_sdk::serde_json::json;
use near_sdk::{env, AccountId, Balance, Gas, Promise};

const MINT_STORAGE_COST: u128 = 5870000000000000000000;
pub(crate) fn promise_mint_pack(
    nft_pack_contract: AccountId,
    default_gas: Gas,
    token_id: String,
    receiver_id: AccountId,
    token_metadata: TokenMetadata,
) {
    // TODO: Mint the NFT pack and send it to the sender
    let promise0 = env::promise_create(
        nft_pack_contract,
        "nft_mint",
        json!({
            "token_id": token_id,
            "receiver_id": receiver_id,
            "token_metadata": token_metadata
        })
        .to_string()
        .as_bytes(),
        0,
        default_gas,
    );
    let promise1 = env::promise_then(
        promise0,
        env::current_account_id(),
        "mint_result",
        &[],
        0,
        default_gas,
    );

    env::promise_return(promise1)
}
