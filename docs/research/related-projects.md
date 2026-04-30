# Related Projects And Borrowed Ideas

Date: 2026-05-01

Agent-Kernel should avoid rebuilding mature pieces from nearby ecosystems. The v0.1 implementation borrows these ideas:

## Agent Skills Standard

- Source: https://agentskills.io/
- Borrowed idea: a Skill is a directory with `SKILL.md` and optional supporting files.
- Product impact: Mirror mode copies the entire directory structure, not only `SKILL.md`.

## Claude Code Skills

- Source: https://code.claude.com/docs/en/skills
- Borrowed idea: progressive disclosure and on-demand skills keep always-loaded context small.
- Product impact: Agent-Kernel treats full Skills as attachable capabilities, while rule files stay short build artifacts.

## OpenAI Codex Skills And AGENTS.md

- Source: https://developers.openai.com/codex/skills
- Source: https://developers.openai.com/codex/guides/agents-md
- Borrowed idea: project instructions and skills are separate surfaces.
- Product impact: Codex export writes `AGENTS.md` and mirrors Skills into `.agents/skills`.

## GitHub Copilot Coding Agent Skills

- Source: https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/create-skills
- Borrowed idea: GitHub also uses `SKILL.md`-based skill directories.
- Product impact: scanning now includes project `.github/skills` and home `.copilot/skills` candidates.

## Superpowers

- Source: https://github.com/obra/superpowers
- Borrowed idea: skills are practical workflow bundles and can be shared through a linked local directory.
- Product impact: the local install is indexed as referenced skills and mirrored without modifying the source.

## Skilldex-Style Validation

- Related search term: "Skilldex AI skills manager SKILL.md"
- Borrowed idea: skill libraries benefit from format checks and dispatch-quality warnings.
- Product impact: Scan records warnings for missing `name`, missing `description`, empty names, and very short descriptions.

## Dotfiles And Build Artifact Tools

- Reference patterns: chezmoi, GNU Stow, Vite/Webpack build outputs.
- Borrowed idea: user-owned source config should produce generated target files.
- Product impact: `.agent-kernel/project.yml` is declarative source; `AGENTS.md`, `CLAUDE.md`, and mirrored Skill folders are build artifacts.

