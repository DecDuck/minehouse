use std::sync::Arc;

use common::{rpc::MinehouseError, work::{WorkUnit, WorkUnitData}};

use crate::state::MinehouseState;

pub mod pool;
pub mod index_container;

pub struct FinishedWorkUnit(pub WorkUnit);

pub trait WorkUnitAction {
    fn action(self, state: Arc<MinehouseState>) -> impl Future<Output = Result<(), MinehouseError>>;
}

/// Implements [`WorkUnitAction`] for [`WorkUnitData`] by delegating to each variant's inner value.
macro_rules! impl_work_unit_action {
    ($($variant:ident),* $(,)?) => {
        impl WorkUnitAction for WorkUnitData {
            async fn action(self, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
                match self {
                    $(WorkUnitData::$variant(inner) => inner.action(state).await,)*
                }
            }
        }
    };
}

impl_work_unit_action!(IndexContainer);

impl WorkUnitAction for WorkUnit {
    async fn action(self, state: Arc<MinehouseState>) -> Result<(), MinehouseError> {
        self.data.action(state).await
    }
}
