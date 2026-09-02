use azalea::Client;
use common::work::{WorkUnit, WorkUnitData};
use worker_core::{client::WorkUnitClient, tick::WorkerControlLoop};

use crate::index_region::index_region;

pub struct WarehouseWorker;

impl WorkerControlLoop for WarehouseWorker {
    fn can_accept(wu: &WorkUnit) -> bool {
        match wu.data {
            WorkUnitData::IndexRegion(_) => true,
            _ => false,
        }
    }

    async fn work(
        wu: WorkUnit,
        client: &WorkUnitClient,
        mc_client: &Client,
    ) -> Result<(), anyhow::Error> {
        match wu.data {
            WorkUnitData::IndexRegion(_) => index_region(wu, client, mc_client).await,
            WorkUnitData::IndexContainer(_) => {
                anyhow::bail!("warehouse worker cannot process container work units")
            }
        }
    }
}
