use fuel_tx::{Hash, Address, Color};
use fuel_asm::Word;
use crate::models::block::BlockHeight;

pub type CoinId = Hash;

#[derive(Debug, Copy, Clone)]
pub struct Coin {
    pub id: CoinId,
    pub owner: Address,
    pub amount: Word,
    pub color: Color,
    pub maturity: BlockHeight,
    pub status: CoinStatus,
    pub block_created: BlockHeight,
}

#[derive(Debug, Copy, Clone)]
pub enum CoinStatus {
    Unspent,
    Spent
}
