use anyhow::Ok;
use near_contract_standards::storage_management::StorageManagement;
use near_sdk::{serde_json::{json, self}, json_types::U128, Balance, serde::{Serialize, Deserialize}};
use workspaces::{Account, Contract, AccountId, Worker, network::Sandbox};
mod utils;
use crate::utils::to_yocto;

const DEFAULT_GAS: u64 = 300_000_000_000_000;
const DRAW_BYTES: &[u8] = include_bytes!("../../res/draw.wasm");
const TEST_TOKEN_BYTES: &[u8] = include_bytes!("../../res/fungible_token.wasm");
const POOL_BYTES: &[u8] = include_bytes!("../../res/pool.wasm");
const DEFI_BYTES: &[u8] = include_bytes!("../../res/defi.wasm");
const TOKEN_SYMBOL: &str = "USDC";
const TOKEN_DESCRIPTION: &str = "USD Coin on the blockchain";
const FT_TOKEN_DECIMALS: u32 = 0;
const FT_TOKEN_TOTAL_SUPPLY: u128 = 1000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenAmountsView{
    token: near_sdk::AccountId,
    shares: U128,
    rewards: U128
}

fn to_token_amount(amount: u64) -> u128{
    (amount as u128) * 10u128.pow(FT_TOKEN_DECIMALS)
}

async fn storage_deposit(caller: &Account, ft_contract: &AccountId) -> anyhow::Result<()>{
    caller.call(ft_contract, "storage_deposit")
        .args_json(json!({}))
        .gas(DEFAULT_GAS)
        .deposit(to_yocto("1"))
        .transact()
        .await?
        .unwrap();
    
    return Ok(());
}

async fn send_near_to_contract_for_future_ft_transfers(caller: &Account, pool: &AccountId, yocto: Balance) -> anyhow::Result<()>{
    caller.call(pool, "accept_deposit_for_future_fungible_token_transfers")
        .args_json(json!({}))
        .gas(DEFAULT_GAS)
        .deposit(yocto)
        .transact()
        .await?
        .unwrap();

    return Ok(());
}

async fn ft_transfer(sender: &Account, receiver: &AccountId, amount: Balance, ft_token: &AccountId) -> anyhow::Result<()>{
    sender.call(ft_token, "ft_transfer")
        .args_json(json!({"receiver_id": receiver, "amount": amount.to_string()}))
        .gas(DEFAULT_GAS)
        .deposit(1)
        .transact()
        .await?
        .into_result()?;

    return Ok(());
}

async fn ft_transfer_call(sender: &Account, receiver: &AccountId, amount: Balance, ft_token: &AccountId, msg: &str) -> anyhow::Result<()>{
    let res = sender.call(ft_token, "ft_transfer_call")
        .args_json((receiver, amount.to_string(), String::from(""), msg))
        .max_gas()
        .deposit(1)
        .transact()
        .await?
        .into_result()?;

    println!("{:?} \n", res);

    return Ok(());
}

async fn ft_balance_of(caller: &Account, contract: &AccountId) -> anyhow::Result<u128>{
    let balance = caller.call(contract, "ft_balance_of")
    .args_json((caller.id(),))
    .max_gas()
    .view()
    .await?
    .json::<U128>()?;

    return Ok(balance.0);
}

async fn deploy_and_init_defi(owner: &Account) -> anyhow::Result<Contract>{
    let defi_acc = create_account(owner, "defi").await?;
    let defi_contract = defi_acc.deploy(&DEFI_BYTES).await?.unwrap();
    
    defi_contract
        .call("new")
        .gas(DEFAULT_GAS)
        .transact()
        .await?
        .into_result()?;

    return Ok(defi_contract);
}

async fn deploy_and_init_pool(owner: &Account, token: &AccountId, draw: &AccountId, burrow: &AccountId) -> anyhow::Result<Contract>{
    let pool_acc = create_account(owner, "pool").await?;
    let pool_contract = pool_acc.deploy(&POOL_BYTES).await?.unwrap();
    
    let res = pool_contract
        .call("new_default_meta")
        .args_json(json!({"owner_id": pool_acc.id(), "token_for_deposit": token, "draw_contract": draw, "burrow_address": burrow, "reward_token": token}))
        .gas(DEFAULT_GAS)
        .transact()
        .await?
        .into_result()?;

    return Ok(pool_contract);
}

async fn deploy_and_init_draw(owner: &Account) -> anyhow::Result<Contract>{
    let draw_acc = create_account(owner, "draw").await?;
    let draw_contract = draw_acc.deploy(&DRAW_BYTES).await?.unwrap();

    draw_contract
        .call("new")
        .gas(DEFAULT_GAS)
        .transact()
        .await?
        .into_result()?;

    return Ok(draw_contract);
}

async fn deploy_and_init_token(owner: &Account) -> anyhow::Result<Contract>{
    let token_acc = create_account(owner, "token").await?;
    let token_contract = token_acc.deploy(&TEST_TOKEN_BYTES).await?.unwrap();
    
    token_contract
        .call("new")
        .args_json(json!({
            "owner_id": token_acc.id(), 
            "total_supply": (FT_TOKEN_TOTAL_SUPPLY * 10u128.pow(FT_TOKEN_DECIMALS)).to_string(), 
            "metadata": 
                { 
                    "spec": "ft-1.0.0", 
                    "name": TOKEN_DESCRIPTION, 
                    "symbol": TOKEN_SYMBOL, 
                    "decimals": FT_TOKEN_DECIMALS 
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

async fn setup() -> anyhow::Result<(Contract, Contract, Contract, Contract, Account)>{
    let workspaces = workspaces::sandbox().await?;
    let root = workspaces.root_account().unwrap();
    let ft_contract = deploy_and_init_token(&root).await?;
    let draw_contract = deploy_and_init_draw(&root).await?;
    let defi = deploy_and_init_defi(&root).await?;
    let pool_contract = deploy_and_init_pool(&root, ft_contract.id(), draw_contract.id(), defi.id()).await?;

    return Ok((pool_contract, draw_contract, ft_contract, defi, root));
}

#[tokio::test]
async fn test_simple_transfer() -> anyhow::Result<()>{
    let (pool, _, ft, defi, root) = setup().await?;
    
    let test1 = create_account(&root, "test1").await?;

    storage_deposit(&test1, ft.id()).await?;

    let balance = ft.call("ft_balance_of")
        .args_json(json!({"account_id": ft.as_account().id()}))
        .view()
        .await?
        .json::<U128>()
        .unwrap();

        println!("{}", balance.0);

    ft_transfer(ft.as_account(), test1.id(), to_token_amount(10), ft.id()).await?;

    // ft.call("ft_transfer")
    //     .args_json(json!({"receiver_id": test1.id(), "amount": to_token_amount(10).to_string()}))
    //     .gas(DEFAULT_GAS)
    //     .deposit(1)
    //     .transact()
    //     .await?
    //     .into_result()?;

    let balance = ft.call("ft_balance_of")
        .args_json(json!({"account_id": ft.as_account().id()}))
        .view()
        .await?
        .json::<U128>()
        .unwrap();

    println!("{}", balance.0);

    let balance = ft.call("ft_balance_of")
        .args_json(json!({"account_id": test1.id()}))
        .view()
        .await?
        .json::<U128>()
        .unwrap();

        println!("{}", balance.0);
     
    return Ok(());
}

#[tokio::test]
async fn test_sending_not_authorized_token() -> anyhow::Result<()>{
    let (pool, _, ft, defi, root) = setup().await.unwrap();

    let token_acc = create_account(&root, "another-token").await.unwrap();
    let token_contract = token_acc.deploy(&TEST_TOKEN_BYTES).await.unwrap().unwrap();

    let test1 = create_account(&root, "test1").await.unwrap();
    
    token_contract
        .call("new")
        .args_json(json!({
            "owner_id": token_acc.id(), 
            "total_supply": (FT_TOKEN_TOTAL_SUPPLY * 10u128.pow(FT_TOKEN_DECIMALS)).to_string(), 
            "metadata": 
                { 
                    "spec": "ft-1.0.0", 
                    "name": "Another token description", 
                    "symbol": "ANOT", 
                    "decimals": FT_TOKEN_DECIMALS 
                }
            }))
        .gas(DEFAULT_GAS)
        .transact()
        .await
        .unwrap()
        .into_result()
        .unwrap();

    storage_deposit(&test1, &token_contract.id()).await?;
    storage_deposit(&test1, &ft.id()).await?;

    storage_deposit(pool.as_account(), &token_contract.id()).await?;
    storage_deposit(pool.as_account(), &ft.id()).await?;

    ft_transfer(token_contract.as_account(), test1.id(), to_token_amount(3), token_contract.id()).await?;
    ft_transfer_call(&test1, pool.id(), to_token_amount(2), token_contract.id(), "").await?;

    let test1_balance = ft_balance_of(&test1, pool.id()).await?;
    assert_eq!(test1_balance, 0);

    return Ok(());
}

#[tokio::test]
async fn test_sending_correct_token() -> anyhow::Result<()>{
    let (pool, _, ft, defi, root) = setup().await.unwrap();

    let test1 = create_account(&root, "test1").await.unwrap();

    storage_deposit(&test1, &ft.id()).await?;
    storage_deposit(pool.as_account(), &ft.id()).await?;
    send_near_to_contract_for_future_ft_transfers(&test1, pool.id(), 2).await?;

    ft_transfer(ft.as_account(), test1.id(), to_token_amount(3), ft.id()).await?;
    ft_transfer_call(&test1, pool.id(), to_token_amount(2), ft.id(), "").await?;

    let test1_balance = ft_balance_of(&test1, pool.id()).await?;
    assert_eq!(test1_balance, to_token_amount(2));
    let pool_balance = ft_balance_of(pool.as_account(), pool.id()).await?;
    assert_eq!(pool_balance, 0);

    return Ok(());
}

#[tokio::test]
async fn test_defi_send_token() -> anyhow::Result<()>{
    let (_, _, ft, defi, root) = setup().await?;
    let test1 = create_account(&root, "test1").await?;  

    let token_acc = create_account(&root, "another-token").await.unwrap();
    let token_contract = token_acc.deploy(&TEST_TOKEN_BYTES).await.unwrap().unwrap();

    token_contract
        .call("new")
        .args_json(json!({
            "owner_id": token_acc.id(), 
            "total_supply": (FT_TOKEN_TOTAL_SUPPLY * 10u128.pow(FT_TOKEN_DECIMALS)).to_string(), 
            "metadata": 
                { 
                    "spec": "ft-1.0.0", 
                    "name": "Another token description", 
                    "symbol": "ANOT", 
                    "decimals": FT_TOKEN_DECIMALS 
                }
            }))
        .gas(DEFAULT_GAS)
        .transact()
        .await
        .unwrap()
        .into_result()
        .unwrap();

    storage_deposit(&test1, ft.id()).await?;
    storage_deposit(defi.as_account(), ft.id()).await?;

    ft_transfer(ft.as_account(), test1.id(), to_token_amount(100), ft.id()).await?;
    ft_transfer_call(&test1, defi.id(), to_token_amount(10), ft.id(), "").await?;
    ft_transfer_call(&test1, defi.id(), to_token_amount(3), ft.id(), test1.id()).await?;

    let res = test1.call(defi.id(), "show_reward")
        .args_json((test1.id(), ))
        .max_gas()
        .view()
        .await?
        .json::<Vec<TokenAmountsView>>()?;

    println!("{:?}", res);

    let token_amounts = res.iter().find(|el| el.token.to_string() == ft.id().to_string()).unwrap();
    assert_eq!(token_amounts.rewards.0, to_token_amount(3));
    assert_eq!(token_amounts.shares.0, to_token_amount(10));

    return Ok(());
}

#[tokio::test]
async fn test_sending_correct_token_check_defi() -> anyhow::Result<()>{
    let (pool, _, ft, defi, root) = setup().await.unwrap();

    let test1 = create_account(&root, "test1").await.unwrap();

    storage_deposit(&test1, &ft.id()).await?;
    storage_deposit(pool.as_account(), &ft.id()).await?;
    storage_deposit(defi.as_account(), ft.id()).await?;

    /// Test1 users has 3 FT tokens
    ft_transfer(ft.as_account(), test1.id(), to_token_amount(3), ft.id()).await?;
    let test1_ft_balance = ft_balance_of(&test1, ft.id()).await?;
    assert_eq!(test1_ft_balance, to_token_amount(3));
    send_near_to_contract_for_future_ft_transfers(&test1, pool.id(), 100).await?;

    /// Test1 user transfer 2 FT tokens to pool contract
    ft_transfer_call(&test1, pool.id(), to_token_amount(2), ft.id(), "").await?;
    let test1_ft_balance = ft_balance_of(&test1, ft.id()).await?;
    assert_eq!(test1_ft_balance, to_token_amount(1));
    
    // Pool contract should not have any balance in the ft contract
    let pool_balance_of_ft = ft_balance_of(&pool.as_account(), &ft.id()).await?;
    assert_eq!(0, pool_balance_of_ft);
    let defi_balance_of_ft = ft_balance_of(&defi.as_account(), &ft.id()).await?;
    assert_eq!(defi_balance_of_ft, to_token_amount(2));

    let test1_tickets = ft_balance_of(&test1, pool.id()).await?;
    assert_eq!(test1_tickets, to_token_amount(2));
    let pool_balance = ft_balance_of(pool.as_account(), pool.id()).await?;
    assert_eq!(pool_balance, 0);

    // set rewards
    ft_transfer_call(ft.as_account(), defi.id(), to_token_amount(5), ft.id(), pool.id()).await?;
    let res = pool.as_account().call(defi.id(), "show_reward")
        .args_json((pool.id(), ))
        .max_gas()
        .view()
        .await?
        .json::<Vec<TokenAmountsView>>()?;

    println!("{:?}", res);

    return Ok(());
}