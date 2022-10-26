use crate::interfaces::{picker::{Picker}, prize_distribution::PrizeDistributionActor};
use near_sdk::{collections::{UnorderedMap}};
use common::types::{DrawId, NumPicks};
use utils::storage_keys::StorageKeys;
use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct DrawPicks{
    draws: UnorderedMap<DrawId, NumPicks>,
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

    pub fn get_picks_for_draw(&self, account_id: &AccountId, draw_id: &DrawId) -> NumPicks{
        return self.get_draws(&account_id).draws.get(&draw_id).unwrap_or_default();
    }

    pub fn add_picks_for_draw(&mut self, account_id: &AccountId, draw_id: &DrawId, picks: NumPicks){
        let mut acc_draws_picks = self.get_draws(account_id);
        if acc_draws_picks.draws.get(&draw_id).is_some(){
            return;
        }

        acc_draws_picks.draws.insert(&draw_id, &picks);
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
        log!("{:?}", draw);
        
        let acc_tickets = self.tickets.average_balance_between_timestamps(&account_id, draw.started_at, draw.completed_at);
        let total_tickets = self.tickets.average_total_supply_between_timestamps(draw.started_at, draw.completed_at);
        let prize_distribution = self.get_prize_distribution(draw.draw_id);
        let acc_picks: NumPicks = (prize_distribution.number_of_picks as u128) * acc_tickets / total_tickets;
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
            return PromiseOrValue::Value(draw_picks.unwrap());
        } else {
            let draw_promise = ext_draw::get_draw(draw_id, self.draw_contract.clone(), 0, gas::GET_DRAW);
            // ext_draw::ext(self.draw_contract.clone())
            // .with_static_gas(gas::GET_DRAW)
            // .get_draw(draw_id);

            let picks = draw_promise.then(this_contract::on_get_draw_calculate_picks(caller, env::current_account_id(), 0, gas::GET_DRAW));
            // draw_promise.then(
            //     Self::ext(env::current_account_id())
            //     .with_static_gas(gas::GET_DRAW)
            //     .on_get_draw_calculate_picks(caller));

            return PromiseOrValue::Promise(picks);
        }
    }
}