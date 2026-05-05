use std::fs;
use std::path::Path;

#[test]
fn rust_source_files_stay_within_1000_lines() {
    let roots = ["src", "src-tauri/src"];
    let mut violations = Vec::new();

    for root in roots {
        let path = Path::new(root);
        if !path.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(path).follow_links(false) {
            let entry = entry.expect("walkdir entry");
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let lines = fs::read_to_string(entry.path())
                .expect("source file")
                .lines()
                .count();
            if lines > 1000 {
                violations.push(format!("{} ({lines} lines)", entry.path().display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Rust source files exceeded 1000 lines:\n{}",
        violations.join("\n")
    );
}

#[test]
fn generated_agent_skills_follow_open_structure() {
    let temp = tempfile::tempdir().expect("tempdir");
    agent_kernel::skilllet::add_skilllet(
        temp.path(),
        "project:frontend-workflow",
        "Frontend Workflow",
        "Use Axios for frontend HTTP requests and Bun for scripts.",
        "procedure",
        "project",
        vec!["claude-code".to_string()],
    )
    .expect("add skilllet");

    agent_kernel::build::sync_project(temp.path()).expect("build");

    let skill_dir = temp
        .path()
        .join(".claude")
        .join("skills")
        .join("frontend-workflow");
    assert_agent_skill_dir(&skill_dir);
}

fn assert_agent_skill_dir(skill_dir: &Path) {
    let skill_md = fs::read_to_string(skill_dir.join("SKILL.md")).expect("SKILL.md");
    assert!(skill_md.starts_with("---\n"));
    assert!(skill_md.contains("\nname: frontend-workflow\n"));
    assert!(skill_md.contains("\ndescription: Use when"));

    let allowed = ["SKILL.md", "references", "scripts", "assets"];
    for entry in fs::read_dir(skill_dir).expect("skill dir") {
        let entry = entry.expect("entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        assert!(
            allowed.contains(&name.as_ref()),
            "unexpected Agent Skill entry: {name}"
        );
    }

    let reference = fs::read_to_string(
        skill_dir
            .join("references")
            .join("project-frontend-workflow.md"),
    )
    .expect("reference");
    assert!(reference.contains("Use Axios for frontend HTTP requests"));
}
