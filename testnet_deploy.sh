#!/bin/bash
set -e

./build_all.sh

MASTER_ACCOUNT=$1
BURROW_ACCOUNT=$2
TOKEN=$3

if [ -z "$4" ]
then
      REWARD_TOKEN=$TOKEN
else
      REWARD_TOKEN=$4
fi

echo $MASTER_ACCOUNT $BURROW_ACCOUNT $TOKEN $REWARD_TOKEN

POOL="pool.$MASTER_ACCOUNT"
DRAW="draw.$MASTER_ACCOUNT"

echo "Creating accounts"
near create-account $POOL --masterAccount=$MASTER_ACCOUNT --initialBalance=10
near create-account $DRAW --masterAccount=$MASTER_ACCOUNT --initialBalance=10

echo "Deploying"
near deploy --accountId $DRAW --wasmFile ./res/draw.wasm --initFunction new --initArgs '{"owner_id": "'$DRAW'"}'

near deploy --accountId $POOL --wasmFile ./res/pool.wasm --initFunction new_default_meta --initArgs '{
      "owner_id": "'$POOL'", 
      "token_for_deposit": "'$TOKEN'", 
      "draw_contract": "'$DRAW'",
      "burrow_address": "'$BURROW_ACCOUNT'",
      "reward_token": "'$REWARD_TOKEN'",
      "min_pick_cost": "10000000000000000000000"
    }'

echo "Storage deposit"
near call $BURROW_ACCOUNT storage_deposit --args '' --amount=1 --accountId $POOL
near call $TOKEN storage_deposit --accountId $POOL --args '' --amount=0.0125
if [ "$TOKEN" != "$REWARD_TOKEN" ]; then
  near call $REWARD_TOKEN storage_deposit --accountId $POOL --args '' --amount=0.0125
fi