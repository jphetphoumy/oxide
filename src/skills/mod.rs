mod loader;
mod types;

pub use loader::discover_skills;
pub use types::Skill;

pub const SKILLS_DIR: &str = ".agents/skills";

/// Check if a skill ID is valid.
///
/// Valid skill IDs contain only alphanumeric characters, hyphens, or underscores.
/// They must not be empty. This is a security check to prevent path traversal attacks.
pub fn is_valid_skill_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_skill_id_alphanumeric() {
        assert!(is_valid_skill_id("code_review"));
    }

    #[test]
    fn valid_skill_id_with_hyphens() {
        assert!(is_valid_skill_id("code-review-tool"));
    }

    #[test]
    fn valid_skill_id_with_underscores() {
        assert!(is_valid_skill_id("sql_optimizer"));
    }

    #[test]
    fn valid_skill_id_mixed() {
        assert!(is_valid_skill_id("code-review_v2"));
    }

    #[test]
    fn invalid_skill_id_empty() {
        assert!(!is_valid_skill_id(""));
    }

    #[test]
    fn invalid_skill_id_with_slash() {
        assert!(!is_valid_skill_id("code/review"));
    }

    #[test]
    fn invalid_skill_id_with_dots() {
        assert!(!is_valid_skill_id("code..review"));
    }

    #[test]
    fn invalid_skill_id_with_spaces() {
        assert!(!is_valid_skill_id("code review"));
    }

    #[test]
    fn invalid_skill_id_path_traversal() {
        assert!(!is_valid_skill_id("../../../etc/passwd"));
    }
}
