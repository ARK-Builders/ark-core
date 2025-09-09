use anyhow::{Context, Result, anyhow};
use arkdrop_cli::{run_receive_files, run_send_files};
use arkdrop_common::{
    Profile, clear_default_out_dir, get_default_out_dir, set_default_out_dir,
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
    Command::new("arkdrop-cli")
        .about("ARK Drop CLI tool for sending and receiving files")
        .version("1.0.0")
        .author("ARK Builders")
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
                        .short('b')
                        .help("Base64 encoded avatar image (alternative to --avatar)")
                        .conflicts_with("avatar")
                )
        )
        .subcommand(
            Command::new("config")
                .about("Manage ARK Drop CLI configuration")
                .subcommand(
                    Command::new("show")
                        .about("Show current configuration")
                )
                .subcommand(
                    Command::new("set-output")
                        .about("Set default receive output directory")
                        .arg(
                            Arg::new("output")
                                .help("Output directory path to set as default")
                                .required(true)
                                .value_parser(clap::value_parser!(PathBuf))
                        )
                )
                .subcommand(
                    Command::new("clear-output")
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

    println!("👤 Sender name: {}", profile.name);

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
    let out_dir = matches
        .get_one::<String>("output")
        .map(|p| PathBuf::from(p));
    let ticket = matches.get_one::<String>("ticket").unwrap();
    let confirmation = matches.get_one::<String>("confirmation").unwrap();
    let verbose = matches.get_flag("verbose");
    let save_output = matches.get_flag("save-output");

    let profile = build_profile(matches)?;

    println!("📥 Preparing to receive files...");

    let out_dir = match out_dir {
        Some(o) => o,
        None => get_default_out_dir(),
    };

    println!("👤 Receiver name: {}", profile.name);

    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }

    run_receive_files(
        out_dir,
        ticket.clone(),
        confirmation.clone(),
        profile,
        verbose,
        save_output,
    )
    .await?;

    Ok(())
}

async fn handle_config_command(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("show", _)) => {
            let out_dir = get_default_out_dir();
            println!(
                "📁 Default receive output directory: {}",
                out_dir.display()
            );
        }

        Some(("set-output", sub_matches)) => {
            let out_dir = sub_matches.get_one::<PathBuf>("output").unwrap();
            let out_dir_str = out_dir.display();

            // Validate output exists or can be created
            if !out_dir.exists() {
                match std::fs::create_dir_all(out_dir) {
                    Ok(_) => {
                        println!("📁 Created output directory: {out_dir_str}")
                    }
                    Err(e) => {
                        return Err(anyhow!(
                            "Failed to create output directory '{}': {}",
                            out_dir_str,
                            e
                        ));
                    }
                }
            }

            set_default_out_dir(out_dir.clone())?;
            println!(
                "✅ Set default receive output directory to: {out_dir_str}"
            );
        }

        Some(("clear-output", _)) => {
            clear_default_out_dir()?;
            println!("✅ Cleared default receive output directory");
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
