use serde::{Deserialize, Serialize};

use crate::{item_stack::ItemStack, mwms::transfers::Transfer, work::WorkUnitDone};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedTransferMove {
    pub move_index: usize,
    /// Offset within the 36 player inventory slots, independent of menu size.
    pub player_slot_offset: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransferWorkUnit {
    pub transfer: Transfer,
    pub completed: Vec<usize>,
    pub staged: Vec<StagedTransferMove>,
    pub from_contents: Option<Vec<ItemStack>>,
    pub to_contents: Option<Vec<ItemStack>>,
}

impl WorkUnitDone for TransferWorkUnit {
    fn is_done(&self) -> bool {
        self.staged.is_empty()
            && self.from_contents.is_some()
            && self.to_contents.is_some()
            && self.completed.len() == self.transfer.moves.len()
            && (0..self.transfer.moves.len()).all(|index| self.completed.contains(&index))
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{mwms::transfers::ItemMove, vec::Point};

    fn work_unit() -> TransferWorkUnit {
        TransferWorkUnit {
            transfer: Transfer {
                from_container: Uuid::new_v4(),
                from_position: Point {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                to_container: Uuid::new_v4(),
                to_position: Point {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                },
                moves: vec![ItemMove {
                    from_slot: 1,
                    to_slot: 2,
                    quantity: 3,
                }],
            },
            completed: Vec::new(),
            staged: Vec::new(),
            from_contents: None,
            to_contents: None,
        }
    }

    #[test]
    fn completes_only_after_moves_and_snapshots_are_finished() {
        let mut unit = work_unit();
        unit.completed.push(0);
        unit.from_contents = Some(Vec::new());
        unit.to_contents = Some(Vec::new());
        assert!(unit.is_done());

        unit.staged.push(StagedTransferMove {
            move_index: 0,
            player_slot_offset: 0,
        });
        assert!(!unit.is_done());
    }

    #[test]
    fn duplicate_progress_does_not_complete_missing_moves() {
        let mut unit = work_unit();
        unit.transfer.moves.push(ItemMove {
            from_slot: 3,
            to_slot: 4,
            quantity: 1,
        });
        unit.completed = vec![0, 0];
        unit.from_contents = Some(Vec::new());
        unit.to_contents = Some(Vec::new());

        assert!(!unit.is_done());
    }

    #[test]
    fn extra_progress_does_not_complete_a_transfer() {
        let mut unit = work_unit();
        unit.completed = vec![0, 1];
        unit.from_contents = Some(Vec::new());
        unit.to_contents = Some(Vec::new());

        assert!(!unit.is_done());
    }
}
