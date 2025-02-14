use crate::{
    eth::backend::db::{
        Db, MaybeHashDatabase, SerializableAccountRecord, SerializableState, StateDb,
    },
    revm::primitives::AccountInfo,
    Address, U256,
};
use ethers::prelude::H256;
pub use foundry_evm::executor::fork::database::ForkedDatabase;
use foundry_evm::{
    executor::{
        backend::{snapshot::StateSnapshot, DatabaseResult},
        fork::database::ForkDbSnapshot,
    },
    revm::Database,
};
use foundry_utils::types::{ToAlloy, ToEthers};

/// Implement the helper for the fork database
impl Db for ForkedDatabase {
    fn insert_account(&mut self, address: Address, account: AccountInfo) {
        self.database_mut().insert_account(address, account)
    }

    fn set_storage_at(&mut self, address: Address, slot: U256, val: U256) -> DatabaseResult<()> {
        // this ensures the account is loaded first
        let _ = Database::basic(self, address.to_alloy())?;
        self.database_mut().set_storage_at(address, slot, val)
    }

    fn insert_block_hash(&mut self, number: U256, hash: H256) {
        self.inner().block_hashes().write().insert(number.to_alloy(), hash.to_alloy());
    }

    fn dump_state(&self) -> DatabaseResult<Option<SerializableState>> {
        let mut db = self.database().clone();
        let accounts = self
            .database()
            .accounts
            .clone()
            .into_iter()
            .map(|(k, v)| -> DatabaseResult<_> {
                let code = if let Some(code) = v.info.code {
                    code
                } else {
                    db.code_by_hash(v.info.code_hash)?
                }
                .to_checked();
                Ok((
                    k.to_ethers(),
                    SerializableAccountRecord {
                        nonce: v.info.nonce,
                        balance: v.info.balance.to_ethers(),
                        code: code.bytes()[..code.len()].to_vec().into(),
                        storage: v
                            .storage
                            .into_iter()
                            .map(|kv| (kv.0.to_ethers(), kv.1.to_ethers()))
                            .collect(),
                    },
                ))
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(SerializableState { accounts }))
    }

    fn snapshot(&mut self) -> U256 {
        self.insert_snapshot().to_ethers()
    }

    fn revert(&mut self, id: U256) -> bool {
        self.revert_snapshot(id.to_alloy())
    }

    fn current_state(&self) -> StateDb {
        StateDb::new(self.create_snapshot())
    }
}

impl MaybeHashDatabase for ForkedDatabase {
    fn clear_into_snapshot(&mut self) -> StateSnapshot {
        let db = self.inner().db();
        let accounts = std::mem::take(&mut *db.accounts.write());
        let storage = std::mem::take(&mut *db.storage.write());
        let block_hashes = std::mem::take(&mut *db.block_hashes.write());
        StateSnapshot { accounts, storage, block_hashes }
    }

    fn clear(&mut self) {
        self.flush_cache();
        self.clear_into_snapshot();
    }

    fn init_from_snapshot(&mut self, snapshot: StateSnapshot) {
        let db = self.inner().db();
        let StateSnapshot { accounts, storage, block_hashes } = snapshot;
        *db.accounts.write() = accounts;
        *db.storage.write() = storage;
        *db.block_hashes.write() = block_hashes;
    }
}
impl MaybeHashDatabase for ForkDbSnapshot {
    fn clear_into_snapshot(&mut self) -> StateSnapshot {
        std::mem::take(&mut self.snapshot)
    }

    fn clear(&mut self) {
        std::mem::take(&mut self.snapshot);
        self.local.clear()
    }

    fn init_from_snapshot(&mut self, snapshot: StateSnapshot) {
        self.snapshot = snapshot;
    }
}
