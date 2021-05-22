use crate::database::{Database, CoinStore};
use crate::models::block::Block;
use fuel_tx::Transaction;
use crate::prelude::Input;
use crate::models::coin::{Coin, CoinStatus};

#[async_trait::async_trait]
pub trait BlockExecutor {
    async fn execute_block(&self, block: Block) -> Result<(), BlockExecutionError>;
    async fn process_transaction(&self, transaction: Transaction, block: Block) -> Result<(), BlockExecutionError>;
}

#[derive(Error)]
pub enum BlockExecutionError {}

#[derive(Error)]
pub enum TransactionValidityError {
    #[error("Coin input was already spent")]
    CoinAlreadySpent,
    #[error("Coin has not yet reached maturity")]
    CoinHasNotMatured
}

pub trait BlockExecutorConfig {
    type Database: Database + Send + Sync;
}

pub struct BlockExecutorImpl<T: BlockExecutorConfig> {
    database: T::Database
}

#[async_trait::async_trait]
impl<T: BlockExecutorConfig> BlockExecutor for BlockExecutorImpl<T> {
    async fn execute_block(&self, block: Block) -> Result<(), BlockExecutionError> {
        todo!()
    }

    async fn process_transaction(&self, transaction: Transaction, block: Block) -> Result<(), BlockExecutionError> {
        transaction.validate(block.height)?;

        Ok(())
    }
}

impl<T: BlockExecutorConfig> BlockExecutorImpl<T> {
    fn verify_input_state(&self, transaction: Transaction, block: Block) -> Result<(), TransactionValidityError> {
        for input in transaction.inputs() {
            match input {
                Input::Coin { utxo_id, .. } => {
                    let coin: Coin = self.database.get(*utxo_id)?;
                    if coin.status == CoinStatus::Spent {
                        return Err(TransactionValidityError::CoinAlreadySpent)
                    }
                    if block.height < coin.block_created + coin.maturity {
                        return Err(TransactionValidityError::CoinHasNotMatured)
                    }
                }
                Input::Contract { .. } => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cannot_spend_non_existent_coins() {

    }

    #[test]
    fn cannot_spend_coins_already_spent() {

    }
}