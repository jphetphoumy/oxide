use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandDef {
    pub name: String,
    pub slash_name: String,
    pub description: String,
}

pub const BUILTIN_COMMANDS: &[(&str, &str, &str)] = &[
    ("new", "/new", "Start a new conversation"),
    ("switch", "/switch", "Switch to a different agent"),
    ("resume", "/resume", "Resume an existing conversation"),
];

static SKILL_COMMANDS: OnceLock<Vec<SlashCommandDef>> = OnceLock::new();

pub fn register_skill_commands(skills: &[crate::skills::Skill]) {
    let mut commands = Vec::new();
    for skill in skills {
        commands.push(SlashCommandDef {
            name: format!("skills:{}", skill.id),
            slash_name: format!("/skills:{}", skill.id),
            description: skill.description.clone(),
        });
    }
    if SKILL_COMMANDS.set(commands).is_err() {
        tracing::warn!("register_skill_commands called more than once; second call ignored");
    }
}

pub fn filter_commands(prefix: &str) -> Vec<SlashCommandDef> {
    let prefix_lower = prefix.to_lowercase();
    let mut results = Vec::new();

    // Filter built-in commands
    for (name, slash_name, desc) in BUILTIN_COMMANDS {
        if name.to_lowercase().starts_with(&prefix_lower) {
            results.push(SlashCommandDef {
                name: name.to_string(),
                slash_name: slash_name.to_string(),
                description: desc.to_string(),
            });
        }
    }

    // Filter skill commands
    if let Some(skill_cmds) = SKILL_COMMANDS.get() {
        for cmd in skill_cmds {
            if cmd.name.to_lowercase().starts_with(&prefix_lower) {
                results.push(cmd.clone());
            }
        }
    }

    results
}

pub fn complete(prefix: &str) -> Option<String> {
    filter_commands(prefix)
        .first()
        .map(|cmd| cmd.slash_name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_commands_is_not_empty() {
        assert!(!BUILTIN_COMMANDS.is_empty());
    }

    #[test]
    fn filter_empty_prefix_returns_all_builtins() {
        let results = filter_commands("");
        // Skill commands may be registered by other tests (shared static), so at least builtins
        assert!(results.len() >= BUILTIN_COMMANDS.len());
        let builtin_names: Vec<_> = BUILTIN_COMMANDS.iter().map(|(n, _, _)| *n).collect();
        for name in builtin_names {
            assert!(results.iter().any(|r| r.name == name));
        }
    }

    #[test]
    fn filter_matching_prefix_returns_matches() {
        let results = filter_commands("sw");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "switch");
    }

    #[test]
    fn filter_full_name_returns_exact_match() {
        let results = filter_commands("switch");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "switch");
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let results = filter_commands("xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_is_case_insensitive() {
        let results = filter_commands("SW");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "switch");
    }

    #[test]
    fn complete_returns_top_match() {
        let result = complete("sw");
        assert_eq!(result, Some("/switch".to_string()));
    }

    #[test]
    fn complete_returns_none_for_no_match() {
        assert_eq!(complete("xyz"), None);
    }

    #[test]
    fn complete_full_name_returns_itself() {
        let result = complete("switch");
        assert_eq!(result, Some("/switch".to_string()));
    }

    #[test]
    fn complete_empty_returns_first_command() {
        let result = complete("");
        assert!(result.is_some());
    }

    #[test]
    fn filter_new_command() {
        let results = filter_commands("new");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "new");
    }

    #[test]
    fn filter_new_prefix() {
        let results = filter_commands("ne");
        assert!(results.iter().any(|cmd| cmd.name == "new"));
    }

    #[test]
    fn complete_new_prefix() {
        let result = complete("ne");
        assert_eq!(result, Some("/new".to_string()));
    }

    #[test]
    fn complete_full_new_returns_itself() {
        let result = complete("new");
        assert_eq!(result, Some("/new".to_string()));
    }

    #[test]
    fn register_skill_commands_adds_skills() {
        let skills = vec![crate::skills::Skill {
            id: "code-review".to_string(),
            name: "Code Reviewer".to_string(),
            description: "Review code".to_string(),
            path: std::path::PathBuf::from(".agents/skills/code-review.md"),
        }];
        register_skill_commands(&skills);
        let results = filter_commands("skills:");
        assert!(results.iter().any(|cmd| cmd.name == "skills:code-review"));
    }
}
