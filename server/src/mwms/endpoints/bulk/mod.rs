//! Bulk storage is the warehouse floor: stock is spread across many far-apart,
//! category-based containers where density matters more than pick speed. This
//! example implements the bookkeeping the trait actually cares about, accepting
//! re-index snapshots and rejecting transfers that would overflow the region.

mod container;
mod document;
mod endpoint;

pub use endpoint::BulkStorage;
pub(crate) use endpoint::PlannedDestination;
