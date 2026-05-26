use std::path::Path;

use crate::api::ApiError;
use crate::output::OutputConfig;

const SKILL_CONTENT: &str = include_str!("../../assets/SKILL.md");

struct Target {
    name: &'static str,
    base: &'static str,
}

const TARGETS: &[Target] = &[
    Target {
        name: "Claude Code",
        base: ".claude",
    },
    Target {
        name: "Cursor",
        base: ".agents",
    },
    Target {
        name: "Codex",
        base: ".codex",
    },
    Target {
        name: "Gemini",
        base: ".gemini",
    },
];

pub fn init(out: &OutputConfig, path: Option<&Path>) -> Result<(), ApiError> {
    if let Some(dir) = path {
        return write_skill(out, "custom", &dir.join("jira"));
    }

    let home = dirs::home_dir()
        .ok_or_else(|| ApiError::Other("cannot determine home directory".into()))?;

    let mut wrote = 0usize;
    for t in TARGETS {
        let base = home.join(t.base);
        if t.base != ".claude" && !base.exists() {
            continue;
        }
        write_skill(out, t.name, &base.join("skills").join("jira"))?;
        wrote += 1;
    }
    if wrote == 0 {
        out.print_message("No coding CLI directories detected.");
    }
    Ok(())
}

fn write_skill(out: &OutputConfig, label: &str, skill_dir: &Path) -> Result<(), ApiError> {
    std::fs::create_dir_all(skill_dir)
        .map_err(|e| ApiError::Other(format!("create {}: {e}", skill_dir.display())))?;
    let dest = skill_dir.join("SKILL.md");
    std::fs::write(&dest, SKILL_CONTENT)
        .map_err(|e| ApiError::Other(format!("write {}: {e}", dest.display())))?;
    out.print_message(&format!("{label}: wrote {}", dest.display()));
    Ok(())
}
