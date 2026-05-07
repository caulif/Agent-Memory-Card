use std::path::PathBuf;

use clap::Parser;

use crate::cli::{Cli, Commands, ObserveCommands};

#[test]
fn cli_accepts_observe_replay_command() {
    let cli = Cli::parse_from([
        "agent-kernel",
        "observe",
        "replay",
        "--home",
        ".",
        "--target",
        "codex",
        "--dry-run",
        "--include-unknown-project",
    ]);

    match cli.command {
        Commands::Observe {
            command:
                ObserveCommands::Replay {
                    home,
                    targets,
                    dry_run,
                    include_unknown_project,
                    ..
                },
        } => {
            assert_eq!(home, Some(PathBuf::from(".")));
            assert_eq!(targets, vec!["codex"]);
            assert!(dry_run);
            assert!(include_unknown_project);
        }
        _ => panic!("expected observe replay command"),
    }
}
