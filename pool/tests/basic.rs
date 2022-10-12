use near_sdk::{serde_json::json};
use workspaces::{Account, Contract};
mod utils;
use crate::utils::to_yocto;

const DEFAULT_GAS: u64 = 300_000_000_000_000;
const DRAW_BYTES: &[u8] = include_bytes!("../../res/draw.wasm");
const TEST_TOKEN_BYTES: &[u8] = include_bytes!("../../res/fungible_token.wasm");
const TOKEN_SYMBOL: &str = "USDC";
const TOKEN_DESCRIPTION: &str = "USD Coin on the blockchain";

async fn deploy_and_init_token(owner: &Account) -> anyhow::Result<Contract>{
    let token_acc = create_account(owner, "token").await?;
    let token_contract = token_acc.deploy(&TEST_TOKEN_BYTES).await?.unwrap();
    
    token_contract
        .call("new")
        .args_json(json!({
            "owner_id": token_acc.id(), 
            "total_supply": "1000000000000000", 
            "metadata": 
                { 
                    "spec": "ft-1.0.0", 
                    "name": TOKEN_DESCRIPTION, 
                    "symbol": TOKEN_SYMBOL, 
                    "decimals": 8 
                }
            }))
        .gas(DEFAULT_GAS)
        .transact()
        .await?
        .into_result()?;
    
    return Ok(token_contract);
}

async fn create_account(owner: &Account, acc_name: &str) -> anyhow::Result<Account>{
    let acc = owner
        .create_subaccount( acc_name)
        .initial_balance(to_yocto("10"))
        .transact()
        .await?
        .unwrap();

    return Ok(acc);
}

#[tokio::test]
async fn test() -> anyhow::Result<()>{
    let workspaces = workspaces::sandbox().await?;
    let x = workspaces.root_account().unwrap();
    let ft_contract = deploy_and_init_token(&workspaces.root_account().unwrap()).await?;
    println!("Mario");
    return Ok(());
}