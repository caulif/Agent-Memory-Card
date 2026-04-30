mod build;
mod config;
mod fsutil;
mod scanner;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-kernel")]
#[command(about = "Project-centered Agent skills and rules workspace")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan existing rules and skills, then create/update .agent-kernel state.
    Import {
        /// Scan home-level skill folders such as ~/.agents/skills.
        #[arg(long)]
        scan_home: bool,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Show discovered skills and rule files.
    Scan {
        /// Scan home-level skill folders such as ~/.agents/skills.
        #[arg(long)]
        scan_home: bool,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Add a declared Mirror from a referenced Skill to an Agent target.
    Mirror {
        /// Skill reference id from .agent-kernel/skill-index.yml.
        #[arg(long)]
        skill: String,

        /// Agent target, such as codex or claude-code.
        #[arg(long)]
        agent: String,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Build or preview Agent artifacts.
    Build {
        /// Print planned changes without writing files.
        #[arg(long)]
        preview: bool,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Check declared mirrors and generated artifacts for drift.
    Status {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Sync mirrors and generated artifacts from declarative project config.
    Sync {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Launch the local Canvas workspace UI.
    Ui {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,

        /// Port to bind.
        #[arg(long, default_value_t = 4765)]
        port: u16,

        /// Do not open a browser.
        #[arg(long)]
        no_open: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Import { scan_home, project } => {
            let summary = scanner::import_project(&project, scan_home)?;
            println!("{}", summary.render());
        }
        Commands::Scan { scan_home, project } => {
            let report = scanner::scan_project(&project, scan_home)?;
            println!("{}", report.render());
        }
        Commands::Mirror {
            skill,
            agent,
            project,
        } => {
            config::add_mirror(&project, &skill, &agent)?;
            println!("Added mirror: {skill} -> {agent}");
            println!("Run `agent-kernel build --preview` to inspect changes.");
        }
        Commands::Build { preview, project } => {
            let report = build::build_project(&project, preview)?;
            println!("{}", report.render());
        }
        Commands::Status { project } => {
            let report = build::status_project(&project)?;
            println!("{}", report.render());
        }
        Commands::Sync { project } => {
            let report = build::sync_project(&project)?;
            println!("{}", report.render());
        }
        Commands::Ui {
            project,
            port,
            no_open,
        } => {
            ui::serve(project, port, !no_open).await?;
        }
    }

    Ok(())
}
