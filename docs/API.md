# The list of APIs that are provided by the contract

Notes:
- `u128_dec_format`, `WrappedBalance`, `Shares` means the value is passed as a decimal string representation. E.g. `1` serialized as `"1"`
- `BigDecimal` is serialized as floating string representation. E.g. `1.5` serialized as `"1.5"`
- `u64` means the value is passed as an integer.
- `Option<_>` means the value can be omitted, or provided as `null`.
- Rust enums are serialized using JSON objects. E.g. `FarmId::Supplied("token.near")` is serialized as `{"Supplied": "token.near"}`
- `HashMap<_, _>` is serialized using JSON objects.

Pool Contract
```rust
trait Contract {
    /// Initializes the contract
    #[init]
    fn new_default_meta(owner_id: AccountId, token_for_deposit: AccountId, draw_contract: AccountId, burrow_address: AccountId, reward_token: AccountId) -> Self;

    #[init]
    fn new(
        owner_id: AccountId,
        deposited_token_id: AccountId,
        metadata: FungibleTokenMetadata,
        draw_contract: AccountId,
        burrow_address: AccountId,
        reward_token: AccountId,
    ) -> Self;

    /// Function showing what token the contract holds
    fn get_asset(&self) -> AccountId

    /// Send some near yoctoNear to the contract, to be able to do fungible token transfers on your behalf
    #[payable]
    fn accept_deposit_for_future_fungible_token_transfers(&mut self);

    /// Showing how much tokens the contract has generated
    fn get_reward(&self) -> Promise;

    /// Showing how much the contract deposited to the external defi
    fn get_deposited_amount(&self) -> Balance;

    /// Returns the fungible token metadata as per NEP-141 spec
    fn ft_metadata(&self) -> FungibleTokenMetadata;

    /// Get number of picks for signer_account_id for the draw
    fn get_picks(&self, draw_id: DrawId) -> PromiseOrValue<NumPicks>;

    /// Get prize distribution for draw
    fn get_prize_distribution(&self, draw_id: DrawId) -> PrizeDistribution;

    /// Add prize distribution for draw, can be called by the owner of the contract only
    fn add_prize_distribution(&mut self, draw_id: DrawId, prize_awards: Balance);

    /// Signer account claims prize for draw and provided picks
    fn claim(&mut self, draw_id: U128, pick: U128) -> u128;

    struct PrizeDistribution{
        pub number_of_picks: u64,
        pub draw_id: u128,
        pub cardinality: u8,
        pub bit_range_size: u8,
        pub tiers: [u32; MAX_TIERS],
        pub prize: u128,
        pub max_picks: u128,
        pub start_time: u64,
        pub end_time: u64,
        #[serde(skip_serializing)]
        pub winning_number: WinningNumber,
    }
```

Draw Contract
```rust
trait Contract{
    /// Initializes the contract
    #[init]
    fn new() -> Self;

    /// get draws
    fn get_draws(&self, from_index: usize, limit: usize) -> Vec<Draw>;

    /// get draws by draw id
    fn get_draw(&self, id: DrawId) -> Draw;

    // Returns true/false whether a draw has started or not
    /// if true then a draw is not started, otherwise a draw is started
    fn can_start_draw(&self) -> bool;

    /// Returns true/false whether a draw can be completed
    /// if false then a draw has not started yet or the draw duration has not passed
    fn can_complete_draw(&self) -> bool;

    /// Starts a draw, internally checks if draw can be started
    fn start_draw(&mut self);

    /// Completes a draw, internally checks if draw can be completed
    /// The completed draw is added to the ring buffer
    fn complete_draw(&mut self);


    struct Draw {
        pub winning_random_number: WinningNumber,
        pub draw_id: DrawId,
        pub started_at: u64,
        pub completed_at: u64,
    }
}
```