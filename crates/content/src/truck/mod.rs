//! A truck as plain data: its tuning (`TruckConfig`, with its `EngineConfig`), the
//! player's setup laid over it (`TruckSetup`), and what it looks like (`TruckData`).

mod config;
mod data;
mod engine_config;

pub use config::{FRONT_WHEELS, TruckConfig, TruckSetup};
pub use data::*;
pub use engine_config::EngineConfig;
