use std::path::Path;

use crate::skills::Skill;

/// Discover skills from a directory.
///
/// Reads all `.md` files in the directory (non-recursive), parses YAML frontmatter,
/// and returns a vector of discovered skills.
///
/// Missing directory → return empty Vec (not an error).
/// Unreadable file → log warning and skip.
/// Missing name or description → log warning and skip.
pub fn discover_skills(dir: &Path) -> Vec<Skill> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        tracing::debug!(path = %dir.display(), "skills directory does not exist or is unreadable");
        return Vec::new();
    };

    let mut skills = Vec::new();

    for entry in entries {
        let Ok(entry) = entry else {
            tracing::warn!("failed to read directory entry in skills directory");
            continue;
        };

        let path = entry.path();

        // Only process .md files
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name)
            .to_string();

        match parse_skill(&path, &id) {
            Ok(skill) => skills.push(skill),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse skill file");
            }
        }
    }

    skills
}

/// Parse a skill file, extracting name and description from YAML frontmatter.
fn parse_skill(path: &Path, id: &str) -> anyhow::Result<Skill> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("failed to read file: {e}"))?;

    // Split on second `---` delimiter
    let parts: Vec<&str> = content.split("---").collect();

    if parts.len() < 3 {
        return Err(anyhow::anyhow!("file does not have valid YAML frontmatter"));
    }

    // Parse the header block (between first and second ---)
    let header = parts[1];

    // Parse YAML
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(header).map_err(|e| anyhow::anyhow!("failed to parse YAML: {e}"))?;

    // Extract name and description
    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'name' field in frontmatter"))?
        .to_string();

    let description = parsed
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing 'description' field in frontmatter"))?
        .to_string();

    // Canonicalize path to absolute
    let absolute_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    Ok(Skill {
        id: id.to_string(),
        name,
        description,
        path: absolute_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_empty_directory_returns_empty_vec() {
        let dir = TempDir::new().expect("temp dir");
        let skills = discover_skills(dir.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_missing_directory_returns_empty_vec() {
        let skills = discover_skills(Path::new("/nonexistent/path/to/skills"));
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_valid_skill_file() {
        let dir = TempDir::new().expect("temp dir");
        let skill_path = dir.path().join("code-review.md");
        let content = r#"---
name: Code Reviewer
description: Review code for correctness
---
You are a code reviewer."#;
        fs::write(&skill_path, content).expect("write skill file");

        let skills = discover_skills(dir.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "code-review");
        assert_eq!(skills[0].name, "Code Reviewer");
        assert_eq!(skills[0].description, "Review code for correctness");
    }

    #[test]
    fn discover_skips_file_missing_name() {
        let dir = TempDir::new().expect("temp dir");
        let skill_path = dir.path().join("incomplete.md");
        let content = r#"---
description: Some description
---
Body"#;
        fs::write(&skill_path, content).expect("write skill file");

        let skills = discover_skills(dir.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_skips_file_missing_description() {
        let dir = TempDir::new().expect("temp dir");
        let skill_path = dir.path().join("incomplete.md");
        let content = r#"---
name: Some Name
---
Body"#;
        fs::write(&skill_path, content).expect("write skill file");

        let skills = discover_skills(dir.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_skips_non_markdown_files() {
        let dir = TempDir::new().expect("temp dir");
        let txt_path = dir.path().join("skill.txt");
        let content = r#"---
name: Text File
description: Not a skill
---
Body"#;
        fs::write(&txt_path, content).expect("write txt file");

        let skills = discover_skills(dir.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_multiple_skills() {
        let dir = TempDir::new().expect("temp dir");

        let skill1_path = dir.path().join("code-review.md");
        let content1 = r#"---
name: Code Reviewer
description: Review code
---
Body"#;
        fs::write(&skill1_path, content1).expect("write skill1");

        let skill2_path = dir.path().join("sql-expert.md");
        let content2 = r#"---
name: SQL Expert
description: Optimize SQL
---
Body"#;
        fs::write(&skill2_path, content2).expect("write skill2");

        let skills = discover_skills(dir.path());
        assert_eq!(skills.len(), 2);
    }
}
