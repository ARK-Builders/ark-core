use clap::Subcommand;

mod receive;
mod send;

/// Available commands for the `drop` subcommand
#[derive(Subcommand, Debug)]
pub enum Drop {
    Send(send::Send),
    Receive(receive::Receive),
}
