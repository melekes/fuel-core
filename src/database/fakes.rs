use std::collections::HashMap;
use crate::models::coin::{CoinId, Coin};
use crate::database::{CoinStore, CoinStoreError};

pub struct FakeCoinStore(HashMap<CoinId, Coin>);

impl CoinStore for FakeCoinStore {
    async fn get(&self, id: &CoinId) -> Result<Coin, CoinStoreError> {
        self.0.get(&id).cloned().ok_or_else(|| CoinStoreError::CoinNotFound)
    }

    async fn store(&mut self, coin: Coin) -> Result<(), CoinStoreError> {
        self.0.insert(coin.id, coin);
        Ok(())
    }

    async fn remove(&mut self, id: &CoinId) -> Result<(), CoinStoreError> {
        self.0.remove(id).ok_or_else(|| CoinStoreError::CoinNotFound);
        Ok(())
    }
}

