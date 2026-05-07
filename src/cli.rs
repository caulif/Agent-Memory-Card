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

    /// Serve a minimal MCP stdio endpoint for Agent-Kernel workflows.
    Mcp {
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
        /// Synthesis engine: local, llm, claude-code, or codex.
        #[arg(long, default_value = "local")]
        engine: String,
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

    /// Re-read all project-related local conversations and synthesize from that full replay.
    Replay {
        #[arg(long)]
        home: Option<PathBuf>,
        /// Agent target. Repeat for multiple agents. Empty means project-level candidate.
        #[arg(long = "target")]
        targets: Vec<String>,
        /// Synthesis engine: local, llm, claude-code, or codex.
        #[arg(long, default_value = "local")]
        engine: String,
        /// Preview synthesis without writing Observation or Draft Inbox files.
        #[arg(long)]
        dry_run: bool,
        /// Include sessions whose project metadata could not be determined.
        #[arg(long)]
        include_unknown_project: bool,
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },

    /// Generate a reviewable weekly extraction reflexion proposal from feedback history.
    Reflect {
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
