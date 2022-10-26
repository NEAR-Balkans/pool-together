use near_sdk::{env, near_bindgen, EpochHeight, PanicOnDefault};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};

const DRAW_DURATION_IN_EPOCHS: u64 = 5;
const DRAW_BUFFER_CAPACITY:usize = 3;

#[cfg(test)]
mod utils;

#[cfg(test)]
pub mod tests {
    use near_sdk::{serde_json::json, json_types::U128};
    use crate::utils::*;
    
    #[test]
    fn draw_test(){
        let mut env= Env::init();
        
        let can_start_draw = env.draw.view(env.draw.account_id(), "can_start_draw", &[]).unwrap_json::<bool>();
        assert!(can_start_draw == true);
        env.draw.call(env.draw.account_id(), "start_draw", &[], DEFAULT_GAS.0, 0).assert_success();
        env.print_epoch();
        let mut draw_time = 4;
        while draw_time > 0{
            env.print_epoch();
            let can_complete_draw = env.draw.view(env.draw.account_id(), "can_complete_draw", &[]).unwrap_json::<bool>();
            assert!(can_complete_draw == false);
            draw_time-=1;
            env.wait_epoch();
        }

        env.draw.call(env.draw.account_id(), "complete_draw", &[], DEFAULT_GAS.0, 0);
        let x = env.draw.view(env.draw.account_id(), "get_draws", &json!({"from_index": 0, "limit": 10}).to_string().into_bytes()).unwrap_json_value();
        println!("{:?}", x);
    }

    #[test]
    fn test_claim_picks(){
        let mut env = Env::init();
        let mut tokens = Tokens::init(&env);
        let mut users = Users::init(&env);

        ft_storage_deposit(&users.alice, &tokens.correct_token.account_id());
        ft_storage_deposit(&users.bob, &tokens.correct_token.account_id());

        ft_storage_deposit(&env.pool, &tokens.correct_token.account_id());
        ft_storage_deposit(&env.defi, &tokens.correct_token.account_id());

        ft_transfer(&tokens.correct_token.account_id(), &tokens.correct_token, &users.alice.account_id(), to_token_amount(30));
        ft_transfer(&tokens.correct_token.account_id(), &tokens.correct_token, &users.bob.account_id(), to_token_amount(100));
        
        assert_eq!(ft_balance_of(&tokens.correct_token.account_id(), &users.alice), to_token_amount(30));

        // Send 10 tokens from Alice to pool contract
        ft_transfer_call(&tokens.correct_token.account_id(), &users.alice, &env.pool.account_id(), to_token_amount(10), "");
        // Alice should have 20 tokens left in the correct_token contract
        assert_eq!(ft_balance_of(&tokens.correct_token.account_id(), &users.alice), to_token_amount(20));
        // Defi contract should have 10 tokens in the correct_token contract
        assert_eq!(ft_balance_of(&tokens.correct_token.account_id(), &env.defi), to_token_amount(10));
        // Pool contract should have no tokens in the correct_token contract
        assert_eq!(ft_balance_of(&tokens.correct_token.account_id(), &env.pool), 0);

        env.draw.call(env.draw.account_id(), "start_draw", &[], DEFAULT_GAS.0, 0);
        env.wait_epoch();
        env.wait_epoch();
        ft_transfer_call(&tokens.correct_token.account_id(), &users.bob, &env.pool.account_id(), to_token_amount(40), "");
        let mut can_complete_draw = env.draw.view(env.draw.account_id(), "can_complete_draw", &[]).unwrap_json::<bool>();
        while !can_complete_draw{
            env.wait_epoch();
        }
        env.draw.call(env.draw.account_id(), "complete_draw", &[], DEFAULT_GAS.0, 0).assert_success();
        ft_transfer_call(&tokens.correct_token.account_id(), &tokens.correct_token, &env.defi.account_id, to_token_amount(300), env.pool.account_id.as_str());
        println!("{:?}", env.defi.view(env.defi.account_id(), "show_reward", &json!({"account_id": env.pool.account_id}).to_string().into_bytes()).unwrap_json_value());
        let add_prize_distribution = env.pool.call(env.pool.account_id(), "add_prize_distribution", &json!({"draw_id": 1}).to_string().into_bytes(), MAX_GAS.0, 0);
        println!("{:?}", add_prize_distribution);
        add_prize_distribution.assert_success();

        let prize_distr = env.pool.view(env.pool.account_id(), "get_prize_distribution", &json!({"draw_id": 1}).to_string().into_bytes()).unwrap_json_value();
        println!("{:?}", prize_distr);

        let mut picks = users.alice.call(env.pool.account_id(), "get_picks", &json!({"draw_id": 1}).to_string().into_bytes(), MAX_GAS.0, 0).unwrap_json::<u128>();
        println!("{:?}", picks);

        while picks >= 1{
            let res = users.alice.call(env.pool.account_id(), "claim", &json!({"draw_id": U128(1), "pick": U128(picks)}).to_string().into_bytes(), MAX_GAS.0, 0);
            println!("{:?}", res);
            picks-=1;
        }
        

    }
}
