use fuel_tx::{Transaction, Hash};
use fuel_asm::Word;

pub type BlockHeight = Word;

#[derive(Debug, Copy, Clone)]
pub struct Block {
    pub id: Hash,
    pub height: BlockHeight,
    pub transactions: Vec<Transaction>
}