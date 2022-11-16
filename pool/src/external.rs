use crate::*;

// Callback
#[ext_contract(this_contract)]
pub trait ExtSelf {
    fn on_get_draw_and_add_prize_distribution(&mut self, prize_awards: U128, #[callback_result] call_result: Result<Draw, PromiseError>);
    fn on_get_draw_calculate_picks(&mut self, account_id: AccountId, #[callback_result] call_result: Result<Draw, PromiseError>) -> U128;
    fn on_get_reward_from_defi(&self, #[callback_result] call_result: Result<Option<AccountDetailedView>, PromiseError>)-> U128;
    fn on_after_rewards_claim_from_defi(&mut self, account_id: AccountId, amount: U128, #[callback_result] result: Result<(), PromiseError>);
}

#[ext_contract(ext_draw)]
pub trait ExtDraw {
    fn get_draw(&self, id: DrawId) -> Draw;
}

#[ext_contract(ext_defi)]
pub trait ExtDeFi {
    fn show_reward(&self, account_id: AccountId) -> Vec<TokenAmountsView>;
    fn get_account(&self, account_id: AccountId) -> Option<AccountDetailedView>;
    fn execute(&self, actions: Vec<Action>);
}

#[ext_contract(ext_fungible_token)]
pub trait FungibleTokenContract {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);

    fn ft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        amount: U128,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<U128>;

    /// Returns the total supply of the token in a decimal string representation.
    fn ft_total_supply(&self) -> U128;

    /// Returns the balance of the account. If the account doesn't exist, `"0"` must be returned.
    fn ft_balance_of(&self, account_id: AccountId) -> U128;
}