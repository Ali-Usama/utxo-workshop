//! Substrate Node Template CLI library.
#![warn(missing_docs)]

// use sc_service::Error;

mod chain_spec;
#[macro_use]
mod service;
mod cli;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
