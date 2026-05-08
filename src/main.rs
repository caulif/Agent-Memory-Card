use std::path::PathBuf;

use agent_kernel::{
    build, candidate, catalog, config, draft, extract, feedback, hooks, index, mcp, memory_card,
    migration, observation, project_registry, provider, review, rule_test, scanner,
};
use anyhow::Result;
use clap::Parser;

mod cli;
#[cfg(test)]
mod cli_tests;

use cli::{
    AgentCommands, CatalogCommands, Cli, Commands, DraftCommands, HookCommands, IndexCommands,
    MemoryCardCommands, ObserveCommands, PreferenceCommands, ProjectCommands, ProviderCommands,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Project { command } => match command {
            ProjectCommands::Scan {
                roots,
                max_depth,
                home,
            } => {
                let home = home.unwrap_or_else(default_home_dir);
                let roots = if roots.is_empty() {
                    let cwd = std::env::current_dir()?;
                    project_registry::default_scan_roots(&cwd)
                } else {
                    roots
                };
                let report = project_registry::scan_and_register(&home, &roots, max_depth)?;
                println!("{}", report.render());
            }
            ProjectCommands::List { home } => {
                let home = home.unwrap_or_else(default_home_dir);
                let registry = project_registry::load_registry(&home)?;
                if registry.projects.is_empty() {
                    println!("No projects registered. Run `agent-kernel project scan` first.");
                } else {
                    println!("Agent Memory Kernel projects\n");
                    for project in registry.projects {
                        println!(
                            "- {} [{}]\n  {}",
                            project.name,
                            project.agents.join(", "),
                            project.path
                        );
                    }
                }
            }
            ProjectCommands::Add { path, home } => {
                let home = home.unwrap_or_else(default_home_dir);
                let project = project_registry::add_project(&home, &path)?;
                println!("Registered project `{}`", project.name);
                println!("{}", project.path);
            }
        },
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
        Commands::MemoryCard { command } => match command {
            MemoryCardCommands::Add {
                id,
                title,
                body,
                kind,
                scope,
                targets,
                project,
            } => {
                memory_card::add_memory_card(&project, &id, &title, &body, &kind, &scope, targets)?;
                println!("Added memory_card `{id}`");
                println!("Run `agent-kernel build --preview` to inspect generated instructions.");
            }
            MemoryCardCommands::List { project } => {
                let records = memory_card::load_memory_cards(&project)?;
                if records.is_empty() {
                    println!("No memory_cards found.");
                } else {
                    for record in records {
                        println!("- {}: {}", record.id, record.title);
                    }
                }
            }
            MemoryCardCommands::Targets {
                id,
                targets,
                project,
            } => {
                memory_card::set_memory_card_targets(&project, &id, targets)?;
                println!("Updated targets for memory_card `{id}`");
            }
            MemoryCardCommands::Merge {
                id,
                title,
                sources,
                targets,
                project,
            } => {
                memory_card::merge_memory_cards(&project, &id, &title, sources, targets)?;
                println!("Merged memory_card `{id}`");
                println!("Run `agent-kernel build --preview` to inspect generated instructions.");
            }
            MemoryCardCommands::AttachSkill { id, skill, project } => {
                config::add_skill_supplement(&project, &skill, &id)?;
                println!("Attached memory_card `{id}` to skill `{skill}`");
                println!("Run `agent-kernel sync` to update mirrored skill supplements.");
            }
            MemoryCardCommands::Matrix { project } => {
                let matrix = memory_card::memory_card_target_matrix(&project)?;
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
                        confidence: None,
                        reason: None,
                        matched_template: None,
                        extraction: candidate::ExtractionMetadata::default(),
                    },
                )?;
                println!("Added draft `{id}`");
            }
            DraftCommands::List { project } => {
                let records = draft::load_reviewable_drafts(&project)?;
                if records.is_empty() {
                    println!("No drafts found.");
                } else {
                    for record in records {
                        println!("- {}: {} [{}]", record.id, record.title, record.status);
                    }
                }
            }
            DraftCommands::Update {
                id,
                title,
                body,
                kind,
                scope,
                targets,
                project,
            } => {
                let targets = if targets.is_empty() {
                    None
                } else {
                    Some(targets)
                };
                let updated = draft::update_draft(
                    &project,
                    &id,
                    draft::DraftUpdate {
                        title,
                        body,
                        kind,
                        scope,
                        targets,
                        ..Default::default()
                    },
                )?;
                println!("Updated draft `{}`: {}", updated.id, updated.title);
            }
            DraftCommands::Merge {
                id,
                title,
                sources,
                targets,
                project,
            } => {
                let merged = draft::merge_drafts(&project, &id, &title, sources, targets)?;
                println!("Merged draft `{}`: {}", merged.id, merged.title);
                println!("Review and approve it when ready.");
            }
            DraftCommands::Approve { id, project } => {
                draft::approve_draft(&project, &id)?;
                println!("Approved or cleared draft `{id}`");
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
        Commands::Preference { command } => match command {
            PreferenceCommands::Init { project } => {
                let created = extract::init_preference_registry(&project)?;
                if created {
                    println!("Wrote .agent-kernel/preference-registry.yml");
                } else {
                    println!(".agent-kernel/preference-registry.yml already exists");
                }
            }
            PreferenceCommands::List { project } => {
                let templates = extract::preference_templates(&project)?;
                if templates.is_empty() {
                    println!("No preference templates found.");
                } else {
                    println!("Agent Memory Kernel preference templates\n");
                    for template in templates {
                        println!(
                            "- [{}] {}: {}",
                            template.source, template.title, template.body
                        );
                    }
                }
            }
            PreferenceCommands::Validate { project } => {
                let report = extract::validate_preference_registry(&project)?;
                println!("{}", report.render());
                if report.errors > 0 {
                    std::process::exit(1);
                }
            }
            PreferenceCommands::Test { text, project } => {
                let report = extract::test_preference_text(&project, &text)?;
                println!("{}", report.render());
            }
        },
        Commands::Observe { command } => match command {
            ObserveCommands::Import {
                file,
                source_kind,
                agent,
                project,
            } => {
                let report = observation::import_observation_file(
                    &project,
                    &file,
                    &source_kind,
                    agent.as_deref(),
                )?;
                println!("{}", report.render());
            }
            ObserveCommands::Local { home, project } => {
                let home = home.unwrap_or_else(default_home_dir);
                let report = observation::import_local_conversations(&project, &home)?;
                println!("{}", report.render());
            }
            ObserveCommands::List { project } => {
                let observations = observation::load_observations(&project)?;
                if observations.is_empty() {
                    println!("No observations found.");
                } else {
                    for observation in observations {
                        println!(
                            "- {} [{}] {}",
                            observation.id,
                            observation.agent.as_deref().unwrap_or("local"),
                            observation.source_path
                        );
                    }
                }
            }
            ObserveCommands::Synthesize {
                targets,
                engine,
                dry_run,
                project,
            } => {
                let report = observation::synthesize_observations_to_drafts_with_engine(
                    &project, targets, dry_run, &engine,
                )?;
                println!("{}", report.render());
            }
            ObserveCommands::Evolve {
                home,
                targets,
                dry_run,
                project,
            } => {
                let home = home.unwrap_or_else(default_home_dir);
                let report =
                    observation::evolve_local_conversations(&project, &home, targets, dry_run)?;
                println!("{}", report.render());
            }
            ObserveCommands::Replay {
                home,
                targets,
                engine,
                dry_run,
                include_unknown_project,
                project,
            } => {
                let home = home.unwrap_or_else(default_home_dir);
                let report = observation::replay_local_conversations(
                    &project,
                    &home,
                    targets,
                    dry_run,
                    &engine,
                    include_unknown_project,
                )?;
                println!("{}", report.render());
            }
            ObserveCommands::Reflect { project } => {
                match feedback::write_weekly_reflexion_proposal(&project)? {
                    Some(path) => {
                        println!("Wrote extraction reflexion proposal:");
                        println!("{}", path.display());
                    }
                    None => println!("No repeated rejection patterns found yet."),
                }
            }
        },
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
        Commands::Hooks { command } => match command {
            HookCommands::Install {
                events,
                targets,
                dry_run,
                project,
            } => {
                let events = events.into_iter().map(Into::into).collect();
                let report = if dry_run {
                    hooks::plan_claude_project_hooks(&project, events, targets)?
                } else {
                    hooks::install_claude_project_hooks(&project, events, targets)?
                };
                println!("{}", serde_yaml::to_string(&report)?);
            }
            HookCommands::Uninstall { project } => {
                let report = hooks::uninstall_claude_project_hooks(&project)?;
                println!("{}", serde_yaml::to_string(&report)?);
            }
        },
        Commands::Index { command } => match command {
            IndexCommands::Rebuild { project } => {
                let report = index::rebuild_project_index(&project)?;
                println!("{}", serde_yaml::to_string(&report)?);
            }
            IndexCommands::Show { project } => {
                let report = index::load_project_index(&project)?;
                println!("{}", serde_yaml::to_string(&report)?);
            }
        },
        Commands::Migrate { project } => {
            let report = migration::migrate_project_records(&project)?;
            println!("{}", serde_yaml::to_string(&report)?);
        }
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
                println!("Installed catalog package `{}` as Memory Card", package.id);
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
        Commands::Mcp { project } => {
            mcp::serve_stdio(&project)?;
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
    }

    Ok(())
}

fn default_home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
