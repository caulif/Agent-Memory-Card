mod build;
mod catalog;
mod config;
mod draft;
mod extract;
mod fsutil;
mod provider;
mod review;
mod rule_test;
mod scanner;
mod skilllet;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-kernel")]
#[command(about = "Project-centered Agent skills and rules workspace")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_exposes_cargo_package_version() {
        use clap::CommandFactory;

        let command = Cli::command();
        let version = command.render_long_version().to_string().trim().to_string();

        assert_eq!(version, "agent-kernel 0.1.0");
    }

    #[test]
    fn cli_accepts_import_artifacts_flag() {
        let cli = Cli::parse_from(["agent-kernel", "import", "--artifacts"]);

        match cli.command {
            Commands::Import { artifacts, .. } => assert!(artifacts),
            _ => panic!("expected import command"),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Scan existing rules and skills, then create/update .agent-kernel state.
    Import {
        /// Import manual changes from generated Agent artifacts into Draft Inbox.
        #[arg(long)]
        artifacts: bool,

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

    /// Manage lightweight project Skilllets.
    Skilllet {
        #[command(subcommand)]
        command: SkillletCommands,
    },

    /// Manage configured Agent targets.
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },

    /// Manage local Draft Inbox items before they become Skilllets.
    Draft {
        #[command(subcommand)]
        command: DraftCommands,
    },

    /// Extract local heuristic Draft Inbox candidates from text or a file.
    Extract {
        /// Inline text to extract from.
        #[arg(long, conflicts_with = "file")]
        text: Option<String>,

        /// File to extract from.
        #[arg(long, conflicts_with = "text")]
        file: Option<PathBuf>,

        /// Agent target. Repeat for multiple agents. Empty means project-level candidate.
        #[arg(long = "target")]
        targets: Vec<String>,

        /// Extraction provider. v0.8 supports local; other providers are config scaffolding.
        #[arg(long)]
        provider: Option<String>,

        /// Show extracted candidates without writing Draft Inbox files.
        #[arg(long)]
        dry_run: bool,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Manage Hybrid provider configuration.
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },

    /// Manage local Skilllet catalog packages.
    Catalog {
        #[command(subcommand)]
        command: CatalogCommands,
    },

    /// Run local Rule CI assertions against generated Agent artifacts.
    TestRules {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Review pending drafts, mirror status, Rule CI, and build preview.
    Review {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Approve a Draft Inbox item before rendering the review. Repeatable.
        #[arg(long = "approve-draft")]
        approve_drafts: Vec<String>,

        /// Reject a Draft Inbox item before rendering the review. Repeatable.
        #[arg(long = "reject-draft")]
        reject_drafts: Vec<String>,

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

#[derive(Subcommand)]
enum SkillletCommands {
    /// Add an owned project Skilllet and include it in project.yml.
    Add {
        /// Stable id, such as project:use-axios.
        #[arg(long)]
        id: String,

        /// Human-readable title.
        #[arg(long)]
        title: String,

        /// Skilllet body text.
        #[arg(long)]
        body: String,

        /// Optional kind: preference, constraint, procedure, fact, episode.
        #[arg(long, default_value = "preference")]
        kind: String,

        /// Optional scope: project, global, directory, agent-specific.
        #[arg(long, default_value = "project")]
        scope: String,

        /// Agent target. Repeat for multiple agents. Empty means all enabled instruction agents.
        #[arg(long = "target")]
        targets: Vec<String>,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List owned project Skilllets.
    List {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Assign an owned Skilllet to one or more Agent targets.
    Targets {
        /// Skilllet id, such as project:use-axios.
        #[arg(long)]
        id: String,

        /// Agent target. Repeat for multiple agents. Empty means all enabled instruction agents.
        #[arg(long = "target")]
        targets: Vec<String>,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Show a Skilllet by Agent target matrix.
    Matrix {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// List configured Agent targets.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Enable an Agent target.
    Enable {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Disable an Agent target.
    Disable {
        #[arg(long)]
        agent: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum DraftCommands {
    /// Add a Draft Inbox item.
    Add {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: String,
        #[arg(long, default_value = "preference")]
        kind: String,
        #[arg(long, default_value = "project")]
        scope: String,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long, default_value = "manual")]
        evidence: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List Draft Inbox items.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Approve a Draft Inbox item into an owned Skilllet.
    Approve {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Reject and remove a Draft Inbox item.
    Reject {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum ProviderCommands {
    /// Write default local-first provider config.
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Show provider config.
    Show {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
enum CatalogCommands {
    /// Write the default local catalog to .agent-kernel/catalog.yml.
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List local catalog packages.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Validate local catalog package metadata.
    Validate {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Install a catalog package as an owned Skilllet.
    Install {
        #[arg(long)]
        id: String,

        /// Agent target. Repeat for multiple agents. Empty means all enabled instruction agents.
        #[arg(long = "target")]
        targets: Vec<String>,

        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Import {
            artifacts,
            scan_home,
            project,
        } => {
            if artifacts {
                let report = build::import_artifact_drifts(&project)?;
                println!("{}", report.render());
                if scan_home {
                    let summary = scanner::import_project(&project, scan_home)?;
                    println!("{}", summary.render());
                }
            } else {
                let summary = scanner::import_project(&project, scan_home)?;
                println!("{}", summary.render());
            }
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
        Commands::Skilllet { command } => match command {
            SkillletCommands::Add {
                id,
                title,
                body,
                kind,
                scope,
                targets,
                project,
            } => {
                skilllet::add_skilllet(&project, &id, &title, &body, &kind, &scope, targets)?;
                println!("Added skilllet `{id}`");
                println!("Run `agent-kernel build --preview` to inspect generated instructions.");
            }
            SkillletCommands::List { project } => {
                let records = skilllet::load_skilllets(&project)?;
                if records.is_empty() {
                    println!("No skilllets found.");
                } else {
                    for record in records {
                        println!("- {}: {}", record.id, record.title);
                    }
                }
            }
            SkillletCommands::Targets {
                id,
                targets,
                project,
            } => {
                skilllet::set_skilllet_targets(&project, &id, targets)?;
                println!("Updated targets for skilllet `{id}`");
            }
            SkillletCommands::Matrix { project } => {
                let matrix = skilllet::skilllet_target_matrix(&project)?;
                println!("{}", matrix.render());
            }
        },
        Commands::Agent { command } => match command {
            AgentCommands::List { project } => {
                let config = config::load_or_default_project_config(&project)?;
                for (name, agent) in config.agents {
                    println!(
                        "- {} [{}]",
                        name,
                        if agent.enabled { "enabled" } else { "disabled" }
                    );
                }
            }
            AgentCommands::Enable { agent, project } => {
                config::set_agent_enabled(&project, &agent, true)?;
                println!("Enabled agent `{agent}`");
            }
            AgentCommands::Disable { agent, project } => {
                config::set_agent_enabled(&project, &agent, false)?;
                println!("Disabled agent `{agent}`");
            }
        },
        Commands::Draft { command } => match command {
            DraftCommands::Add {
                id,
                title,
                body,
                kind,
                scope,
                targets,
                evidence,
                project,
            } => {
                draft::add_draft(
                    &project,
                    draft::NewDraft {
                        id: id.clone(),
                        title,
                        body,
                        kind,
                        scope,
                        targets,
                        evidence,
                    },
                )?;
                println!("Added draft `{id}`");
            }
            DraftCommands::List { project } => {
                let records = draft::load_drafts(&project)?;
                if records.is_empty() {
                    println!("No drafts found.");
                } else {
                    for record in records {
                        println!("- {}: {} [{}]", record.id, record.title, record.status);
                    }
                }
            }
            DraftCommands::Approve { id, project } => {
                draft::approve_draft(&project, &id)?;
                println!("Approved draft `{id}` into owned Skilllet");
            }
            DraftCommands::Reject { id, project } => {
                draft::reject_draft(&project, &id)?;
                println!("Rejected draft `{id}`");
            }
        },
        Commands::Extract {
            text,
            file,
            targets,
            provider,
            dry_run,
            project,
        } => {
            let report =
                extract::extract_to_drafts(&project, text, file, targets, provider, dry_run)?;
            println!("{}", report.render());
        }
        Commands::Provider { command } => match command {
            ProviderCommands::Init { project } => {
                let cfg = provider::init_provider_config(&project)?;
                println!("{}", serde_yaml::to_string(&cfg)?);
            }
            ProviderCommands::Show { project } => {
                let cfg = provider::load_or_default_provider_config(&project)?;
                println!("{}", serde_yaml::to_string(&cfg)?);
            }
        },
        Commands::Catalog { command } => match command {
            CatalogCommands::Init { project } => {
                let catalog = catalog::init_catalog(&project)?;
                println!("{}", serde_yaml::to_string(&catalog)?);
            }
            CatalogCommands::List { project } => {
                let status = catalog::catalog_status(&project)?;
                if status.items.is_empty() {
                    println!("No catalog packages found.");
                } else {
                    for item in status.items {
                        println!(
                            "- {}@{}: {} [{}] {}",
                            item.package.id,
                            item.package.version,
                            item.package.title,
                            if item.installed {
                                "installed"
                            } else {
                                "available"
                            },
                            item.package.source_url
                        );
                    }
                }
            }
            CatalogCommands::Validate { project } => {
                let catalog = catalog::load_or_default_catalog(&project)?;
                let report = catalog::validate_catalog(&catalog);
                println!("{}", report.render());
                if report.errors > 0 {
                    std::process::exit(1);
                }
            }
            CatalogCommands::Install {
                id,
                targets,
                project,
            } => {
                let package = catalog::install_catalog_package(&project, &id, targets)?;
                println!("Installed catalog package `{}` as Skilllet", package.id);
                println!("Run `agent-kernel build --preview` to inspect generated instructions.");
            }
        },
        Commands::TestRules { project } => {
            let report = rule_test::run_rule_tests(&project)?;
            println!("{}", report.render());
            if report.failed > 0 {
                std::process::exit(1);
            }
        }
        Commands::Review {
            json,
            approve_drafts,
            reject_drafts,
            project,
        } => {
            let decisions = approve_drafts
                .into_iter()
                .map(review::ReviewDecision::ApproveDraft)
                .chain(
                    reject_drafts
                        .into_iter()
                        .map(review::ReviewDecision::RejectDraft),
                )
                .collect::<Vec<_>>();
            let decision_results = review::apply_review_decisions(&project, &decisions)?;
            let report = review::review_project(&project)?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "decisions": decision_results,
                        "report": report,
                    }))?
                );
            } else {
                for result in decision_results {
                    println!("Applied {} `{}`", result.action, result.id);
                }
                println!("{}", report.render());
            }
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
