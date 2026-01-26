//! Skill commands: manage Claude Code skill installation.

use std::fs;
use std::path::PathBuf;

const SKILL_MD: &str = include_str!("../../skills/silo/SKILL.md");

pub fn init(global: bool, dry_run: bool, quiet: bool) -> Result<(), String> {
    let skill_dir = if global {
        let home =
            std::env::var("HOME").map_err(|_| "HOME environment variable not set".to_string())?;
        PathBuf::from(home).join(".claude/skills/silo")
    } else {
        PathBuf::from(".claude/skills/silo")
    };

    if dry_run {
        println!("Would create skill at: {}", skill_dir.display());
        return Ok(());
    }

    fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("Failed to create skill directory: {}", e))?;

    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, SKILL_MD).map_err(|e| format!("Failed to write SKILL.md: {}", e))?;

    if !quiet {
        println!("Installed silo skill to: {}", skill_dir.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_skill_md_content_is_valid() {
        // Verify the embedded content is valid markdown with YAML frontmatter
        assert!(SKILL_MD.starts_with("---"));
        assert!(SKILL_MD.contains("name: silo"));
        assert!(SKILL_MD.contains("description:"));
    }

    #[test]
    fn test_init_dry_run() {
        // Dry run should succeed without creating files
        let result = init(false, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_creates_skill_in_temp_dir() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let original_dir = std::env::current_dir().expect("Failed to get current dir");

        std::env::set_current_dir(temp_dir.path()).expect("Failed to change to temp dir");

        let result = init(false, false, true);
        assert!(result.is_ok());

        let skill_path = temp_dir.path().join(".claude/skills/silo/SKILL.md");
        assert!(skill_path.exists(), "SKILL.md should be created");

        let content = fs::read_to_string(&skill_path).expect("Failed to read SKILL.md");
        assert!(content.contains("name: silo"));

        std::env::set_current_dir(original_dir).expect("Failed to restore dir");
    }
}
