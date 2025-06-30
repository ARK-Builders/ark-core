use anyhow::Result;
use drop_cli::{run_receive_files, run_send_files};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let arg_slices = arg_refs.as_slice();

    if arg_slices[0] == "send" {
        run_send_files(
            arg_slices[1..]
                .to_vec()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        )
        .await?;
        return Ok(());
    } else if arg_slices[0] == "receive" {
        run_receive_files(
            arg_slices[1..]
                .to_vec()
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
        )
        .await?;
        return Ok(());
    } else {
        on_invalid(args);
    }
    Ok(())
}

fn on_invalid(args: Vec<String>) {
    println!("Couldn't parse command line arguments: {args:?}");
    println!("Usage:");
    println!("    # to send:");
    println!("    cargo run send [SOURCE...]");
    println!("    # this will print a ticket and a confirmation code.");
    println!();
    println!("    # to receive:");
    println!("    cargo run receive [OUTPUT] [TICKET] [CONFIRMATION]");
}
