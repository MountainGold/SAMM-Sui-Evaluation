// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;

use crate::models_v2::address_metrics::{StoredActiveAddress, StoredAddress, StoredAddressMetrics};
use crate::models_v2::checkpoints::StoredCheckpoint;
use crate::models_v2::move_call_metrics::{StoredMoveCall, StoredMoveCallMetrics};
use crate::models_v2::network_metrics::StoredEpochPeakTps;
use crate::models_v2::transactions::{
    StoredTransactionCheckpoint, StoredTransactionSuccessCommandCount, StoredTransactionTimestamp,
};
use crate::models_v2::tx_count_metrics::StoredTxCountMetrics;
use crate::models_v2::tx_indices::{StoredTxCalls, StoredTxRecipients, StoredTxSenders};
use crate::types_v2::IndexerResult;

#[async_trait]
pub trait IndexerAnalyticalStore {
    async fn get_latest_stored_checkpoint(&self) -> IndexerResult<StoredCheckpoint>;
    async fn get_checkpoints_in_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredCheckpoint>>;
    async fn get_tx_timestamps_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionTimestamp>>;
    async fn get_tx_checkpoints_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionCheckpoint>>;
    async fn get_tx_success_cmd_counts_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionSuccessCommandCount>>;

    // for network metrics including TPS and counts of objects etc.
    async fn get_latest_tx_count_metrics(&self) -> IndexerResult<StoredTxCountMetrics>;
    async fn get_latest_epoch_peak_tps(&self) -> IndexerResult<StoredEpochPeakTps>;
    async fn persist_tx_count_metrics(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<()>;
    async fn persist_epoch_peak_tps(&self, epoch: i64) -> IndexerResult<()>;

    // for address metrics
    async fn get_latest_address_metrics(&self) -> IndexerResult<StoredAddressMetrics>;
    fn persist_addresses(&self, addresses: Vec<StoredAddress>) -> IndexerResult<()>;
    async fn get_senders_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxSenders>>;
    async fn get_recipients_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxRecipients>>;
    fn persist_active_addresses(
        &self,
        active_addresses: Vec<StoredActiveAddress>,
    ) -> IndexerResult<()>;
    async fn calculate_address_metrics(
        &self,
        checkpoint: StoredCheckpoint,
    ) -> IndexerResult<StoredAddressMetrics>;
    async fn persist_address_metrics(
        &self,
        address_metrics: StoredAddressMetrics,
    ) -> IndexerResult<()>;

    // for move call metrics
    async fn get_latest_move_call_metrics(&self) -> IndexerResult<StoredMoveCallMetrics>;
    async fn get_move_calls_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxCalls>>;
    fn persist_move_calls(&self, move_calls: Vec<StoredMoveCall>) -> IndexerResult<()>;
    async fn calculate_move_call_metrics(
        &self,
        checkpoint: StoredCheckpoint,
    ) -> IndexerResult<Vec<StoredMoveCallMetrics>>;
    async fn persist_move_call_metrics(
        &self,
        move_call_metrics: Vec<StoredMoveCallMetrics>,
    ) -> IndexerResult<()>;
}
