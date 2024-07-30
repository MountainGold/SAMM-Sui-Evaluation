// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{error::Error as SuiGraphQLError, types::big_int::BigInt};
use async_graphql::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};
use sui_json_rpc::name_service::NameServiceConfig;

use crate::functional_group::FunctionalGroup;

// TODO: calculate proper cost limits
const MAX_QUERY_DEPTH: u32 = 20;
const MAX_QUERY_NODES: u32 = 200;
const MAX_QUERY_PAYLOAD_SIZE: u32 = 5_000;
const MAX_DB_QUERY_COST: u64 = 20_000; // Max DB query cost (normally f64) truncated

const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 40_000;

const DEFAULT_IDE_TITLE: &str = "Sui GraphQL IDE";

/// Configuration on connections for the RPC, passed in as command-line arguments.
#[derive(Serialize, Clone, Deserialize, Debug, Eq, PartialEq)]
pub struct ConnectionConfig {
    pub(crate) port: u16,
    pub(crate) host: String,
    pub(crate) db_url: String,
    pub(crate) prom_url: String,
    pub(crate) prom_port: u16,
}

/// Configuration on features supported by the RPC, passed in a TOML-based file.
#[derive(Serialize, Clone, Deserialize, Debug, Eq, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ServiceConfig {
    #[serde(default)]
    pub(crate) limits: Limits,

    #[serde(default)]
    pub(crate) disabled_features: BTreeSet<FunctionalGroup>,

    #[serde(default)]
    pub(crate) experiments: Experiments,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Copy)]
#[serde(rename_all = "kebab-case")]
pub struct Limits {
    #[serde(default)]
    pub(crate) max_query_depth: u32,
    #[serde(default)]
    pub(crate) max_query_nodes: u32,
    #[serde(default)]
    pub(crate) max_query_payload_size: u32,
    #[serde(default)]
    pub(crate) max_db_query_cost: u64,
    #[serde(default)]
    pub(crate) request_timeout_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Ide {
    #[serde(default)]
    pub(crate) ide_title: String,
}

impl Default for Ide {
    fn default() -> Self {
        Self {
            ide_title: DEFAULT_IDE_TITLE.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Experiments {
    // Add experimental flags here, to provide access to them through-out the GraphQL
    // implementation.
    #[cfg(test)]
    test_flag: bool,
}

impl ConnectionConfig {
    pub fn new(
        port: Option<u16>,
        host: Option<String>,
        db_url: Option<String>,
        prom_url: Option<String>,
        prom_port: Option<u16>,
    ) -> Self {
        let default = Self::default();
        Self {
            port: port.unwrap_or(default.port),
            host: host.unwrap_or(default.host),
            db_url: db_url.unwrap_or(default.db_url),
            prom_url: prom_url.unwrap_or(default.prom_url),
            prom_port: prom_port.unwrap_or(default.prom_port),
        }
    }

    pub fn ci_integration_test_cfg() -> Self {
        Self {
            db_url: "postgres://postgres:postgrespw@localhost:5432/sui_indexer_v2".to_string(),
            ..Default::default()
        }
    }

    pub fn db_url(&self) -> String {
        self.db_url.clone()
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl ServiceConfig {
    pub fn read(contents: &str) -> Result<Self, toml::de::Error> {
        toml::de::from_str::<Self>(contents)
    }
}

#[Object]
impl ServiceConfig {
    /// Check whether `feature` is enabled on this GraphQL service.
    async fn is_enabled(&self, feature: FunctionalGroup) -> bool {
        !self.disabled_features.contains(&feature)
    }

    /// List of all features that are enabled on this GraphQL service.
    async fn enabled_features(&self) -> Vec<FunctionalGroup> {
        FunctionalGroup::all()
            .iter()
            .filter(|g| !self.disabled_features.contains(g))
            .copied()
            .collect()
    }

    /// The maximum depth a GraphQL query can be to be accepted by this service.
    async fn max_query_depth(&self) -> u32 {
        self.limits.max_query_depth
    }

    /// The maximum number of nodes (field names) the service will accept in a single query.
    async fn max_query_nodes(&self) -> u32 {
        self.limits.max_query_nodes
    }

    /// Maximum estimated cost of a database query used to serve a GraphQL request.  This is
    /// measured in the same units that the database uses in EXPLAIN queries.
    async fn max_db_query_cost(&self) -> BigInt {
        BigInt::from(self.limits.max_db_query_cost)
    }

    /// Maximum time in milliseconds that will be spent to serve one request.
    async fn request_timeout_ms(&self) -> BigInt {
        BigInt::from(self.limits.request_timeout_ms)
    }

    /// Maximum length of a query payload string.
    async fn max_query_payload_size(&self) -> u32 {
        self.limits.max_query_payload_size
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            port: 8000,
            host: "127.0.0.1".to_string(),
            db_url: "postgres://postgres:postgrespw@localhost:5432/sui_indexer_v2".to_string(),
            prom_url: "0.0.0.0".to_string(),
            prom_port: 9184,
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_query_depth: MAX_QUERY_DEPTH,
            max_query_nodes: MAX_QUERY_NODES,
            max_query_payload_size: MAX_QUERY_PAYLOAD_SIZE,
            max_db_query_cost: MAX_DB_QUERY_COST,
            request_timeout_ms: DEFAULT_REQUEST_TIMEOUT_MS,
        }
    }
}

#[derive(Serialize, Clone, Deserialize, Debug, Eq, PartialEq)]
pub struct InternalFeatureConfig {
    #[serde(default)]
    pub(crate) query_limits_checker: bool,
    #[serde(default)]
    pub(crate) feature_gate: bool,
    #[serde(default)]
    pub(crate) logger: bool,
    #[serde(default)]
    pub(crate) query_timeout: bool,
    #[serde(default)]
    pub(crate) metrics: bool,
}

impl Default for InternalFeatureConfig {
    fn default() -> Self {
        Self {
            query_limits_checker: true,
            feature_gate: true,
            logger: true,
            query_timeout: true,
            metrics: true,
        }
    }
}

#[derive(Serialize, Clone, Deserialize, Debug, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub service: ServiceConfig,
    #[serde(default)]
    pub connection: ConnectionConfig,
    #[serde(default)]
    pub internal_features: InternalFeatureConfig,
    #[serde(default)]
    pub name_service: NameServiceConfig,
    #[serde(default)]
    pub ide: Ide,
}

impl ServerConfig {
    pub fn from_yaml(path: &str) -> Result<Self, SuiGraphQLError> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            SuiGraphQLError::Internal(format!(
                "Failed to read service cfg yaml file at {}, err: {}",
                path, e
            ))
        })?;
        serde_yaml::from_str::<Self>(&contents).map_err(|e| {
            SuiGraphQLError::Internal(format!(
                "Failed to deserialize service cfg from yaml: {}",
                e
            ))
        })
    }

    pub fn to_yaml(&self) -> Result<String, SuiGraphQLError> {
        serde_yaml::to_string(&self).map_err(|e| {
            SuiGraphQLError::Internal(format!("Failed to create yaml from cfg: {}", e))
        })
    }

    pub fn to_yaml_file(&self, path: PathBuf) -> Result<(), SuiGraphQLError> {
        let config = self.to_yaml()?;
        std::fs::write(path, config).map_err(|e| {
            SuiGraphQLError::Internal(format!("Failed to create yaml from cfg: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_empty_service_config() {
        let actual = ServiceConfig::read("").unwrap();
        let expect = ServiceConfig::default();
        assert_eq!(actual, expect);
    }

    #[test]
    fn test_read_limits_in_service_config() {
        let actual = ServiceConfig::read(
            r#" [limits]
                max-query-depth = 100
                max-query-nodes = 300
                max-query-payload-size = 2000
                max-db-query-cost = 50
                request-timeout-ms = 27000
            "#,
        )
        .unwrap();

        let expect = ServiceConfig {
            limits: Limits {
                max_query_depth: 100,
                max_query_nodes: 300,
                max_query_payload_size: 2000,
                max_db_query_cost: 50,
                request_timeout_ms: 27_000,
            },
            ..Default::default()
        };

        assert_eq!(actual, expect)
    }

    #[test]
    fn test_read_enabled_features_in_service_config() {
        let actual = ServiceConfig::read(
            r#" disabled-features = [
                  "coins",
                  "name-service",
                ]
            "#,
        )
        .unwrap();

        use FunctionalGroup as G;
        let expect = ServiceConfig {
            limits: Limits::default(),
            disabled_features: BTreeSet::from([G::Coins, G::NameService]),
            experiments: Experiments::default(),
        };

        assert_eq!(actual, expect)
    }

    #[test]
    fn test_read_experiments_in_service_config() {
        let actual = ServiceConfig::read(
            r#" [experiments]
                test-flag = true
            "#,
        )
        .unwrap();

        let expect = ServiceConfig {
            experiments: Experiments { test_flag: true },
            ..Default::default()
        };

        assert_eq!(actual, expect)
    }

    #[test]
    fn test_read_everything_in_service_config() {
        let actual = ServiceConfig::read(
            r#" disabled-features = ["analytics"]

                [limits]
                max-query-depth = 42
                max-query-nodes = 320
                max-query-payload-size = 200
                max-db-query-cost = 20
                request-timeout-ms = 30000

                [experiments]
                test-flag = true
            "#,
        )
        .unwrap();

        let expect = ServiceConfig {
            limits: Limits {
                max_query_depth: 42,
                max_query_nodes: 320,
                max_query_payload_size: 200,
                max_db_query_cost: 20,
                request_timeout_ms: 30_000,
            },
            disabled_features: BTreeSet::from([FunctionalGroup::Analytics]),
            experiments: Experiments { test_flag: true },
        };

        assert_eq!(actual, expect);
    }
}
