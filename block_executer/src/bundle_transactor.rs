//! Bundle execution of transaction

use fuel_tx::{Receipt, Transaction, TxId, UtxoId};

use fuel_vm::{
    prelude::Transactor,
    storage::InterpreterStorage,
};

use fuel_tx::crypto::Hasher;
use fuel_types::bytes;
use fuel_vm::consts::*;
use fuel_vm::prelude::*;
use fuel_vm::substorage::{Metadata, SubStorage};
use hashbrown::HashMap;

/// Bundle transaction execution into one module.
pub struct BundleTransactor<'a, STORAGE> {
    /// executed and commited transactions.
    /// Maybe even had TxId in vec for order and HashMap<TxId,Tx> for fast fetching.
    transactions: Vec<(Transaction, Vec<Receipt>)>,
    /// tx_id to index;
    transction_order: HashMap<TxId, usize>,
    /// outputs that are spend or newly created by one of previous transactions in bundle.
    /// Can probably be done outside of VM but for not leave it here.
    outputs: HashMap<UtxoId, UtxoStatus>,
    /// pending used outputs
    pending_outputs: HashMap<UtxoId, UtxoStatus>,
    /// Executor
    transactor: Transactor<'a, SubStorage<STORAGE>>,
}

pub enum UtxoStatus {
    Spend,
    Unspend,
    FromDB,
}

impl<STORAGE> BundleTransactor<'_, STORAGE> {
    /// Create bundler.
    pub fn new(storage: STORAGE, metadata: Metadata) -> Self {
        Self {
            transactions: Vec::new(),
            outputs: HashMap::new(),
            transction_order: HashMap::new(),
            pending_outputs: HashMap::new(),
            transactor: Transactor::new(SubStorage::new(storage, metadata)),
        }
    }

    /// Get reference to internal storage.
    pub fn storage(&self) -> &SubStorage<STORAGE> {
        self.transactor.as_ref()
    }

    /// Iterate over all inputs and check ones that are not pre_checked. Add all outputs to pending_outputs.
    fn check_dependent_outputs(
        &mut self,
        tx: &Transaction,
        pre_checked_inputs: &HashMap<UtxoId, bool>,
    ) -> Result<(), TransactorError> {
        // iterate over all inputs and check ones that are not prechecked before execution
        for input in tx.inputs() {
            let utxo_id = input.utxo_id();
            // check if we precheked this output. This includes db.
            if let Some(is_db) = pre_checked_inputs.get(utxo_id) {
                self.pending_outputs.insert(
                    utxo_id.clone(),
                    if *is_db {
                        UtxoStatus::FromDB
                    } else {
                        UtxoStatus::Spend
                    },
                );
            }
            // TODO
            self.pending_outputs
                .insert(utxo_id.clone(), UtxoStatus::Spend);
        }

        //TODO iterate over outputs and add them
        for (output_index, _) in tx.outputs().iter().enumerate() {
            // Safety: it is okay to cast to u8 bcs output number is already checked.
            let utxo_id = UtxoId::new(tx.id(), output_index as u8);
            self.pending_outputs.insert(utxo_id, UtxoStatus::Unspend);
        }

        Ok(())
    }
}

/// Bundler errors.
pub enum TransactorError {
    /// None
    None,
    /// Some
    Some,
    /// Hello
    HelloError,
}

impl<STORAGE> BundleTransactor<'_, STORAGE>
where
    STORAGE: InterpreterStorage,
{
    /// transaction that we want to include.
    /// Idea of pre_checked_output is to do precalculation on inputs/outputs before it comes to transactor.and
    /// in transactor check only variable outputs that we dont know value beforehand.
    /// TODO specify what is expected and what checks we dont need to do
    pub fn transact(
        &mut self,
        tx: Transaction,
        pre_checked_inputs: &HashMap<UtxoId, bool>,
    ) -> Result<&mut Self, TransactorError> {
        // TODO clean state
        self.check_dependent_outputs(&tx, pre_checked_inputs)?;
        self.transactor.transact(tx);
        Ok(self)
    }

    /// Commit last executed transaction and do cleanup on state
    pub fn commit(&mut self) -> &mut Self {
        if let Ok(transition) = self.transactor.result() {
            let outputs = self.pending_outputs.drain();
            self.outputs.extend(outputs.into_iter());
            // commit transaction transition to bundler.
            let index = self.transactions.len();
            self.transactions
                .push((transition.tx().clone(), transition.receipts().to_vec()));
            self.transction_order.insert(transition.tx().id(), index);
            self.transactor.as_mut().commit_pending();
            self.transactor.clear();
        } else {
            self.transactor.as_mut().reject_pending();
        }
        self
    }

    pub fn transactions(&self) -> &[(Transaction, Vec<Receipt>)] {
        &self.transactions
    }

    pub fn outputs(&self) -> &HashMap<UtxoId, UtxoStatus> {
        &self.outputs
    }

    /// cleanup the state and return list of executed transactions and used outputs
    pub fn finalize(&mut self) {}
}

// For transactor
impl<'a, STORAGE> AsRef<Transactor<'a, SubStorage<STORAGE>>> for BundleTransactor<'a, STORAGE> {
    fn as_ref(&self) -> &Transactor<'a, SubStorage<STORAGE>> {
        &self.transactor
    }
}

// For storage
impl<'a, STORAGE> AsRef<SubStorage<STORAGE>> for BundleTransactor<'a, STORAGE> {
    fn as_ref(&self) -> &SubStorage<STORAGE> {
        self.transactor.as_ref()
    }
}

// Mut for transactor
impl<'a, STORAGE> AsMut<Transactor<'a, SubStorage<STORAGE>>> for BundleTransactor<'a, STORAGE> {
    fn as_mut(&mut self) -> &mut Transactor<'a, SubStorage<STORAGE>> {
        &mut self.transactor
    }
}

// Mut for storage
impl<'a, STORAGE> AsMut<SubStorage<STORAGE>> for BundleTransactor<'a, STORAGE> {
    fn as_mut(&mut self) -> &mut SubStorage<STORAGE> {
        self.transactor.as_mut()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::{rngs::StdRng, Rng};
    use fuel_vm::memory_client::MemoryStorage;

    #[test]
    fn initial_test() {
        let metadata = Metadata::default();
        let storage = MemoryStorage::new(metadata.block_height(), *metadata.coinbase());
        let bundler = BundleTransactor::new(storage, metadata);
        //bundler.transact()
    }

    #[test]
    pub fn simple_execution() {
        const WORD_SIZE: usize = core::mem::size_of::<Word>();

        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};
        // tests data are taken from tests/blockchain/state_read_write

        // FIRST TRANSACTIONS
        let rng = &mut StdRng::seed_from_u64(2322u64);

        // Create a program with two main routines
        //
        // 0 - Fetch (key, value) from call[b] and state[key] += value
        //
        // 1 - Fetch (key, value) from call[b], unpack b into 4x16, xor the limbs of
        // state[key] with unpacked b

        let salt: Salt = rng.gen();

        let tx1 = tx1(salt);
        let tx2 = tx2(rng, salt);

        // create bundler
        let metadata = Metadata::default();
        let storage = MemoryStorage::new(metadata.block_height(), *metadata.coinbase());
        let mut bundler = BundleTransactor::new(storage, metadata);

        let pco = HashMap::new();
        assert!(
            (bundler.transact(tx1, &pco).ok().unwrap() as &dyn AsRef<Transactor<'_, _>>)
                .as_ref()
                .result()
                .is_ok()
        );
        bundler.commit();

        let storage = (&bundler as &dyn AsRef<SubStorage<_>>).as_ref();
        println!("STATE:{:?}", storage.commited_storage());

        assert!(
            (bundler.transact(tx2, &pco).ok().unwrap() as &dyn AsRef<Transactor<'_, _>>)
                .as_ref()
                .result()
                .is_ok()
        );
        bundler.commit();

        let storage = (&bundler as &dyn AsRef<SubStorage<_>>).as_ref();
        println!("STATE2:{:?}", storage.commited_storage());
    }

    fn contract() -> (Contract, Witness) {
        #[rustfmt::skip]
    let function_selector: Vec<Opcode> = vec![
        Opcode::ADDI(0x30, REG_ZERO, 0),
        Opcode::ADDI(0x31, REG_ONE, 0),
    ];

        #[rustfmt::skip]
    let call_arguments_parser: Vec<Opcode> = vec![
        Opcode::ADDI(0x10, REG_FP, CallFrame::a_offset() as Immediate12),
        Opcode::LW(0x10, 0x10, 0),
        Opcode::ADDI(0x11, REG_FP, CallFrame::b_offset() as Immediate12),
        Opcode::LW(0x11, 0x11, 0),
    ];

        #[rustfmt::skip]
    let routine_add_word_to_state: Vec<Opcode> = vec![
        Opcode::JNEI(0x10, 0x30, 13),       // (0, b) Add word to state
        Opcode::LW(0x20, 0x11, 4),          // r[0x20]      := m[b+32, 8]
        Opcode::SRW(0x21, 0x11),            // r[0x21]      := s[m[b, 32], 8]
        Opcode::ADD(0x20, 0x20, 0x21),      // r[0x20]      += r[0x21]
        Opcode::SWW(0x11, 0x20),            // s[m[b,32]]   := r[0x20]
        Opcode::LOG(0x20, 0x21, 0x00, 0x00),
        Opcode::RET(REG_ONE),
    ];

        #[rustfmt::skip]
    let routine_unpack_and_xor_limbs_into_state: Vec<Opcode> = vec![
        Opcode::JNEI(0x10, 0x31, 45),       // (1, b) Unpack arg into 4x16 and xor into state
        Opcode::ADDI(0x20, REG_ZERO, 32),   // r[0x20]      := 32
        Opcode::ALOC(0x20),                 // aloc            0x20
        Opcode::ADDI(0x20, REG_HP, 1),      // r[0x20]      := $hp+1
        Opcode::SRWQ(0x20, 0x11),           // m[0x20,32]   := s[m[b, 32], 32]
        Opcode::LW(0x21, 0x11, 4),          // r[0x21]      := m[b+32, 8]
        Opcode::LOG(0x21, 0x00, 0x00, 0x00),
        Opcode::SRLI(0x22, 0x21, 48),       // r[0x22]      := r[0x21] >> 48
        Opcode::SRLI(0x23, 0x21, 32),       // r[0x23]      := r[0x21] >> 32
        Opcode::ANDI(0x23, 0x23, 0xff),     // r[0x23]      &= 0xffff
        Opcode::SRLI(0x24, 0x21, 16),       // r[0x24]      := r[0x21] >> 16
        Opcode::ANDI(0x24, 0x24, 0xff),     // r[0x24]      &= 0xffff
        Opcode::ANDI(0x25, 0x21, 0xff),     // r[0x25]      := r[0x21] & 0xffff
        Opcode::LOG(0x22, 0x23, 0x24, 0x25),
        Opcode::LW(0x26, 0x20, 0),          // r[0x26]      := m[$fp, 8]
        Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
        Opcode::LOG(0x26, 0x00, 0x00, 0x00),
        Opcode::SW(0x20, 0x26, 0),          // m[0x20,8]    := r[0x26]
        Opcode::LW(0x26, 0x20, 1),          // r[0x26]      := m[$fp+8, 8]
        Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
        Opcode::LOG(0x26, 0x00, 0x00, 0x00),
        Opcode::SW(0x20, 0x26, 1),          // m[0x20+8,8]  := r[0x26]
        Opcode::LW(0x26, 0x20, 2),          // r[0x26]      := m[$fp+16, 8]
        Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
        Opcode::LOG(0x26, 0x00, 0x00, 0x00),
        Opcode::SW(0x20, 0x26, 2),          // m[0x20+16,8] := r[0x26]
        Opcode::LW(0x26, 0x20, 3),          // r[0x26]      := m[$fp+24, 8]
        Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
        Opcode::LOG(0x26, 0x00, 0x00, 0x00),
        Opcode::SW(0x20, 0x26, 3),          // m[0x20+24,8] := r[0x26]
        Opcode::SWWQ(0x11, 0x20),           // s[m[b,32],32]:= m[0x20, 32]
        Opcode::RET(REG_ONE),
    ];

        #[rustfmt::skip]
    let invalid_call: Vec<Opcode> = vec![
        Opcode::RET(REG_ZERO),
    ];

        let program: Witness = function_selector
            .into_iter()
            .chain(call_arguments_parser.into_iter())
            .chain(routine_add_word_to_state.into_iter())
            .chain(routine_unpack_and_xor_limbs_into_state.into_iter())
            .chain(invalid_call.into_iter())
            .collect::<Vec<u8>>()
            .into();

        (Contract::from(program.as_ref()), program)
    }

    fn tx1(salt: Salt) -> Transaction {
        let gas_price = 0;
        let gas_limit = 1_000_000;
        let maturity = 0;

        // Create a program with two main routines
        //
        // 0 - Fetch (key, value) from call[b] and state[key] += value
        //
        // 1 - Fetch (key, value) from call[b], unpack b into 4x16, xor the limbs of
        // state[key] with unpacked b

        let (contract, program) = contract();
        let contract_root = contract.root();
        let contract = contract.id(&salt, &contract_root);

        let output = Output::contract_created(contract);

        let bytecode_witness = 0;
        let tx_deploy = Transaction::create(
            gas_price,
            gas_limit,
            maturity,
            bytecode_witness,
            salt,
            vec![],
            vec![],
            vec![output],
            vec![program],
        );
        tx_deploy
    }

    fn tx2(rng: &mut StdRng, salt: Salt) -> Transaction {
        let gas_price = 0;
        let gas_limit = 1_000_000;
        let maturity = 0;

        let (contract, _) = contract();

        let contract_root = contract.root();
        let contract = contract.id(&salt, &contract_root);

        let input = Input::contract(rng.gen(), rng.gen(), rng.gen(), contract);
        let output = Output::contract(0, rng.gen(), rng.gen());

        // The script needs to locate the data offset at runtime. Hence, we need to know
        // upfront the serialized size of the script so we can set the registers
        // accordingly.
        //
        // This variable is created to assert we have correct script size in the
        // instructions.
        let script_len = 16;

        // Based on the defined script length, we set the appropriate data offset
        let script_data_offset = VM_TX_MEMORY + Transaction::script_offset() + script_len;
        let script_data_offset = script_data_offset as Immediate12;

        let script = vec![
            Opcode::ADDI(0x10, REG_ZERO, script_data_offset),
            Opcode::ADDI(0x11, REG_ZERO, gas_limit as Immediate12),
            Opcode::CALL(0x10, REG_ZERO, 0x10, 0x11),
            Opcode::RET(REG_ONE),
        ]
        .iter()
        .copied()
        .collect::<Vec<u8>>();

        // Assert the offsets are set correctnly
        let offset =
            VM_TX_MEMORY + Transaction::script_offset() + bytes::padded_len(script.as_slice());
        assert_eq!(script_data_offset, offset as Immediate12);

        let mut script_data = vec![];

        // Routine to be called: Add word to state
        let routine: Word = 0;

        // Offset of the script data relative to the call data
        let call_data_offset = script_data_offset as usize + ContractId::LEN + 2 * WORD_SIZE;
        let call_data_offset = call_data_offset as Word;

        // Key and value to be added
        let key = Hasher::hash(b"some key");
        let val: Word = 150;

        // Script data containing the call arguments (contract, a, b) and (key, value)
        script_data.extend(contract.as_ref());
        script_data.extend(&routine.to_be_bytes());
        script_data.extend(&call_data_offset.to_be_bytes());
        script_data.extend(key.as_ref());
        script_data.extend(&val.to_be_bytes());

        let tx_add_word = Transaction::script(
            gas_price,
            gas_limit,
            maturity,
            script.clone(),
            script_data,
            vec![input.clone()],
            vec![output],
            vec![],
        );
        tx_add_word
    }

    /*
    fn state_read_write() {
        let rng = &mut StdRng::seed_from_u64(2322u64);

        let mut client = MemoryClient::default();

        let gas_price = 0;
        let gas_limit = 1_000_000;
        let maturity = 0;

        let salt: Salt = rng.gen();

        // Create a program with two main routines
        //
        // 0 - Fetch (key, value) from call[b] and state[key] += value
        //
        // 1 - Fetch (key, value) from call[b], unpack b into 4x16, xor the limbs of
        // state[key] with unpacked b

        #[rustfmt::skip]
        let function_selector: Vec<Opcode> = vec![
            Opcode::ADDI(0x30, REG_ZERO, 0),
            Opcode::ADDI(0x31, REG_ONE, 0),
        ];

        #[rustfmt::skip]
        let call_arguments_parser: Vec<Opcode> = vec![
            Opcode::ADDI(0x10, REG_FP, CallFrame::a_offset() as Immediate12),
            Opcode::LW(0x10, 0x10, 0),
            Opcode::ADDI(0x11, REG_FP, CallFrame::b_offset() as Immediate12),
            Opcode::LW(0x11, 0x11, 0),
        ];

        #[rustfmt::skip]
        let routine_add_word_to_state: Vec<Opcode> = vec![
            Opcode::JNEI(0x10, 0x30, 13),       // (0, b) Add word to state
            Opcode::LW(0x20, 0x11, 4),          // r[0x20]      := m[b+32, 8]
            Opcode::SRW(0x21, 0x11),            // r[0x21]      := s[m[b, 32], 8]
            Opcode::ADD(0x20, 0x20, 0x21),      // r[0x20]      += r[0x21]
            Opcode::SWW(0x11, 0x20),            // s[m[b,32]]   := r[0x20]
            Opcode::LOG(0x20, 0x21, 0x00, 0x00),
            Opcode::RET(REG_ONE),
        ];

        #[rustfmt::skip]
        let routine_unpack_and_xor_limbs_into_state: Vec<Opcode> = vec![
            Opcode::JNEI(0x10, 0x31, 45),       // (1, b) Unpack arg into 4x16 and xor into state
            Opcode::ADDI(0x20, REG_ZERO, 32),   // r[0x20]      := 32
            Opcode::ALOC(0x20),                 // aloc            0x20
            Opcode::ADDI(0x20, REG_HP, 1),      // r[0x20]      := $hp+1
            Opcode::SRWQ(0x20, 0x11),           // m[0x20,32]   := s[m[b, 32], 32]
            Opcode::LW(0x21, 0x11, 4),          // r[0x21]      := m[b+32, 8]
            Opcode::LOG(0x21, 0x00, 0x00, 0x00),
            Opcode::SRLI(0x22, 0x21, 48),       // r[0x22]      := r[0x21] >> 48
            Opcode::SRLI(0x23, 0x21, 32),       // r[0x23]      := r[0x21] >> 32
            Opcode::ANDI(0x23, 0x23, 0xff),     // r[0x23]      &= 0xffff
            Opcode::SRLI(0x24, 0x21, 16),       // r[0x24]      := r[0x21] >> 16
            Opcode::ANDI(0x24, 0x24, 0xff),     // r[0x24]      &= 0xffff
            Opcode::ANDI(0x25, 0x21, 0xff),     // r[0x25]      := r[0x21] & 0xffff
            Opcode::LOG(0x22, 0x23, 0x24, 0x25),
            Opcode::LW(0x26, 0x20, 0),          // r[0x26]      := m[$fp, 8]
            Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
            Opcode::LOG(0x26, 0x00, 0x00, 0x00),
            Opcode::SW(0x20, 0x26, 0),          // m[0x20,8]    := r[0x26]
            Opcode::LW(0x26, 0x20, 1),          // r[0x26]      := m[$fp+8, 8]
            Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
            Opcode::LOG(0x26, 0x00, 0x00, 0x00),
            Opcode::SW(0x20, 0x26, 1),          // m[0x20+8,8]  := r[0x26]
            Opcode::LW(0x26, 0x20, 2),          // r[0x26]      := m[$fp+16, 8]
            Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
            Opcode::LOG(0x26, 0x00, 0x00, 0x00),
            Opcode::SW(0x20, 0x26, 2),          // m[0x20+16,8] := r[0x26]
            Opcode::LW(0x26, 0x20, 3),          // r[0x26]      := m[$fp+24, 8]
            Opcode::XOR(0x26, 0x26, 0x22),      // r[0x26]      ^= r[0x22]
            Opcode::LOG(0x26, 0x00, 0x00, 0x00),
            Opcode::SW(0x20, 0x26, 3),          // m[0x20+24,8] := r[0x26]
            Opcode::SWWQ(0x11, 0x20),           // s[m[b,32],32]:= m[0x20, 32]
            Opcode::RET(REG_ONE),
        ];

        #[rustfmt::skip]
        let invalid_call: Vec<Opcode> = vec![
            Opcode::RET(REG_ZERO),
        ];

        let program: Witness = function_selector
            .into_iter()
            .chain(call_arguments_parser.into_iter())
            .chain(routine_add_word_to_state.into_iter())
            .chain(routine_unpack_and_xor_limbs_into_state.into_iter())
            .chain(invalid_call.into_iter())
            .collect::<Vec<u8>>()
            .into();

        let contract = Contract::from(program.as_ref());
        let contract_root = contract.root();
        let contract = contract.id(&salt, &contract_root);

        let output = Output::contract_created(contract);

        let bytecode_witness = 0;
        let tx_deploy = Transaction::create(
            gas_price,
            gas_limit,
            maturity,
            bytecode_witness,
            salt,
            vec![],
            vec![],
            vec![output],
            vec![program],
        );

        let input = Input::contract(rng.gen(), rng.gen(), rng.gen(), contract);
        let output = Output::contract(0, rng.gen(), rng.gen());

        // The script needs to locate the data offset at runtime. Hence, we need to know
        // upfront the serialized size of the script so we can set the registers
        // accordingly.
        //
        // This variable is created to assert we have correct script size in the
        // instructions.
        let script_len = 16;

        // Based on the defined script length, we set the appropriate data offset
        let script_data_offset = VM_TX_MEMORY + Transaction::script_offset() + script_len;
        let script_data_offset = script_data_offset as Immediate12;

        let script = vec![
            Opcode::ADDI(0x10, REG_ZERO, script_data_offset),
            Opcode::ADDI(0x11, REG_ZERO, gas_limit as Immediate12),
            Opcode::CALL(0x10, REG_ZERO, 0x10, 0x11),
            Opcode::RET(REG_ONE),
        ]
        .iter()
        .copied()
        .collect::<Vec<u8>>();

        // Assert the offsets are set correctnly
        let offset = VM_TX_MEMORY + Transaction::script_offset() + bytes::padded_len(script.as_slice());
        assert_eq!(script_data_offset, offset as Immediate12);

        let mut script_data = vec![];

        // Routine to be called: Add word to state
        let routine: Word = 0;

        // Offset of the script data relative to the call data
        let call_data_offset = script_data_offset as usize + ContractId::LEN + 2 * WORD_SIZE;
        let call_data_offset = call_data_offset as Word;

        // Key and value to be added
        let key = Hasher::hash(b"some key");
        let val: Word = 150;

        // Script data containing the call arguments (contract, a, b) and (key, value)
        script_data.extend(contract.as_ref());
        script_data.extend(&routine.to_be_bytes());
        script_data.extend(&call_data_offset.to_be_bytes());
        script_data.extend(key.as_ref());
        script_data.extend(&val.to_be_bytes());

        let tx_add_word = Transaction::script(
            gas_price,
            gas_limit,
            maturity,
            script.clone(),
            script_data,
            vec![input.clone()],
            vec![output],
            vec![],
        );

        // Assert the initial state of `key` is empty
        let state = client.as_ref().contract_state(&contract, &key);
        assert_eq!(Bytes32::default(), state.into_owned());

        client.transact(tx_deploy);
        client.transact(tx_add_word);






        let receipts = client.receipts().expect("The transaction was executed");
        let state = client.as_ref().contract_state(&contract, &key);

        // Assert the state of `key` is mutated to `val`
        assert_eq!(&val.to_be_bytes()[..], &state.as_ref()[..WORD_SIZE]);

        // Expect the correct receipt
        assert_eq!(receipts[1].ra().expect("Register value expected"), val);
        assert_eq!(receipts[1].rb().expect("Register value expected"), 0);

        let mut script_data = vec![];

        // Routine to be called: Unpack and XOR into state
        let routine: Word = 1;

        // Create limbs reference values
        let a = 0x25;
        let b = 0xc1;
        let c = 0xd3;
        let d = 0xaa;

        let val: Word = (a << 48) | (b << 32) | (c << 16) | d;

        // Script data containing the call arguments (contract, a, b) and (key, value)
        script_data.extend(contract.as_ref());
        script_data.extend(&routine.to_be_bytes());
        script_data.extend(&call_data_offset.to_be_bytes());
        script_data.extend(key.as_ref());
        script_data.extend(&val.to_be_bytes());

        let tx_unpack_xor = Transaction::script(
            gas_price,
            gas_limit,
            maturity,
            script,
            script_data,
            vec![input],
            vec![output],
            vec![],
        );

        // Mutate the state
        client.transact(tx_unpack_xor);

        let receipts = client.receipts().expect("Expected receipts");

        // Expect the arguments to be received correctly by the VM
        assert_eq!(receipts[1].ra().expect("Register value expected"), val);
        assert_eq!(receipts[2].ra().expect("Register value expected"), a);
        assert_eq!(receipts[2].rb().expect("Register value expected"), b);
        assert_eq!(receipts[2].rc().expect("Register value expected"), c);
        assert_eq!(receipts[2].rd().expect("Register value expected"), d);

        let m = a ^ 0x96;
        let n = a ^ 0x00;
        let o = a ^ 0x00;
        let p = a ^ 0x00;

        // Expect the value to be unpacked correctly into 4x16 limbs + XOR state
        assert_eq!(receipts[3].ra().expect("Register value expected"), m);
        assert_eq!(receipts[4].ra().expect("Register value expected"), n);
        assert_eq!(receipts[5].ra().expect("Register value expected"), o);
        assert_eq!(receipts[6].ra().expect("Register value expected"), p);

        let mut bytes = [0u8; 32];

        // Reconstruct the final state out of the limbs
        (&mut bytes[..8]).copy_from_slice(&m.to_be_bytes());
        (&mut bytes[8..16]).copy_from_slice(&n.to_be_bytes());
        (&mut bytes[16..24]).copy_from_slice(&o.to_be_bytes());
        (&mut bytes[24..]).copy_from_slice(&p.to_be_bytes());

        // Assert the state is correct
        let bytes = Bytes32::from(bytes);
        let state = client.as_ref().contract_state(&contract, &key);
        assert_eq!(bytes, state.into_owned());
    }

    */
}
