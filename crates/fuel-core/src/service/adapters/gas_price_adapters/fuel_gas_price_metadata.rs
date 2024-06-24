use fuel_core_gas_price_service::fuel_gas_price_updater::{
    Error as GasPriceError,
    MetadataStorage,
    Result as GasPriceResult,
    UpdaterMetadata,
};
use fuel_core_storage::{
    tables::GasPriceMetadata,
    transactional::AtomicView,
    StorageAsMut,
    StorageAsRef,
    StorageInspect,
    StorageMutate,
};
use fuel_core_types::fuel_types::BlockHeight;

#[cfg(test)]
mod tests;

pub struct FuelGasPriceMetadataStorage<Database> {
    database: Database,
}

#[async_trait::async_trait]
impl<Database> MetadataStorage for FuelGasPriceMetadataStorage<Database>
where
    Database: AtomicView<Height = BlockHeight>,
    Database::View: StorageAsRef,
    Database::View: StorageInspect<GasPriceMetadata>,
    Database::View: StorageMutate<GasPriceMetadata>,
    <Database::View as StorageInspect<GasPriceMetadata>>::Error: Into<anyhow::Error>,
{
    async fn get_metadata(
        &self,
        block_height: &BlockHeight,
    ) -> GasPriceResult<Option<UpdaterMetadata>> {
        let view = self.database.latest_view();
        let metadata = view
            .storage::<GasPriceMetadata>()
            .get(block_height)
            .map_err(|err| GasPriceError::CouldNotFetchMetadata {
                block_height: *block_height,
                source_error: err.into(),
            })?;
        Ok(metadata.map(|inner| inner.into_owned()))
    }

    async fn set_metadata(&mut self, _metadata: UpdaterMetadata) -> GasPriceResult<()> {
        let block_height = _metadata.l2_block_height();
        let mut view = self.database.latest_view();
        view.storage_as_mut::<GasPriceMetadata>()
            .insert(&block_height, &_metadata)
            .map_err(|err| GasPriceError::CouldNotSetMetadata {
                block_height,
                source_error: err.into(),
            })?;
        Ok(())
    }
}
