use anyhow::{Context, Result, anyhow};
use clap::{Arg, Command, ArgMatches};
use drop_cli::{run_receive_files, run_send_files, Profile};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = build_cli().get_matches();
    
    match matches.subcommand() {
        Some(("send", sub_matches)) => {
            handle_send_command(sub_matches).await
        }
        Some(("receive", sub_matches)) => {
            handle_receive_command(sub_matches).await
        }
        _ => {
            eprintln!("❌ Invalid command. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

fn build_cli() -> Command {
    Command::new("drop-cli")
        .about("A CLI tool for sending and receiving files")
        .version("0.8.0")
        .author("@oluiscabral")
        .arg_required_else_help(true)
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
                        .default_value("drop-cli-sender")
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
                    Arg::new("output")
                        .help("Output directory for received files")
                        .required(true)
                        .value_parser(clap::value_parser!(PathBuf))
                )
                .arg(
                    Arg::new("ticket")
                        .help("Transfer ticket")
                        .required(true)
                )
                .arg(
                    Arg::new("confirmation")
                        .help("Confirmation code")
                        .required(true)
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .help("Your display name")
                        .default_value("drop-cli-receiver")
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
}

async fn handle_send_command(matches: &ArgMatches) -> Result<()> {
    let files: Vec<PathBuf> = matches.get_many::<PathBuf>("files")
        .unwrap()
        .cloned()
        .collect();
    
    let profile = build_profile(matches)?;
    
    println!("📤 Preparing to send {} file(s)...", files.len());
    for file in &files {
        println!("   📄 {}", file.display());
    }
    
    if let Some(name) = profile.name.strip_prefix("drop-cli-") {
        println!("👤 Sender name: {}", name);
    } else {
        println!("👤 Sender name: {}", profile.name);
    }
    
    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }
    
    let file_strings: Vec<String> = files.into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    
    run_send_files(file_strings, profile).await
}

async fn handle_receive_command(matches: &ArgMatches) -> Result<()> {
    let output_dir = matches.get_one::<PathBuf>("output").unwrap();
    let ticket = matches.get_one::<String>("ticket").unwrap();
    let confirmation = matches.get_one::<String>("confirmation").unwrap();
    
    let profile = build_profile(matches)?;
    
    println!("📥 Preparing to receive files...");
    println!("📁 Output directory: {}", output_dir.display());
    println!("🎫 Ticket: {}", ticket);
    println!("🔑 Confirmation: {}", confirmation);
    
    if let Some(name) = profile.name.strip_prefix("drop-cli-") {
        println!("👤 Receiver name: {}", name);
    } else {
        println!("👤 Receiver name: {}", profile.name);
    }
    
    if profile.avatar_b64.is_some() {
        println!("🖼️  Avatar: Set");
    }
    
    run_receive_files(
        output_dir.to_string_lossy().to_string(),
        ticket.clone(),
        confirmation.clone(),
        profile,
    ).await
}

fn build_profile(matches: &ArgMatches) -> Result<Profile> {
    let name = matches.get_one::<String>("name").unwrap().clone();
    let mut profile = Profile::new(name, None);
    
    // Handle avatar from file
    if let Some(avatar_path) = matches.get_one::<PathBuf>("avatar") {
        if !avatar_path.exists() {
            return Err(anyhow!("Avatar file does not exist: {}", avatar_path.display()));
        }
        
        profile = profile.with_avatar_file(&avatar_path.to_string_lossy())
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