use crate::*;
use near_contract_standards::fungible_token::metadata::{FungibleTokenMetadata, FT_METADATA_SPEC};
use near_contract_standards::storage_management::StorageBalance;
use near_sdk::json_types::U128;
use near_sdk::serde_json::json;
use near_sdk::{env, serde_json, Balance, Gas, Timestamp};
use near_sdk_sim::types::AccountId;
use near_sdk_sim::{
    deploy, init_simulator, to_yocto, ContractAccount, ExecutionResult, UserAccount,
};
use near_sdk_sim::runtime::RuntimeStandalone;

const FT_WASM_BYTES: &[u8] = include_bytes!("../../res/fungible_token.wasm");
const DEFI_WASM_BYTES: &[u8] = include_bytes!("../../res/defi.wasm");
const DRAW_WASM_BYTES: &[u8] = include_bytes!("../../res/draw.wasm");
const POOL_WASM_BYTES: &[u8] = include_bytes!("../../res/pool.wasm");

pub fn defi_bytes() -> &'static [u8] {
    &DEFI_WASM_BYTES
}

pub fn draw_bytes() -> &'static [u8] {
    &DRAW_WASM_BYTES
}

pub fn ft_bytes() -> &'static [u8] {
    &FT_WASM_BYTES
}

pub fn pool_bytes() -> &'static [u8] {
    &POOL_WASM_BYTES
}

pub const NEAR: &str = "near";
pub const POOL_ID: &str = "pool.near";
pub const DEFI_ID: &str = "defi.near";
pub const DRAW_ID: &str = "draw.near";
pub const FT_ID: &str = "ft.near";
pub const FALSE_FT_ID: &str = "another-ft.near";
pub const OWNER_ID: &str = "owner.near";

pub const DEFAULT_GAS: Gas = Gas(Gas::ONE_TERA.0 * 15);
pub const MAX_GAS: Gas = Gas(Gas::ONE_TERA.0 * 300);

const TOKEN_SYMBOL: &str = "USDC";
const TOKEN_DESCRIPTION: &str = "USD Coin on the blockchain";
const FT_TOKEN_DECIMALS: u32 = 0;
const FT_TOKEN_TOTAL_SUPPLY: u128 = 1000;

pub struct Env {
    pub root: UserAccount,
    pub near: UserAccount,
    pub owner: UserAccount,
    pub pool: UserAccount,
    pub draw: UserAccount,
    pub defi: UserAccount,
}

pub struct Tokens{
    pub correct_token: UserAccount,
    pub false_token: UserAccount,
}

pub struct Users {
    pub alice: UserAccount,
    pub bob: UserAccount
}

pub fn storage_deposit(
    user: &UserAccount,
    contract_id: &AccountId,
    account_id: &AccountId,
    attached_deposit: Balance,
) {
    user.call(
        contract_id.clone().into(),
        "storage_deposit",
        &json!({ "account_id": account_id }).to_string().into_bytes(),
        DEFAULT_GAS.0,
        attached_deposit,
    )
    .assert_success();
}

pub fn ft_storage_deposit(
    user: &UserAccount,
    token_account_id: &AccountId,
    account_id: &AccountId,
) {
    storage_deposit(
        user,
        token_account_id,
        account_id,
        125 * env::STORAGE_PRICE_PER_BYTE,
    );
}

impl Env{
    pub fn init() -> Self{
        let root = init_simulator(None);
        
        let near = root.create_user(
            NEAR.to_string(),
            to_yocto("10000")
        );
        
        let owner = near.create_user(
            OWNER_ID.to_string(),
            to_yocto("100")
        );

        let ft = near.deploy_and_init(
            ft_bytes(),
            FT_ID.to_string(),
            "new",
            &json!({
                "owner_id": FT_ID, 
                "total_supply": (FT_TOKEN_TOTAL_SUPPLY * 10u128.pow(FT_TOKEN_DECIMALS)).to_string(), 
                "metadata": 
                    { 
                        "spec": "ft-1.0.0", 
                        "name": TOKEN_DESCRIPTION, 
                        "symbol": TOKEN_SYMBOL, 
                        "decimals": FT_TOKEN_DECIMALS 
                    }
            })
            .to_string()
            .into_bytes(),
            to_yocto("10"),
            MAX_GAS.0
        );

        let draw = near.deploy_and_init(
            draw_bytes(),
            DRAW_ID.to_string(),
            "new",
            &[],
            to_yocto("10"),
            MAX_GAS.0,
        );

        
        println!("TEST");

        // let defi = near.deploy_and_init(
        //     defi_bytes(),
        //     DEFI_ID.to_string(),
        //     "new",
        //     &[],
        //     to_yocto("10"),
        //     MAX_GAS.0,
        // );

        let pool = near.deploy_and_init(
            &pool_bytes(),
            POOL_ID.to_string(),
            "new_default_meta",
            &json!({
                "owner_id": POOL_ID, 
                "token_for_deposit": FT_ID, 
                "draw_contract": DRAW_ID, 
                "burrow_address": DEFI_ID
            })
            .to_string()
            .into_bytes(),
            to_yocto("10"),
            MAX_GAS.0,
        );


        return Self{
            root: root,
            near: near,
            owner: owner,
            pool: pool,
            draw: draw,
            defi: ft,
        };
    }
}