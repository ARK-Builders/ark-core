use anyhow::{Context, Result, anyhow};
use clap::{Arg, ArgMatches, Command};
use drop_cli::{
    Profile, clear_default_receive_dir, get_default_receive_dir,
    run_ready_to_receive, run_receive_files, run_send_files, run_send_files_to,
    set_default_receive_dir, suggested_default_receive_dir,
};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

mod handshake;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = build_cli().get_matches();

    match matches.subcommand() {
        Some(("send", sub_matches)) => handle_send_command(sub_matches).await,
        Some(("receive", sub_matches)) => {
            handle_receive_command(sub_matches).await
        }
        Some(("config", sub_matches)) => {
            handle_config_command(sub_matches).await
        }
        _ => {
            eprintln!(
                "Error: Invalid command. Use --help for usage information."
            );
            std::process::exit(1);
        }
    }
}

fn build_cli() -> Command {
    Command::new("drop-cli")
        .about("A Drop CLI tool for sending and receiving files")
        .version("1.0.0")
        .author("oluiscabral@ark-builders.dev")
        .arg_required_else_help(true)
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Enable verbose logging")
                .action(clap::ArgAction::SetTrue)
                .global(true)
        )
        .subcommand(
            Command::new("send")
                .about("Send files - generates QR code or accepts receiver's credentials")
                .arg(
                    Arg::new("files")
                        .help("Files to send")
                        .required(true)
                        .num_args(1..)
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("Your display name")
                        .default_value("arkdrop-sender")
                )
                .arg(
                    Arg::new("avatar")
                        .long("avatar")
                        .short('a')
                        .help("Path to avatar image file")
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("avatar-b64")
                        .long("avatar-b64")
                        .help("Base64 encoded avatar image (alternative to --avatar)")
                        .conflicts_with("avatar")
                )
        )
        .subcommand(
            Command::new("receive")
                .about("Receive files - generates QR code or accepts sender's credentials")
                .arg(
                    Arg::new("output")
                        .help("Output directory for received files (optional if default is set)")
                        .long("output")
                        .short('o')
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("save-dir")
                        .long("save-dir")
                        .help("Save the specified output directory as default for future use")
                        .action(clap::ArgAction::SetTrue)
                        .requires("output")
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("Your display name")
                        .default_value("arkdrop-receiver")
                )
                .arg(
                    Arg::new("avatar")
                        .long("avatar")
                        .short('a')
                        .help("Path to avatar image file")
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("avatar-b64")
                        .long("avatar-b64")
                        .help("Base64 encoded avatar image (alternative to --avatar)")
                        .conflicts_with("avatar")
                )
        )
        .subcommand(
            Command::new("config")
                .about("Manage CLI configuration")
                .subcommand(
                    Command::new("show")
                        .about("Show current configuration")
                )
                .subcommand(
                    Command::new("set-receive-dir")
                        .about("Set default receive directory")
                        .arg(
                            Arg::new("directory")
                                .help("Directory path to set as default")
                                .required(true)
                                .value_parser(clap::value_parser!(PathBuf))
                        )
                )
                .subcommand(
                    Command::new("clear-receive-dir")
                        .about("Clear default receive directory")
                )
        )
}

async fn handle_send_command(matches: &ArgMatches) -> Result<()> {
    let files: Vec<PathBuf> = matches
        .get_many::<PathBuf>("files")
        .unwrap()
        .cloned()
        .collect();

    let verbose: bool = matches.get_flag("verbose");
    let profile = build_profile(matches)?;

    // Display info
    println!("\nPreparing to send {} file(s):", files.len());
    for file in &files {
        println!("  - {}", file.display());
    }
    println!("Sender: {}", profile.name);
    if profile.avatar_b64.is_some() {
        println!("Avatar: Set");
    }
    println!();

    let file_strings: Vec<String> = files
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Start QR flow and listen for 'c' press concurrently
    let qr_task = tokio::spawn({
        let file_strings = file_strings.clone();
        let profile = profile.clone();
        async move { run_send_files(file_strings, profile, verbose).await }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    tokio::select! {
        result = qr_task => {
            // QR flow completed (peer connected and files sent)
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(anyhow!("Task error: {}", e)),
            }
        }
        _ = reader.read_line(&mut input) => {
            // User pressed something - check if it's 'c'
            if input.trim().eq_ignore_ascii_case("c") {
                println!("\nSwitching to manual credential entry...\n");
                // Get peer's credentials
                let (ticket, confirmation) = handshake::prompt_for_credentials().await?;
                // Send files to peer
                run_send_files_to(
                    file_strings,
                    ticket,
                    confirmation.to_string(),
                    profile,
                    verbose,
                )
                .await
            } else {
                // Ignore other input, keep waiting
                Ok(())
            }
        }
    }
}

async fn handle_receive_command(matches: &ArgMatches) -> Result<()> {
    let verbose = matches.get_flag("verbose");
    let save_dir = matches.get_flag("save-dir");
    let profile = build_profile(matches)?;

    let output_dir = matches
        .get_one::<PathBuf>("output")
        .map(|p| p.to_string_lossy().to_string());

    // Display info
    println!("\nPreparing to receive files...");
    if let Some(ref dir) = output_dir {
        println!("Output directory: {}", dir);
    } else if let Some(default_dir) = get_default_receive_dir()? {
        println!("Using default directory: {}", default_dir);
    } else {
        let fallback = suggested_default_receive_dir();
        println!("Using default directory: {}", fallback.display());
    }
    println!("Receiver: {}", profile.name);
    if profile.avatar_b64.is_some() {
        println!("Avatar: Set");
    }
    println!();

    // Start QR flow and listen for 'c' press concurrently
    let qr_task = tokio::spawn({
        let output_dir = output_dir.clone();
        let profile = profile.clone();
        async move {
            run_ready_to_receive(output_dir, profile, verbose, save_dir).await
        }
    });

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut input = String::new();

    tokio::select! {
        result = qr_task => {
            // QR flow completed (peer connected and files received)
            match result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(anyhow!("Task error: {}", e)),
            }
        }
        _ = reader.read_line(&mut input) => {
            // User pressed something - check if it's 'c'
            if input.trim().eq_ignore_ascii_case("c") {
                println!("\nSwitching to manual credential entry...\n");
                // Get peer's credentials
                let (ticket, confirmation) = handshake::prompt_for_credentials().await?;
                // Receive files from peer
                run_receive_files(
                    output_dir,
                    ticket,
                    confirmation.to_string(),
                    profile,
                    verbose,
                    save_dir,
                )
                .await
            } else {
                // Ignore other input, keep waiting
                Ok(())
            }
        }
    }
}

async fn handle_config_command(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("show", _)) => match get_default_receive_dir()? {
            Some(dir) => {
                println!("Default receive directory: {}", dir);
            }
            None => {
                println!("No default receive directory set");
            }
        },
        Some(("set-receive-dir", sub_matches)) => {
            let directory = sub_matches
                .get_one::<PathBuf>("directory")
                .unwrap();
            let dir_str = directory.to_string_lossy().to_string();

            // Validate directory exists or can be created
            if !directory.exists() {
                match std::fs::create_dir_all(directory) {
                    Ok(_) => println!("Created directory: {}", dir_str),
                    Err(e) => {
                        return Err(anyhow!(
                            "Failed to create directory '{}': {}",
                            dir_str,
                            e
                        ));
                    }
                }
            }

            set_default_receive_dir(dir_str.clone())?;
            println!("Set default receive directory to: {}", dir_str);
        }
        Some(("clear-receive-dir", _)) => {
            clear_default_receive_dir()?;
            println!("Cleared default receive directory");
        }
        _ => {
            eprintln!(
                "Error: Invalid config command. Use --help for usage information."
            );
            std::process::exit(1);
        }
    }
    Ok(())
}

fn build_profile(matches: &ArgMatches) -> Result<Profile> {
    let name = matches.get_one::<String>("name").unwrap().clone();
    let mut profile = Profile::new(name, None);

    // Handle avatar from file
    if let Some(avatar_path) = matches.get_one::<PathBuf>("avatar") {
        if !avatar_path.exists() {
            return Err(anyhow!(
                "Avatar file does not exist: {}",
                avatar_path.display()
            ));
        }

        profile = profile
            .with_avatar_file(&avatar_path.to_string_lossy())
            .with_context(|| "Failed to load avatar file")?;
    }

    // Handle avatar from base64 string
    if let Some(avatar_b64) = matches.get_one::<String>("avatar-b64") {
        profile = profile.with_avatar_b64(avatar_b64.clone());
    }

    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("test-user".to_string(), None);
        assert_eq!(profile.name, "test-user");
        assert!(profile.avatar_b64.is_none());
    }

    #[test]
    fn test_profile_with_avatar() {
        let profile = Profile::new("test-user".to_string(), None)
            .with_avatar_b64("dGVzdA==".to_string());
        assert_eq!(profile.name, "test-user");
        assert_eq!(profile.avatar_b64, Some("dGVzdA==".to_string()));
    }
}
