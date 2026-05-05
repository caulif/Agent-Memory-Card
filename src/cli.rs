use std::path::PathBuf;

use agent_kernel::hooks;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "agent-kernel")]
#[command(about = "Project-centered Agent skills and rules workspace")]
#[command(version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
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

    #[test]
    fn cli_accepts_observe_local_command() {
        let cli = Cli::parse_from(["agent-kernel", "observe", "local", "--home", "."]);

        match cli.command {
            Commands::Observe {
                command: ObserveCommands::Local { home, .. },
            } => assert_eq!(home, Some(PathBuf::from("."))),
            _ => panic!("expected observe local command"),
        }
    }

    #[test]
    fn cli_accepts_observe_synthesize_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "observe",
            "synthesize",
            "--target",
            "codex",
            "--dry-run",
        ]);

        match cli.command {
            Commands::Observe {
                command:
                    ObserveCommands::Synthesize {
                        targets, dry_run, ..
                    },
            } => {
                assert_eq!(targets, vec!["codex"]);
                assert!(dry_run);
            }
            _ => panic!("expected observe synthesize command"),
        }
    }

    #[test]
    fn cli_accepts_observe_evolve_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "observe",
            "evolve",
            "--home",
            ".",
            "--target",
            "claude-code",
        ]);

        match cli.command {
            Commands::Observe {
                command: ObserveCommands::Evolve { home, targets, .. },
            } => {
                assert_eq!(home, Some(PathBuf::from(".")));
                assert_eq!(targets, vec!["claude-code"]);
            }
            _ => panic!("expected observe evolve command"),
        }
    }

    #[test]
    fn cli_accepts_preference_list_command() {
        let cli = Cli::parse_from(["agent-kernel", "preference", "list", "--project", "."]);

        match cli.command {
            Commands::Preference {
                command: PreferenceCommands::List { project },
            } => assert_eq!(project, PathBuf::from(".")),
            _ => panic!("expected preference list command"),
        }
    }

    #[test]
    fn cli_accepts_preference_init_command() {
        let cli = Cli::parse_from(["agent-kernel", "preference", "init", "--project", "."]);

        match cli.command {
            Commands::Preference {
                command: PreferenceCommands::Init { project },
            } => assert_eq!(project, PathBuf::from(".")),
            _ => panic!("expected preference init command"),
        }
    }

    #[test]
    fn cli_accepts_preference_validate_command() {
        let cli = Cli::parse_from(["agent-kernel", "preference", "validate", "--project", "."]);

        match cli.command {
            Commands::Preference {
                command: PreferenceCommands::Validate { project },
            } => assert_eq!(project, PathBuf::from(".")),
            _ => panic!("expected preference validate command"),
        }
    }

    #[test]
    fn cli_accepts_preference_test_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "preference",
            "test",
            "--text",
            "Use Playwright instead of Cypress.",
            "--project",
            ".",
        ]);

        match cli.command {
            Commands::Preference {
                command: PreferenceCommands::Test { text, project },
            } => {
                assert_eq!(text, "Use Playwright instead of Cypress.");
                assert_eq!(project, PathBuf::from("."));
            }
            _ => panic!("expected preference test command"),
        }
    }

    #[test]
    fn cli_accepts_project_scan_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "project",
            "scan",
            "--root",
            ".",
            "--max-depth",
            "4",
        ]);

        match cli.command {
            Commands::Project {
                command:
                    ProjectCommands::Scan {
                        roots, max_depth, ..
                    },
            } => {
                assert_eq!(roots, vec![PathBuf::from(".")]);
                assert_eq!(max_depth, 4);
            }
            _ => panic!("expected project scan command"),
        }
    }

    #[test]
    fn cli_accepts_project_add_command() {
        let cli = Cli::parse_from(["agent-kernel", "project", "add", "--path", "."]);

        match cli.command {
            Commands::Project {
                command: ProjectCommands::Add { path, .. },
            } => assert_eq!(path, PathBuf::from(".")),
            _ => panic!("expected project add command"),
        }
    }

    #[test]
    fn cli_accepts_hooks_install_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "hooks",
            "install",
            "--event",
            "stop",
            "--event",
            "session-end",
            "--target",
            "codex",
            "--project",
            ".",
        ]);

        match cli.command {
            Commands::Hooks {
                command:
                    HookCommands::Install {
                        events,
                        targets,
                        dry_run,
                        project,
                    },
            } => {
                assert_eq!(
                    events,
                    vec![ClaudeHookEventArg::Stop, ClaudeHookEventArg::SessionEnd]
                );
                assert_eq!(targets, vec!["codex"]);
                assert!(!dry_run);
                assert_eq!(project, PathBuf::from("."));
            }
            _ => panic!("expected hooks install command"),
        }
    }

    #[test]
    fn cli_accepts_index_rebuild_command() {
        let cli = Cli::parse_from(["agent-kernel", "index", "rebuild", "--project", "."]);

        match cli.command {
            Commands::Index {
                command: IndexCommands::Rebuild { project },
            } => assert_eq!(project, PathBuf::from(".")),
            _ => panic!("expected index rebuild command"),
        }
    }

    #[test]
    fn cli_accepts_draft_update_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "draft",
            "update",
            "--id",
            "project:prefer-bun",
            "--title",
            "Prefer Bun Runtime",
            "--body",
            "Use Bun everywhere.",
            "--target",
            "codex",
            "--target",
            "claude-code",
            "--project",
            ".",
        ]);

        match cli.command {
            Commands::Draft {
                command:
                    DraftCommands::Update {
                        id,
                        title,
                        body,
                        targets,
                        ..
                    },
            } => {
                assert_eq!(id, "project:prefer-bun");
                assert_eq!(title.as_deref(), Some("Prefer Bun Runtime"));
                assert_eq!(body.as_deref(), Some("Use Bun everywhere."));
                assert_eq!(targets, vec!["codex", "claude-code"]);
            }
            _ => panic!("expected draft update command"),
        }
    }

    #[test]
    fn cli_accepts_draft_merge_command() {
        let cli = Cli::parse_from([
            "agent-kernel",
            "draft",
            "merge",
            "--id",
            "project:frontend-defaults",
            "--title",
            "Frontend Defaults",
            "--source",
            "project:use-axios",
            "--source",
            "project:prefer-bun",
            "--target",
            "codex",
            "--target",
            "claude-code",
            "--project",
            ".",
        ]);

        match cli.command {
            Commands::Draft {
                command:
                    DraftCommands::Merge {
                        id,
                        title,
                        sources,
                        targets,
                        ..
                    },
            } => {
                assert_eq!(id, "project:frontend-defaults");
                assert_eq!(title, "Frontend Defaults");
                assert_eq!(sources, vec!["project:use-axios", "project:prefer-bun"]);
                assert_eq!(targets, vec!["codex", "claude-code"]);
            }
            _ => panic!("expected draft merge command"),
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Discover and manage local projects known to Agent-Kernel.
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },

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

    /// Inspect high-confidence preference extraction templates.
    Preference {
        #[command(subcommand)]
        command: PreferenceCommands,
    },

    /// Import and inspect raw observations before Skilllet synthesis.
    Observe {
        #[command(subcommand)]
        command: ObserveCommands,
    },

    /// Manage Hybrid provider configuration.
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },

    /// Install or remove optional Agent-Kernel hooks for Claude Code.
    Hooks {
        #[command(subcommand)]
        command: HookCommands,
    },

    /// Rebuild or inspect local file-native indexes.
    Index {
        #[command(subcommand)]
        command: IndexCommands,
    },

    /// Run file-native data migrations.
    Migrate {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
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
}

#[derive(Subcommand)]
pub(crate) enum ProjectCommands {
    /// Scan local folders and update ~/.agent-kernel/projects.yml.
    Scan {
        /// Root to scan. Repeat for multiple roots. Empty scans common local project folders.
        #[arg(long = "root")]
        roots: Vec<PathBuf>,

        /// Maximum directory depth to scan.
        #[arg(long, default_value_t = 5)]
        max_depth: usize,

        /// Home directory used for the global Agent-Kernel registry.
        #[arg(long)]
        home: Option<PathBuf>,
    },

    /// List registered local projects.
    List {
        /// Home directory used for the global Agent-Kernel registry.
        #[arg(long)]
        home: Option<PathBuf>,
    },

    /// Add one project path to the global registry.
    Add {
        /// Project root path.
        #[arg(long)]
        path: PathBuf,

        /// Home directory used for the global Agent-Kernel registry.
        #[arg(long)]
        home: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
pub(crate) enum SkillletCommands {
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

    /// Merge multiple Skilllets into a new Skilllet.
    Merge {
        /// New merged Skilllet id.
        #[arg(long)]
        id: String,

        /// Human-readable title.
        #[arg(long)]
        title: String,

        /// Source Skilllet id. Repeat for multiple sources.
        #[arg(long = "source")]
        sources: Vec<String>,

        /// Agent target. Repeat for multiple agents.
        #[arg(long = "target")]
        targets: Vec<String>,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Attach a Skilllet as a generated supplement to a mirrored Skill.
    AttachSkill {
        /// Skilllet id.
        #[arg(long)]
        id: String,

        /// Skill reference id from .agent-kernel/skill-index.yml.
        #[arg(long)]
        skill: String,

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
pub(crate) enum AgentCommands {
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
pub(crate) enum DraftCommands {
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

    /// Update editable Draft Inbox fields before approval.
    Update {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Merge multiple Draft Inbox items into one new reviewable Draft.
    Merge {
        #[arg(long)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long = "source")]
        sources: Vec<String>,
        #[arg(long = "target")]
        targets: Vec<String>,
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
pub(crate) enum ObserveCommands {
    /// Import one local conversation or note file as an Observation.
    Import {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "manual-file")]
        source_kind: String,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Scan known local Claude Code and Codex conversation folders.
    Local {
        #[arg(long)]
        home: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List imported Observations.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Synthesize reviewable Draft Inbox items from imported Observations.
    Synthesize {
        /// Agent target. Repeat for multiple agents. Empty means project-level candidate.
        #[arg(long = "target")]
        targets: Vec<String>,
        /// Show candidate count without writing Draft Inbox files.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Import local conversations and synthesize Draft Inbox items in one reviewed step.
    Evolve {
        #[arg(long)]
        home: Option<PathBuf>,
        /// Agent target. Repeat for multiple agents. Empty means project-level candidate.
        #[arg(long = "target")]
        targets: Vec<String>,
        /// Import observations and preview synthesis without writing Draft Inbox files.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum PreferenceCommands {
    /// Write an example project preference registry.
    Init {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// List built-in and project preference templates.
    List {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Validate the project preference registry.
    Validate {
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Test a text snippet against preference templates.
    Test {
        #[arg(long)]
        text: String,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProviderCommands {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ClaudeHookEventArg {
    Stop,
    PreCompact,
    SessionEnd,
}

impl From<ClaudeHookEventArg> for hooks::ClaudeHookEvent {
    fn from(value: ClaudeHookEventArg) -> Self {
        match value {
            ClaudeHookEventArg::Stop => hooks::ClaudeHookEvent::Stop,
            ClaudeHookEventArg::PreCompact => hooks::ClaudeHookEvent::PreCompact,
            ClaudeHookEventArg::SessionEnd => hooks::ClaudeHookEvent::SessionEnd,
        }
    }
}

#[derive(Subcommand)]
pub(crate) enum HookCommands {
    /// Install project-local Claude Code hooks that evolve observations into review candidates.
    Install {
        /// Hook event to install. Repeat for multiple events.
        #[arg(long = "event", value_enum, default_values_t = vec![ClaudeHookEventArg::Stop])]
        events: Vec<ClaudeHookEventArg>,

        /// Agent target. Repeat for multiple agents.
        #[arg(long = "target")]
        targets: Vec<String>,

        /// Print the hook plan without writing settings.local.json.
        #[arg(long)]
        dry_run: bool,

        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Remove Agent-Kernel handlers from project-local Claude Code hooks.
    Uninstall {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum IndexCommands {
    /// Rebuild .agent-kernel/index.yml from source YAML records.
    Rebuild {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Load .agent-kernel/index.yml, rebuilding it if missing.
    Show {
        /// Project root.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum CatalogCommands {
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
