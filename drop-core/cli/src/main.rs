use anyhow::{Context, Result, anyhow};
use arkdrop::{
    Profile, clear_default_receive_dir, get_default_receive_dir,
    run_receive_files, run_send_files, set_default_receive_dir,
    suggested_default_receive_dir,
};
use clap::{Arg, ArgMatches, Command};
use std::path::PathBuf;

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
            eprintln!("❌ Invalid command. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

fn build_cli() -> Command {
    Command::new("arkdrop")
        .about("A ARK Drop tool for sending and receiving files")
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
                .about("Send files to another user")
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
                .about("Receive files from another user")
                .arg(
                    Arg::new("ticket")
                        .help("Transfer ticket")
                        .required(true)
                        .index(1)
                )
                .arg(
                    Arg::new("confirmation")
                        .help("Confirmation code")
                        .required(true)
                        .index(2)
                )
                .arg(
                    Arg::new("output")
                        .help("Output directory for received files (optional if default is set)")
                        .long("output")
                        .short('o')
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("save-output")
                        .long("save-output")
                        .short('u')
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
                .about("Manage ARK Drop configuration")
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

    println!("📤 Preparing to send {} file(s)...", files.len());
    for file in &files {
        println!("   📄 {}", file.display());
    }

    if let Some(name) = profile.name.strip_prefix("arkdrop-") {
        println!("👤 Sender name: {name}");
    } else {
        println!("👤 Sender name: {}", profile.name);
    }

    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }

    let file_strings: Vec<String> = files
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    run_send_files(file_strings, profile, verbose).await
}

async fn handle_receive_command(matches: &ArgMatches) -> Result<()> {
    let output_dir = matches
        .get_one::<PathBuf>("output")
        .map(|p| p.to_string_lossy().to_string());
    let ticket = matches.get_one::<String>("ticket").unwrap();
    let confirmation = matches.get_one::<String>("confirmation").unwrap();
    let verbose = matches.get_flag("verbose");
    let save_dir = matches.get_flag("save-output");

    let profile = build_profile(matches)?;

    println!("📥 Preparing to receive files...");

    if let Some(ref dir) = output_dir {
        println!("📁 Output directory: {dir}");
    } else if let Some(default_dir) = get_default_receive_dir()? {
        println!("📁 Using default directory: {default_dir}");
    } else {
        let fallback = suggested_default_receive_dir();
        println!("📁 Using default directory: {}", fallback.display());
    }

    println!("🎫 Ticket: {ticket}");
    println!("🔑 Confirmation: {confirmation}");

    if let Some(name) = profile.name.strip_prefix("arkdrop-") {
        println!("👤 Receiver name: {name}");
    } else {
        println!("👤 Receiver name: {}", profile.name);
    }

    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }

    run_receive_files(
        output_dir,
        ticket.clone(),
        confirmation.clone(),
        profile,
        verbose,
        save_dir,
    )
    .await
}

async fn handle_config_command(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("show", _)) => match get_default_receive_dir()? {
            Some(dir) => {
                println!("📁 Default receive directory: {dir}");
            }
            None => {
                println!("📁 No default receive directory set");
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
                    Ok(_) => println!("📁 Created directory: {dir_str}"),
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
            println!("✅ Set default receive directory to: {dir_str}");
        }
        Some(("clear-receive-dir", _)) => {
            clear_default_receive_dir()?;
            println!("✅ Cleared default receive directory");
        }
        _ => {
            eprintln!(
                "❌ Invalid config command. Use --help for usage information."
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
