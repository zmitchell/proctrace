pub mod cli;
pub mod ingest;
pub mod models;
pub mod record;
pub mod render;
pub mod utils;
pub mod writers;

#[cfg(target_os = "linux")]
const SCRIPT: &'static str = include_str!("../assets/proctrace.bt");
