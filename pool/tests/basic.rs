use anyhow::Ok;
use common::types::{NumPicks, WinningNumber, DrawId};
use near_contract_standards::storage_management::StorageManagement;
use near_sdk::{serde_json::{json, self}, json_types::U128, Balance, serde::{Serialize, Deserialize}};
use workspaces::{Account, Contract, AccountId, Worker, network::Sandbox};
mod utils;
use crate::utils::{to_yocto, create_masks, get_user_winning_number, get_tier_match, prize_tier_fraction};

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

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
    #[serde(crate = "near_sdk::serde")]
    pub struct PrizeDistribution{
        pub number_of_picks: u64,
        pub draw_id: DrawId,
        pub cardinality: u8,
        pub bit_range_size: u8,
        pub tiers: [u32; 16],
        pub prize: u128,
        pub max_picks: NumPicks,
        pub start_time: u64,
        pub end_time: u64,
        pub winning_number: WinningNumber,
    }

fn to_token_amount(amount: u64) -> u128{
    (amount as u128) * 10u128.pow(FT_TOKEN_DECIMALS)
}

fn most_profitable_pick(caller: &near_sdk::AccountId, cardinality: u8, bit_range_size: u8, winning_number: WinningNumber, user_max_picks: NumPicks, tiers: &[u32]) -> NumPicks{
    let masks = create_masks(bit_range_size, cardinality);
    let mut min_tier_match = u8::MAX;
    let mut winning_pick:u128 = 0;
    for x in 1..user_max_picks + 1{
        let user_winning_number = get_user_winning_number(&caller, x);
        let tier_match = get_tier_match(&masks, &user_winning_number, &winning_number);
        if tier_match < min_tier_match{
            println!("{} tier {} pick {}", caller.as_str(), tier_match, x);
            let fraction = prize_tier_fraction(tier_match, 1, tiers);
            println!("{:?}", fraction);
            winning_pick = x;
            min_tier_match = tier_match;
        }
    }

    return winning_pick;
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

async fn can_complete_draw(draw: &Account) -> anyhow::Result<bool>{
    let res = draw
        .call(draw.id(), "can_complete_draw")
        .view()
        .await?
        .json::<bool>()?;

    return Ok(res);
}

async fn complete_draw(draw: &Account) -> anyhow::Result<()>{
     draw
        .call(draw.id(), "complete_draw")
        .transact()
        .await?
        .into_result()?;

    return Ok(());
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

async fn deploy_and_init_pool(owner: &Account, token: &AccountId, draw: &AccountId, burrow: &AccountId, min_pick_cost: Option<Balance>) -> anyhow::Result<Contract>{
    let pool_acc = create_account(owner, "pool").await?;
    let pool_contract = pool_acc.deploy(&POOL_BYTES).await?.unwrap();
    let pick_cost = min_pick_cost.unwrap_or(10000000000000000000000);

    let res = pool_contract
        .call("new_default_meta")
        .args_json(json!({"owner_id": pool_acc.id(), "token_for_deposit": token, "draw_contract": draw, "burrow_address": burrow, "reward_token": token, "min_pick_cost": U128(pick_cost)}))
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
    let pool_contract = deploy_and_init_pool(&root, ft_contract.id(), draw_contract.id(), defi.id(), None).await?;

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

#[tokio::test]
async fn test_add_prize_distribution() -> anyhow::Result<()>{
    let workspaces = workspaces::sandbox().await?;
    let root = workspaces.root_account().unwrap();
    let ft_contract = deploy_and_init_token(&root).await?;
    let draw_contract = deploy_and_init_draw(&root).await?;
    let defi = deploy_and_init_defi(&root).await?;
    let pool_contract = deploy_and_init_pool(&root, ft_contract.id(), draw_contract.id(), defi.id(), None).await?;

    let res = draw_contract.call("start_draw")
    .args_json(json!({}))
    .max_gas()
    .transact()
    .await?
    .into_result();

    workspaces.fast_forward(10000).await?;
    let can_complete_draw = draw_contract.call("can_complete_draw")
        .args_json(json!({}))
        .max_gas()
        .view()
        .await?
        .json::<bool>()?;

    assert_eq!(can_complete_draw, true);
    draw_contract.call("complete_draw")
        .args_json(json!({}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let res = pool_contract.as_account().call(pool_contract.id(), "add_prize_distribution")
        .args_json(json!({"draw_id": 1, "prize_awards": "1000000", "cardinality": 8, "bit_range_size": 4}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    res.logs().iter().for_each(|el| {
        println!("{}", *el);
    });

    pool_contract.view("get_prize_distribution", json!({"draw_id": 1}).to_string().into_bytes())
        .await?;

    return Ok(());
}


#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct AssetAmount {
    pub token_id: near_sdk::AccountId,
    /// The amount of tokens intended to be used for the action
    /// If 'None', then the maximum will be tried
    pub amount: Option<U128>
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub enum Action{
    Withdraw(AssetAmount)
}

#[tokio::test]
async fn test_defi_reward_generator() -> anyhow::Result<()>{
    let workspaces = workspaces::sandbox().await?;
    let root = workspaces.root_account().unwrap();
    let token = deploy_and_init_token(&root).await?;
    let draw = deploy_and_init_draw(&root).await?;
    let defi = deploy_and_init_defi(&root).await?;
    let pool = deploy_and_init_pool(&root, token.id(), draw.id(), defi.id(), Some(1)).await?;

    // create account
    let test1 = create_account(&root, "test1").await.unwrap();
    // storage deposit for account
    storage_deposit(&test1, token.id()).await?;
    // transfer to test account tokens
    ft_transfer(token.as_account(), test1.id(), to_token_amount(15), token.id()).await?;
    assert_eq!(to_token_amount(15), ft_balance_of(&test1, token.id()).await?);

    storage_deposit(pool.as_account(), token.id()).await?;
    storage_deposit(defi.as_account(), token.id()).await?;
    
    // send from test account to pool some near tokens for future ft_transfer calls
    test1
        .call(pool.id(), "accept_deposit_for_future_fungible_token_transfers")
        .deposit(10)
        .transact()
        .await?
        .into_result()?;

    // send 5 tokens to pool, those tokens should be sent to defi
    ft_transfer_call(&test1, pool.id(), to_token_amount(5), token.id(), "").await?;
    assert_eq!(to_token_amount(10), ft_balance_of(&test1, token.id()).await?);

    // defi should have 5 FT tokens
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, to_token_amount(5));
    // test1 account should have 5 tickets in the pool
    assert_eq!(ft_balance_of(&test1, pool.id()).await?, to_token_amount(5));

    // set reward to mocked defi
    ft_transfer_call(token.as_account(), defi.id(), to_token_amount(3), token.id(), pool.id()).await?;

    let generated_reward = test1
        .call(pool.id(), "get_reward")
        .max_gas()
        .transact()
        .await?
        .json::<U128>()?.0;

    assert_eq!(generated_reward, to_token_amount(3));
    let asset_amount = AssetAmount{ token_id: near_sdk::AccountId::new_unchecked(token.id().to_string()), amount: Some(U128(generated_reward))};
    let action = Action::Withdraw(
        asset_amount
    );
    let old_pool_amount = ft_balance_of(pool.as_account(), token.id()).await?;
    let exec_res = pool.as_account().call(defi.id(), "execute")
        .args_json(json!({"actions": vec![action]}))
        .max_gas()
        .deposit(1)
        .transact()
        .await?
        .into_result()?;

    exec_res.logs().iter().for_each(|l| println!("{}", l));

    let new_pool_amount = ft_balance_of(pool.as_account(), token.id()).await?;

    assert!(new_pool_amount > old_pool_amount);

    return Ok(());
}

#[tokio::test]
async fn test_claim() -> anyhow::Result<()>{
    let workspaces = workspaces::sandbox().await?;
    let root = workspaces.root_account().unwrap();
    let token = deploy_and_init_token(&root).await?;
    let draw = deploy_and_init_draw(&root).await?;
    let defi = deploy_and_init_defi(&root).await?;
    let pool = deploy_and_init_pool(&root, token.id(), draw.id(), defi.id(), Some(1)).await?;
    let initial_balance = to_token_amount(15);
    let deposit_to_pool = to_token_amount(5);

    // draw can be started
    let can_start_draw = draw.as_account()
        .call(draw.id(), "can_start_draw")
        .view()
        .await?
        .json::<bool>()?;

    assert_eq!(can_start_draw, true);

    draw.as_account()
        .call(draw.id(), "start_draw")
        .transact()
        .await?
        .into_result()?;

    // create account
    let test1 = create_account(&root, "test1").await.unwrap();
    // storage deposit for account
    storage_deposit(&test1, token.id()).await?;
    // transfer to test account tokens
    ft_transfer(token.as_account(), test1.id(), initial_balance, token.id()).await?;
    assert_eq!(initial_balance, ft_balance_of(&test1, token.id()).await?);

    storage_deposit(pool.as_account(), token.id()).await?;
    storage_deposit(defi.as_account(), token.id()).await?;
    
    // send from test account to pool some near tokens for future ft_transfer calls
    test1
        .call(pool.id(), "accept_deposit_for_future_fungible_token_transfers")
        .deposit(10)
        .transact()
        .await?
        .into_result()?;

    // send 5 tokens to pool, those tokens should be sent to defi
    ft_transfer_call(&test1, pool.id(), deposit_to_pool, token.id(), "").await?;
    assert_eq!(initial_balance - deposit_to_pool, ft_balance_of(&test1, token.id()).await?);

    // defi should have 5 FT tokens
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, deposit_to_pool);
    // test1 account should have 5 tickets in the pool
    assert_eq!(ft_balance_of(&test1, pool.id()).await?, deposit_to_pool);

    let reward = to_token_amount(100);
    // set reward to mocked defi
    ft_transfer_call(token.as_account(), defi.id(), reward, token.id(), pool.id()).await?;
    let defi_balance = ft_balance_of(defi.as_account(), token.id()).await?;
    assert_eq!(defi_balance, deposit_to_pool + reward);

    let generated_reward = test1
        .call(pool.id(), "get_reward")
        .max_gas()
        .transact()
        .await?
        .json::<U128>()?.0;

    assert_eq!(generated_reward, reward);

    let mut can_complete = can_complete_draw(draw.as_account()).await?;

    while !can_complete{
        workspaces.fast_forward(1000).await?;
        can_complete = can_complete_draw(draw.as_account()).await?;
    }

    complete_draw(draw.as_account()).await?;
    
    pool.as_account().call(pool.id(), "add_prize_distribution")
        .args_json(json!({"draw_id": 1, "prize_awards": U128(generated_reward), "cardinality": 4, "bit_range_size": 1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let prize_distribution = pool
        .view("get_prize_distribution", json!({"draw_id": 1}).to_string().into_bytes())
        .await?
        .json::<PrizeDistribution>()?;

    println!("{:?}", prize_distribution);

    // find most profitable pick
    let picks = test1.call(pool.id(), "get_picks")
        .args_json(json!({"draw_id": 1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json::<NumPicks>()?;

    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(picks + 1)}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .expect_err("Invalid pick");

    let winning_pick = most_profitable_pick(&near_sdk::AccountId::new_unchecked(test1.id().to_string()), prize_distribution.cardinality, prize_distribution.bit_range_size, prize_distribution.winning_number, picks, &prize_distribution.tiers);

    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(winning_pick)}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let balance = ft_balance_of(&test1, token.id()).await?;
    println!("Balance is {}", balance);
    assert!(balance > initial_balance - deposit_to_pool);

    // expect err
    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(winning_pick)}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .expect_err("Pick already claimed");

    // defi balance should be equal to reward + initial deposit minus the reward claimed by test1 user
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, defi_balance - (balance - (initial_balance - deposit_to_pool)));

    return Ok(());
}

#[tokio::test]
async fn test_claim_multiple_users() -> anyhow::Result<()>{
    let workspaces = workspaces::sandbox().await?;
    let root = workspaces.root_account().unwrap();
    let token = deploy_and_init_token(&root).await?;
    let draw = deploy_and_init_draw(&root).await?;
    let defi = deploy_and_init_defi(&root).await?;
    let pool = deploy_and_init_pool(&root, token.id(), draw.id(), defi.id(), Some(1)).await?;

    let test_1_initial_balance = to_token_amount(15);
    let test_1_deposit_to_pool = to_token_amount(5);

    let test_2_initial_balance = to_token_amount(50);
    let test_2_deposit_to_pool = to_token_amount(10);

    let test_3_initial_balance = to_token_amount(30);
    let test_3_deposit_to_pool = to_token_amount(5);

    // create accounts
    let test1 = create_account(&root, "test1").await.unwrap();
    let test2 = create_account(&root, "test2").await.unwrap();
    let test3 = create_account(&root, "test3").await.unwrap();

    // storage deposit for accounts
    storage_deposit(&test1, token.id()).await?;
    storage_deposit(&test2, token.id()).await?;
    storage_deposit(&test3, token.id()).await?;

    // transfer to test account tokens
    ft_transfer(token.as_account(), test1.id(), test_1_initial_balance, token.id()).await?;
    assert_eq!(test_1_initial_balance, ft_balance_of(&test1, token.id()).await?);

    ft_transfer(token.as_account(), test2.id(), test_2_initial_balance, token.id()).await?;
    assert_eq!(test_2_initial_balance, ft_balance_of(&test2, token.id()).await?);

    ft_transfer(token.as_account(), test3.id(), test_3_initial_balance, token.id()).await?;
    assert_eq!(test_3_initial_balance, ft_balance_of(&test3, token.id()).await?);

    storage_deposit(pool.as_account(), token.id()).await?;
    storage_deposit(defi.as_account(), token.id()).await?;
    
    // send from test account to pool some near tokens for future ft_transfer calls
    test1
        .call(pool.id(), "accept_deposit_for_future_fungible_token_transfers")
        .deposit(10)
        .transact()
        .await?
        .into_result()?;

    test2
        .call(pool.id(), "accept_deposit_for_future_fungible_token_transfers")
        .deposit(10)
        .transact()
        .await?
        .into_result()?;

    test3
        .call(pool.id(), "accept_deposit_for_future_fungible_token_transfers")
        .deposit(10)
        .transact()
        .await?
        .into_result()?;

    // send 5 tokens to pool, those tokens should be sent to defi
    ft_transfer_call(&test1, pool.id(), test_1_deposit_to_pool, token.id(), "").await?;
    assert_eq!(test_1_initial_balance - test_1_deposit_to_pool, ft_balance_of(&test1, token.id()).await?);
    // defi should have 5 FT tokens
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, test_1_deposit_to_pool);
    // test1 account should have 5 tickets in the pool
    assert_eq!(ft_balance_of(&test1, pool.id()).await?, test_1_deposit_to_pool);

    // send tokens to pool, those tokens should be sent to defi
    ft_transfer_call(&test2, pool.id(), test_2_deposit_to_pool, token.id(), "").await?;
    assert_eq!(test_2_initial_balance - test_2_deposit_to_pool, ft_balance_of(&test2, token.id()).await?);
    // defi should have test_1_deposit + test_2_deposit FT tokens
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, test_1_deposit_to_pool + test_2_deposit_to_pool);
    // test1 account should have test_2_deposit amount tickets in the pool
    assert_eq!(ft_balance_of(&test2, pool.id()).await?, test_2_deposit_to_pool);

    // send tokens to pool, those tokens should be sent to defi
    ft_transfer_call(&test3, pool.id(), test_3_deposit_to_pool, token.id(), "").await?;
    assert_eq!(test_3_initial_balance - test_3_deposit_to_pool, ft_balance_of(&test3, token.id()).await?);
    // defi should have test_1_deposit + test_2_deposit FT tokens
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, test_1_deposit_to_pool + test_2_deposit_to_pool + test_3_deposit_to_pool);
    // test1 account should have test_2_deposit amount tickets in the pool
    assert_eq!(ft_balance_of(&test3, pool.id()).await?, test_3_deposit_to_pool);

    // draw can be started
    let can_start_draw = draw.as_account()
        .call(draw.id(), "can_start_draw")
        .view()
        .await?
        .json::<bool>()?;

    assert_eq!(can_start_draw, true);

    draw.as_account()
        .call(draw.id(), "start_draw")
        .transact()
        .await?
        .into_result()?;

    let reward = to_token_amount(100);
    // set reward to mocked defi
    ft_transfer_call(token.as_account(), defi.id(), reward, token.id(), pool.id()).await?;
    let defi_balance = ft_balance_of(defi.as_account(), token.id()).await?;
    assert_eq!(defi_balance, test_1_deposit_to_pool + test_2_deposit_to_pool + test_3_deposit_to_pool + reward);

    let generated_reward = test1
        .call(pool.id(), "get_reward")
        .max_gas()
        .transact()
        .await?
        .json::<U128>()?.0;

    assert_eq!(generated_reward, reward);

    let mut can_complete = can_complete_draw(draw.as_account()).await?;

    while !can_complete{
        workspaces.fast_forward(1000).await?;
        can_complete = can_complete_draw(draw.as_account()).await?;
    }

    complete_draw(draw.as_account()).await?;
    
    let cardinality:u8 = 4;
    let bit_range_size:u8 = 1;

    pool.as_account().call(pool.id(), "add_prize_distribution")
        .args_json(json!({"draw_id": 1, "prize_awards": U128(generated_reward), "cardinality": cardinality, "bit_range_size": bit_range_size}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let prize_distribution = pool
        .view("get_prize_distribution", json!({"draw_id": 1}).to_string().into_bytes())
        .await?
        .json::<PrizeDistribution>()?;

    println!("{:?}", prize_distribution);

    // find most profitable pick
    let test_1_picks = test1.call(pool.id(), "get_picks")
        .args_json(json!({"draw_id": 1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json::<NumPicks>()?;

    let test_2_picks = test2.call(pool.id(), "get_picks")
        .args_json(json!({"draw_id": 1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json::<NumPicks>()?;

    let test_3_picks = test3.call(pool.id(), "get_picks")
        .args_json(json!({"draw_id": 1}))
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json::<NumPicks>()?;

    println!("{} {} {}", test_1_picks, test_2_picks, test_3_picks);

    // relation between deposits should be equal to the number of picks
    // test1_deposit    test1_picks
    // -------------  = -----------     
    // test2_deposit    test2_picks

    assert_eq!(test_1_deposit_to_pool * test_2_picks, test_2_deposit_to_pool * test_1_picks);

    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(test_1_picks + 1)}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .expect_err("Invalid pick");

    let winning_pick_test_1 = most_profitable_pick(&near_sdk::AccountId::new_unchecked(test1.id().to_string()), prize_distribution.cardinality, prize_distribution.bit_range_size, prize_distribution.winning_number, test_1_picks, &prize_distribution.tiers);
    let winning_pick_test_2 = most_profitable_pick(&near_sdk::AccountId::new_unchecked(test2.id().to_string()), prize_distribution.cardinality, prize_distribution.bit_range_size, prize_distribution.winning_number, test_2_picks, &prize_distribution.tiers);
    let winning_pick_test_3 = most_profitable_pick(&near_sdk::AccountId::new_unchecked(test3.id().to_string()), prize_distribution.cardinality, prize_distribution.bit_range_size, prize_distribution.winning_number, test_3_picks, &prize_distribution.tiers);

    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(winning_pick_test_1)}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let test_1_balance = ft_balance_of(&test1, token.id()).await?;
    println!("{} Balance is {}", test1.id(), test_1_balance);
    assert!(test_1_balance > test_1_initial_balance - test_1_deposit_to_pool);

    test2.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(winning_pick_test_2)}))
        .max_gas()
        .transact()
        .await?
        .into_result()?;

    let test_2_balance = ft_balance_of(&test2, token.id()).await?;
    println!("{} Balance is {}", test2.id(), test_2_balance);
    assert!(test_2_balance > test_2_initial_balance - test_2_deposit_to_pool);

    // expect err
    test1.call(pool.id(), "claim")
        .args_json(json!({"draw_id": 1, "pick": U128(winning_pick_test_1)}))
        .max_gas()
        .transact()
        .await?
        .into_result()
        .expect_err("Pick already claimed");

    let expected_defi_balance = defi_balance - (test_1_balance - (test_1_initial_balance - test_1_deposit_to_pool)) - (test_2_balance - (test_2_initial_balance - test_2_deposit_to_pool));
    // defi balance should be equal to reward + initial deposit minus the reward claimed by test1 user
    assert_eq!(ft_balance_of(defi.as_account(), token.id()).await?, expected_defi_balance);

    return Ok(());
}