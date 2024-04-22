use self::import_task::ImportTable;

use super::{progress::MultipleProgressReporter, task_manager::TaskManager};
mod import_task;
mod logic;

use crate::{
    combined_database::CombinedDatabase,
    database::database_description::{off_chain::OffChain, on_chain::OnChain},
    fuel_core_graphql_api::storage::messages::SpentMessages,
    graphql_api::storage::{
        coins::OwnedCoins,
        contracts::ContractsInfo,
        messages::OwnedMessageIds,
        old::{OldFuelBlockConsensus, OldFuelBlocks, OldTransactions},
        transactions::{OwnedTransactions, TransactionStatuses},
    },
};
use fuel_core_chain_config::{AsTable, SnapshotReader, StateConfig, TableEntry};
use fuel_core_services::StateWatcher;
use fuel_core_storage::{
    kv_store::StorageColumn,
    structured_storage::TableWithBlueprint,
    tables::{
        Coins, ContractsAssets, ContractsLatestUtxo, ContractsRawCode, ContractsState,
        FuelBlocks, Messages, ProcessedTransactions, SealedBlockConsensus, Transactions,
    },
};
use fuel_core_types::{
    blockchain::{block::Block, primitives::DaBlockHeight},
    fuel_types::BlockHeight,
};

pub struct SnapshotImporter {
    db: CombinedDatabase,
    task_manager: TaskManager<()>,
    genesis_block: Block,
    snapshot_reader: SnapshotReader,
    multi_progress_reporter: MultipleProgressReporter,
}

impl SnapshotImporter {
    fn new(
        db: CombinedDatabase,
        genesis_block: Block,
        snapshot_reader: SnapshotReader,
        watcher: StateWatcher,
    ) -> Self {
        Self {
            db,
            genesis_block,
            task_manager: TaskManager::new(watcher),
            snapshot_reader,
            multi_progress_reporter: MultipleProgressReporter::new(tracing::info_span!(
                "snapshot_importer"
            )),
        }
    }

    pub async fn import(
        db: CombinedDatabase,
        genesis_block: Block,
        snapshot_reader: SnapshotReader,
        watcher: StateWatcher,
    ) -> anyhow::Result<()> {
        Self::new(db, genesis_block, snapshot_reader, watcher)
            .run_workers()
            .await
    }

    async fn run_workers(mut self) -> anyhow::Result<()> {
        tracing::info!("Running imports");
        self.spawn_worker::<Coins>()?;
        self.spawn_worker::<ContractsAssets>()?;
        self.spawn_worker::<ContractsLatestUtxo>()?;
        self.spawn_worker::<ContractsRawCode>()?;
        self.spawn_worker::<ContractsState>()?;
        self.spawn_worker::<FuelBlocks>()?;
        self.spawn_worker::<Messages>()?;
        self.spawn_worker::<OldFuelBlockConsensus>()?;
        self.spawn_worker::<OldFuelBlocks>()?;
        self.spawn_worker::<OldTransactions>()?;
        self.spawn_worker::<OwnedTransactions>()?;
        self.spawn_worker::<ProcessedTransactions>()?;
        self.spawn_worker::<SealedBlockConsensus>()?;
        self.spawn_worker::<SpentMessages>()?;
        self.spawn_worker::<TransactionStatuses>()?;
        self.spawn_worker::<Transactions>()?;

        self.task_manager.wait().await?;

        Ok(())
    }

    pub fn spawn_worker<TableInSnapshot>(&mut self) -> anyhow::Result<()>
    where
        TableInSnapshot: TableWithBlueprint + 'static + Send,
        TableEntry<TableInSnapshot>: serde::de::DeserializeOwned + Send,
        StateConfig: AsTable<TableInSnapshot>,
        Handler: ImportTable<TableInSnapshot>,
    {
        let groups = self.snapshot_reader.read::<TableInSnapshot>()?;
        let num_groups = groups.len();

        let block_height = *self.genesis_block.header().height();
        let da_block_height = self.genesis_block.header().da_height;

        let on_chain_db = self.db.on_chain().clone();
        let off_chain_db = self.db.off_chain().clone();

        let progress_reporter = self
            .multi_progress_reporter
            .table_reporter::<TableInSnapshot>(Some(num_groups));

        self.task_manager.spawn(move |token| {
            tokio_rayon::spawn(move || {
                import_task::import_entries(
                    token,
                    Handler::new(block_height, da_block_height),
                    groups,
                    on_chain_db,
                    off_chain_db,
                    progress_reporter,
                )
            })
        });

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Handler {
    pub block_height: BlockHeight,
    pub da_block_height: DaBlockHeight,
}

impl Handler {
    pub fn new(block_height: BlockHeight, da_block_height: DaBlockHeight) -> Self {
        Self {
            block_height,
            da_block_height,
        }
    }
}
