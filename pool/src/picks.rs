use crate::interfaces::{picker::{Picker}, prize_distribution::PrizeDistributionActor};
use near_sdk::{collections::{UnorderedMap, Vector, LookupSet}};
use common::types::{DrawId, NumPicks};
use utils::storage_keys::StorageKeys;
use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct PickInfo{
    allowed_picks: NumPicks,
    claimed_picks: LookupSet<NumPicks>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct DrawPicks{
    draws: UnorderedMap<DrawId, PickInfo>,
}

impl DrawPicks{
    fn get_pick_info(&self, account_id: &AccountId, draw_id: &DrawId) -> PickInfo{
        return self.draws.get(draw_id).unwrap_or_else(||{
            let draw_id_bytes:&[u8] = &(draw_id.to_le_bytes());

            PickInfo { 
                allowed_picks: NumPicks::default(), 
                claimed_picks: LookupSet::new(
                    StorageKeys::AccountClaimedPicks { 
                        account_hash: env::sha256_array(&[account_id.as_bytes(), draw_id_bytes].concat()) 
                    }
                ) 
            }
        });
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct AccountsPicks{
    accounts: UnorderedMap<AccountId, DrawPicks>
}

impl Default for AccountsPicks{
    fn default() -> Self {
        AccountsPicks { accounts: UnorderedMap::new(StorageKeys::AccountPicks) }
    }
}

impl AccountsPicks{
    fn get_draws(&self, account_id: &AccountId) -> DrawPicks{
        return self.accounts.get(&account_id).unwrap_or_else(|| {
            DrawPicks {
                draws: UnorderedMap::new(
                    StorageKeys::AccountDrawPicks { 
                        account_hash: utils::utils::get_hash(&account_id)
                    }
                )
            }
        });
    }

    pub fn remove_claimed_pick_for_draw(&mut self, account_id: &AccountId, draw_id: &DrawId, pick: NumPicks){
        let mut acc_draws_picks = self.get_draws(account_id);
        let mut pick_info = acc_draws_picks.get_pick_info(account_id, draw_id);

        assert!(pick_info.allowed_picks != NumPicks::default(), "Picks should be already generated");
        pick_info.claimed_picks.remove(&pick);
        
        acc_draws_picks.draws.insert(draw_id, &pick_info);
        self.accounts.insert(&account_id, &acc_draws_picks);
    }

    pub fn check_and_add_picks_for_draw(&mut self, account_id: &AccountId, draw_id: &DrawId, pick: NumPicks){
        let mut acc_draws_picks = self.get_draws(account_id);
        let mut pick_info = acc_draws_picks.get_pick_info(account_id, draw_id);
        assert!(pick_info.allowed_picks != NumPicks::default(), "There are no generated picks for this draw for client");
        assert!(pick_info.allowed_picks > pick, "Invalid pick");
        assert!(!pick_info.claimed_picks.contains(&pick), "Pick already claimed");

        pick_info.claimed_picks.insert(&pick);
        acc_draws_picks.draws.insert(draw_id, &pick_info);
        self.accounts.insert(&account_id, &acc_draws_picks);
    }

    pub fn add_picks_for_draw(&mut self, account_id: &AccountId, draw_id: &DrawId, picks: NumPicks){
        let mut acc_draws_picks = self.get_draws(account_id);
        let mut pick = acc_draws_picks.get_pick_info(account_id, draw_id);

        assert!(pick.allowed_picks == NumPicks::default(), "Picks should not be already generated");

        pick.allowed_picks = picks;
        acc_draws_picks.draws.insert(draw_id, &pick);
        self.accounts.insert(&account_id, &acc_draws_picks);
    }
}

#[near_bindgen]
impl Contract{
    #[private]
    pub fn on_get_draw_calculate_picks(&mut self, account_id: AccountId, #[callback_result] call_result: Result<Draw, PromiseError>) -> NumPicks{
        if call_result.is_err() {
            log!("{:?}", call_result.err().unwrap());
            panic!("Cannot get draw")
        }
        let draw = call_result.unwrap();
        
        let acc_tickets = self.tickets.average_balance_between_timestamps(&account_id, draw.started_at, draw.completed_at);
        let total_tickets = self.tickets.average_total_supply_between_timestamps(draw.started_at, draw.completed_at);
        let prize_distribution = self.get_prize_distribution(draw.draw_id);
        let acc_picks: NumPicks = prize_distribution.max_picks * acc_tickets / total_tickets;
        self.acc_picks.add_picks_for_draw(&account_id, &draw.draw_id, acc_picks);

        return acc_picks;
    }
}

#[near_bindgen]
impl Picker for Contract{
    fn get_picks(&self, draw_id: DrawId) -> PromiseOrValue<NumPicks> {
        let caller = env::signer_account_id();
        let acc_draws_picks = self.acc_picks.get_draws(&caller);

        let draw_picks = acc_draws_picks.draws.get(&draw_id);
        if draw_picks.is_some(){
            return PromiseOrValue::Value(draw_picks.unwrap().allowed_picks);
        } else {
            let draw_promise = ext_draw::get_draw(draw_id, self.draw_contract.clone(), 0, gas::GET_DRAW);
            // ext_draw::ext(self.draw_contract.clone())
            // .with_static_gas(gas::GET_DRAW)
            // .get_draw(draw_id);

            let picks = draw_promise.then(this_contract::on_get_draw_calculate_picks(caller, env::current_account_id(), 0, gas::ON_GET_DRAW));
            // draw_promise.then(
            //     Self::ext(env::current_account_id())
            //     .with_static_gas(gas::GET_DRAW)
            //     .on_get_draw_calculate_picks(caller));

            return PromiseOrValue::Promise(picks);
        }
    }
}