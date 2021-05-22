use crate::models::coin::{Coin, CoinId};
use thiserror::Error;

pub mod fakes;

pub trait Database: CoinStore + ContractStore {}

#[async_trait::async_trait]
pub trait CoinStore {
    async fn get(&self, id: &CoinId) -> Result<Coin, CoinStoreError>;
    async fn store(&mut self, coin: Coin) -> Result<(), CoinStoreError>;
    async fn remove(&mut self, id: &CoinId) -> Result<(), CoinStoreError>;
}

#[derive(Error)]
pub enum CoinStoreError {
    CoinNotFound
}

pub trait ContractStore {

}

