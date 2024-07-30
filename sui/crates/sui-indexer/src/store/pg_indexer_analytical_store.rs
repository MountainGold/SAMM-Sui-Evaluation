// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;
use tap::tap::TapFallible;
use tracing::error;

use async_trait::async_trait;
use core::result::Result::Ok;
use diesel::dsl::count;
use diesel::upsert::excluded;
use diesel::ExpressionMethods;
use diesel::{QueryDsl, RunQueryDsl};
use sui_types::base_types::ObjectID;

use crate::errors::{Context, IndexerError};
use crate::models_v2::address_metrics::{StoredActiveAddress, StoredAddress, StoredAddressMetrics};
use crate::models_v2::checkpoints::StoredCheckpoint;
use crate::models_v2::move_call_metrics::{
    build_move_call_metric_query, QueriedMoveCallMetrics, QueriedMoveMetrics, StoredMoveCall,
    StoredMoveCallMetrics,
};
use crate::models_v2::network_metrics::{StoredEpochPeakTps, Tps};
use crate::models_v2::transactions::{
    StoredTransactionCheckpoint, StoredTransactionSuccessCommandCount, StoredTransactionTimestamp,
};
use crate::models_v2::tx_count_metrics::StoredTxCountMetrics;
use crate::models_v2::tx_indices::{StoredTxCalls, StoredTxRecipients, StoredTxSenders};
use crate::schema_v2::{
    active_addresses, address_metrics, addresses, checkpoints, epoch_peak_tps, move_call_metrics,
    move_calls, transactions, tx_calls, tx_count_metrics, tx_recipients, tx_senders,
};
use crate::store::diesel_macro::{read_only_blocking, transactional_blocking_with_retry};
use crate::types_v2::IndexerResult;
use crate::PgConnectionPool;

use super::IndexerAnalyticalStore;

#[derive(Clone)]
pub struct PgIndexerAnalyticalStore {
    blocking_cp: PgConnectionPool,
}

impl PgIndexerAnalyticalStore {
    pub fn new(blocking_cp: PgConnectionPool) -> Self {
        Self { blocking_cp }
    }
}

#[async_trait]
impl IndexerAnalyticalStore for PgIndexerAnalyticalStore {
    async fn get_latest_stored_checkpoint(&self) -> IndexerResult<StoredCheckpoint> {
        let latest_cp = read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .order(checkpoints::sequence_number.desc())
                .first::<StoredCheckpoint>(conn)
        })
        .context("Failed reading latest checkpoint from PostgresDB")?;
        Ok(latest_cp)
    }

    async fn get_checkpoints_in_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredCheckpoint>> {
        let cps = read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .filter(checkpoints::sequence_number.ge(start_checkpoint))
                .filter(checkpoints::sequence_number.lt(end_checkpoint))
                .order(checkpoints::sequence_number.asc())
                .load::<StoredCheckpoint>(conn)
        })
        .context("Failed reading checkpoints from PostgresDB")?;
        Ok(cps)
    }

    async fn get_tx_timestamps_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionTimestamp>> {
        let tx_timestamps = read_only_blocking!(&self.blocking_cp, |conn| {
            transactions::dsl::transactions
                .filter(transactions::dsl::checkpoint_sequence_number.ge(start_checkpoint))
                .filter(transactions::dsl::checkpoint_sequence_number.lt(end_checkpoint))
                .order(transactions::dsl::tx_sequence_number.asc())
                .select((
                    transactions::dsl::tx_sequence_number,
                    transactions::dsl::timestamp_ms,
                ))
                .load::<StoredTransactionTimestamp>(conn)
        })
        .context("Failed reading transaction timestamps from PostgresDB")?;
        Ok(tx_timestamps)
    }

    async fn get_tx_checkpoints_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionCheckpoint>> {
        let tx_checkpoints = read_only_blocking!(&self.blocking_cp, |conn| {
            transactions::dsl::transactions
                .filter(transactions::dsl::checkpoint_sequence_number.ge(start_checkpoint))
                .filter(transactions::dsl::checkpoint_sequence_number.lt(end_checkpoint))
                .order(transactions::dsl::tx_sequence_number.asc())
                .select((
                    transactions::dsl::tx_sequence_number,
                    transactions::dsl::checkpoint_sequence_number,
                ))
                .load::<StoredTransactionCheckpoint>(conn)
        })
        .context("Failed reading transaction checkpoints from PostgresDB")?;
        Ok(tx_checkpoints)
    }

    async fn get_tx_success_cmd_counts_in_checkpoint_range(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<Vec<StoredTransactionSuccessCommandCount>> {
        let tx_success_cmd_counts = read_only_blocking!(&self.blocking_cp, |conn| {
            transactions::dsl::transactions
                .filter(transactions::dsl::checkpoint_sequence_number.ge(start_checkpoint))
                .filter(transactions::dsl::checkpoint_sequence_number.lt(end_checkpoint))
                .order(transactions::dsl::tx_sequence_number.asc())
                .select((
                    transactions::dsl::tx_sequence_number,
                    transactions::dsl::checkpoint_sequence_number,
                    transactions::dsl::success_command_count,
                    transactions::dsl::timestamp_ms,
                ))
                .load::<StoredTransactionSuccessCommandCount>(conn)
        })
        .context("Failed reading transaction success command counts from PostgresDB")?;
        Ok(tx_success_cmd_counts)
    }

    async fn get_latest_tx_count_metrics(&self) -> IndexerResult<StoredTxCountMetrics> {
        let latest_tx_count = read_only_blocking!(&self.blocking_cp, |conn| {
            tx_count_metrics::dsl::tx_count_metrics
                .order(tx_count_metrics::dsl::checkpoint_sequence_number.desc())
                .first::<StoredTxCountMetrics>(conn)
        })
        .context("Failed reading latest tx count metrics from PostgresDB")?;
        Ok(latest_tx_count)
    }

    async fn get_latest_epoch_peak_tps(&self) -> IndexerResult<StoredEpochPeakTps> {
        let latest_network_metrics = read_only_blocking!(&self.blocking_cp, |conn| {
            epoch_peak_tps::dsl::epoch_peak_tps
                .order(epoch_peak_tps::dsl::epoch.desc())
                .first::<StoredEpochPeakTps>(conn)
        })
        .context("Failed reading latest epoch peak TPS from PostgresDB")?;
        Ok(latest_network_metrics)
    }

    async fn persist_tx_count_metrics(
        &self,
        start_checkpoint: i64,
        end_checkpoint: i64,
    ) -> IndexerResult<()> {
        let tx_count_query = construct_checkpoint_tx_count_query(start_checkpoint, end_checkpoint);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::sql_query(tx_count_query.clone()).execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting tx count metrics to PostgresDB")?;
        Ok(())
    }

    async fn persist_epoch_peak_tps(&self, epoch: i64) -> IndexerResult<()> {
        let epoch_peak_tps_query = construct_peak_tps_query(epoch, 1);
        let peak_tps_30d_query = construct_peak_tps_query(epoch, 30);
        let epoch_tps: Tps = read_only_blocking!(&self.blocking_cp, |conn|
            // diesel::sql_query(epoch_peak_tps_query).first::<Option<f64>>(conn).map(|x| x.unwrap_or(0.0))
            diesel::RunQueryDsl::get_result(
                diesel::sql_query(epoch_peak_tps_query),
                conn
            ))
        .context("Failed reading epoch peak TPS from PostgresDB")?;
        let tps_30d: Tps = read_only_blocking!(&self.blocking_cp, |conn|
            // diesel::sql_query(peak_tps_30d_query).first::<Option<f64>>(conn).map(|x| x.unwrap_or(0.0))
            diesel::RunQueryDsl::get_result(
                diesel::sql_query(peak_tps_30d_query),
                conn
            ))
        .context("Failed reading 30d peak TPS from PostgresDB")?;

        let epoch_peak_tps = StoredEpochPeakTps {
            epoch,
            peak_tps: epoch_tps.peak_tps,
            peak_tps_30d: tps_30d.peak_tps,
        };
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(epoch_peak_tps::table)
                    .values(epoch_peak_tps.clone())
                    .on_conflict_do_nothing()
                    .execute(conn)
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting epoch peak TPS to PostgresDB.")?;
        Ok(())
    }

    async fn get_latest_address_metrics(&self) -> IndexerResult<StoredAddressMetrics> {
        let latest_address_metrics = read_only_blocking!(&self.blocking_cp, |conn| {
            address_metrics::dsl::address_metrics
                .order(address_metrics::dsl::checkpoint.desc())
                .first::<StoredAddressMetrics>(conn)
        })
        .context("Failed reading latest address metrics from PostgresDB")?;
        Ok(latest_address_metrics)
    }

    async fn get_senders_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxSenders>> {
        let senders = read_only_blocking!(&self.blocking_cp, |conn| {
            tx_senders::dsl::tx_senders
                .filter(tx_senders::dsl::tx_sequence_number.ge(start_tx_seq))
                .filter(tx_senders::dsl::tx_sequence_number.lt(end_tx_seq))
                .order(tx_senders::dsl::tx_sequence_number.asc())
                .load::<StoredTxSenders>(conn)
        })
        .context("Failed reading tx senders from PostgresDB")?;
        Ok(senders)
    }

    async fn get_recipients_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxRecipients>> {
        let recipients = read_only_blocking!(&self.blocking_cp, |conn| {
            tx_recipients::dsl::tx_recipients
                .filter(tx_recipients::dsl::tx_sequence_number.ge(start_tx_seq))
                .filter(tx_recipients::dsl::tx_sequence_number.lt(end_tx_seq))
                .order(tx_recipients::dsl::tx_sequence_number.asc())
                .load::<StoredTxRecipients>(conn)
        })
        .context("Failed reading tx recipients from PostgresDB")?;
        Ok(recipients)
    }

    fn persist_addresses(&self, addresses: Vec<StoredAddress>) -> IndexerResult<()> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(addresses::table)
                    .values(addresses.clone())
                    .on_conflict(addresses::address)
                    .do_update()
                    .set((
                        addresses::last_appearance_time
                            .eq(excluded(addresses::last_appearance_time)),
                        addresses::last_appearance_tx.eq(excluded(addresses::last_appearance_tx)),
                    ))
                    .execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting addresses to PostgresDB")?;
        Ok(())
    }

    fn persist_active_addresses(
        &self,
        active_addresses: Vec<StoredActiveAddress>,
    ) -> IndexerResult<()> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(active_addresses::table)
                    .values(active_addresses.clone())
                    .on_conflict(active_addresses::address)
                    .do_update()
                    .set((
                        active_addresses::last_appearance_time
                            .eq(excluded(active_addresses::last_appearance_time)),
                        active_addresses::last_appearance_tx
                            .eq(excluded(active_addresses::last_appearance_tx)),
                    ))
                    .execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting active addresses to PostgresDB")?;
        Ok(())
    }

    async fn calculate_address_metrics(
        &self,
        checkpoint: StoredCheckpoint,
    ) -> IndexerResult<StoredAddressMetrics> {
        let cp_timestamp_ms = checkpoint.timestamp_ms;
        let addr_count = read_only_blocking!(&self.blocking_cp, |conn| {
            addresses::dsl::addresses
                .filter(addresses::first_appearance_time.le(cp_timestamp_ms))
                .count()
                .get_result::<i64>(conn)
        })?;
        let active_addr_count = read_only_blocking!(&self.blocking_cp, |conn| {
            active_addresses::dsl::active_addresses
                .filter(active_addresses::first_appearance_time.le(cp_timestamp_ms))
                .count()
                .get_result::<i64>(conn)
        })?;
        let time_one_day_ago = cp_timestamp_ms - 1000 * 60 * 60 * 24;
        let daily_active_addresses = read_only_blocking!(&self.blocking_cp, |conn| {
            active_addresses::dsl::active_addresses
                .filter(active_addresses::first_appearance_time.le(cp_timestamp_ms))
                .filter(active_addresses::last_appearance_time.gt(time_one_day_ago))
                .select(count(active_addresses::address))
                .first(conn)
        })?;
        Ok(StoredAddressMetrics {
            checkpoint: checkpoint.sequence_number,
            epoch: checkpoint.epoch,
            timestamp_ms: checkpoint.timestamp_ms,
            cumulative_addresses: addr_count,
            cumulative_active_addresses: active_addr_count,
            daily_active_addresses,
        })
    }

    async fn persist_address_metrics(
        &self,
        address_metrics: StoredAddressMetrics,
    ) -> IndexerResult<()> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(address_metrics::table)
                    .values(address_metrics.clone())
                    .on_conflict_do_nothing()
                    .execute(conn)
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting address metrics to PostgresDB")?;
        Ok(())
    }

    async fn get_latest_move_call_metrics(&self) -> IndexerResult<StoredMoveCallMetrics> {
        let latest_move_call_metrics = read_only_blocking!(&self.blocking_cp, |conn| {
            move_call_metrics::dsl::move_call_metrics
                .order(move_call_metrics::checkpoint_sequence_number.desc())
                .first::<QueriedMoveCallMetrics>(conn)
        })
        .context("Failed reading latest move call metrics from PostgresDB")?;
        Ok(latest_move_call_metrics.into())
    }

    async fn get_move_calls_in_tx_range(
        &self,
        start_tx_seq: i64,
        end_tx_seq: i64,
    ) -> IndexerResult<Vec<StoredTxCalls>> {
        let move_calls = read_only_blocking!(&self.blocking_cp, |conn| {
            tx_calls::dsl::tx_calls
                .filter(tx_calls::dsl::tx_sequence_number.ge(start_tx_seq))
                .filter(tx_calls::dsl::tx_sequence_number.lt(end_tx_seq))
                .order(tx_calls::dsl::tx_sequence_number.asc())
                .load::<StoredTxCalls>(conn)
        })
        .context("Failed reading tx move calls from PostgresDB")?;
        Ok(move_calls)
    }

    fn persist_move_calls(&self, move_calls: Vec<StoredMoveCall>) -> IndexerResult<()> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(move_calls::table)
                    .values(move_calls.clone())
                    .on_conflict_do_nothing()
                    .execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting move calls to PostgresDB")?;
        Ok(())
    }

    async fn calculate_move_call_metrics(
        &self,
        checkpoint: StoredCheckpoint,
    ) -> IndexerResult<Vec<StoredMoveCallMetrics>> {
        let epoch = checkpoint.epoch;
        let move_call_query_3d = build_move_call_metric_query(epoch, 3);
        let move_call_query_7d = build_move_call_metric_query(epoch, 7);
        let move_call_query_30d = build_move_call_metric_query(epoch, 30);

        let mut calculate_tasks = vec![];
        let blocking_cp_3d = self.blocking_cp.clone();
        calculate_tasks.push(tokio::task::spawn_blocking(move || {
            read_only_blocking!(&blocking_cp_3d, |conn| {
                diesel::sql_query(move_call_query_3d).get_results::<QueriedMoveMetrics>(conn)
            })
        }));
        let blocking_cp_7d = self.blocking_cp.clone();
        calculate_tasks.push(tokio::task::spawn_blocking(move || {
            read_only_blocking!(&blocking_cp_7d, |conn| {
                diesel::sql_query(move_call_query_7d).get_results::<QueriedMoveMetrics>(conn)
            })
        }));
        let blocking_cp_30d = self.blocking_cp.clone();
        calculate_tasks.push(tokio::task::spawn_blocking(move || {
            read_only_blocking!(&blocking_cp_30d, |conn| {
                diesel::sql_query(move_call_query_30d).get_results::<QueriedMoveMetrics>(conn)
            })
        }));
        let chained = futures::future::join_all(calculate_tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| {
                error!("Error joining move call calculation tasks: {:?}", e);
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .tap_err(|e| {
                error!("Error calculating move call metrics: {:?}", e);
            })?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        let move_call_metrics: Vec<StoredMoveCallMetrics> = chained
            .into_iter()
            .filter_map(|queried_move_metrics| {
                let package = ObjectID::from_bytes(queried_move_metrics.move_package.clone()).ok();
                let package_str = match package {
                    Some(p) => p.to_canonical_string(/* with_prefix */ true),
                    None => {
                        tracing::error!(
                            "Failed to parse move package ID: {:?}",
                            queried_move_metrics.move_package
                        );
                        return None;
                    }
                };
                Some(StoredMoveCallMetrics {
                    id: None,
                    checkpoint_sequence_number: checkpoint.sequence_number,
                    epoch: checkpoint.epoch,
                    day: queried_move_metrics.day,
                    move_package: package_str,
                    move_module: queried_move_metrics.move_module,
                    move_function: queried_move_metrics.move_function,
                    count: queried_move_metrics.count,
                })
            })
            .collect();
        Ok(move_call_metrics)
    }

    async fn persist_move_call_metrics(
        &self,
        move_call_metrics: Vec<StoredMoveCallMetrics>,
    ) -> IndexerResult<()> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(move_call_metrics::table)
                    .values(move_call_metrics.clone())
                    .on_conflict_do_nothing()
                    .execute(conn)
            },
            Duration::from_secs(60)
        )
        .context("Failed persisting move call metrics to PostgresDB")?;
        Ok(())
    }
}

fn construct_checkpoint_tx_count_query(start_checkpoint: i64, end_checkpoint: i64) -> String {
    format!(
        "With filtered_txns AS (
            SELECT 
                t.checkpoint_sequence_number,
                c.epoch,
                t.timestamp_ms,
                t.success_command_count
            FROM transactions t
            LEFT JOIN checkpoints c
            ON t.checkpoint_sequence_number = c.sequence_number
            WHERE t.checkpoint_sequence_number >= {} AND t.checkpoint_sequence_number <= {}
          )
          SELECT 
            checkpoint_sequence_number,
            epoch,
            MAX(timestamp_ms) AS timestamp_ms,
            COUNT(*) AS total_transaction_blocks,
            SUM(CASE WHEN success_command_count > 0 THEN 1 ELSE 0 END) AS total_successful_transaction_blocks,
            SUM(success_command_count) AS total_successful_transactions
          FROM filtered_txns
          GROUP BY checkpoint_sequence_number, epoch ORDER BY checkpoint_sequence_number;
        ", start_checkpoint, end_checkpoint
    )
}

fn construct_peak_tps_query(epoch: i64, offset: i64) -> String {
    format!(
        "WITH filtered_checkpoints AS (
            SELECT
              MAX(checkpoint_sequence_number) AS checkpoint_sequence_number,
              SUM(total_successful_transactions) AS total_successful_transactions,
              timestamp_ms
            FROM
              tx_count_metrics
              WHERE epoch > ({} - {})
            GROUP BY
              timestamp_ms
          ),
          tps_data AS (
            SELECT
              checkpoint_sequence_number,
              total_successful_transactions,
              timestamp_ms - LAG(timestamp_ms) OVER (ORDER BY timestamp_ms) AS time_diff
            FROM 
              filtered_checkpoints
          )
          SELECT 
            MAX(total_successful_transactions * 1000.0 / time_diff)::float8 as peak_tps
          FROM 
            tps_data
          WHERE 
            time_diff IS NOT NULL;
        ",
        epoch, offset
    )
}
