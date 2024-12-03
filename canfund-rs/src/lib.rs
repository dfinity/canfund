#![doc = include_str!("../../README.md")]

pub mod api;
pub mod errors;
pub mod manager;
pub mod operations;
pub mod types;
pub mod utils;

pub use manager::FundManager;
