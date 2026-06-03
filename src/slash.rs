#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandDef {
    pub name: &'static str,
    pub slash_name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[SlashCommandDef] = &[SlashCommandDef {
    name: "switch",
    slash_name: "/switch",
    description: "Switch to a different agent",
}];

pub fn filter_commands(prefix: &str) -> Vec<&'static SlashCommandDef> {
    let prefix_lower = prefix.to_lowercase();
    COMMANDS
        .iter()
        .filter(|cmd| cmd.name.to_lowercase().starts_with(&prefix_lower))
        .collect()
}

pub fn complete(prefix: &str) -> Option<&'static str> {
    filter_commands(prefix).first().map(|cmd| cmd.slash_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_is_not_empty() {
        assert!(!COMMANDS.is_empty());
    }

    #[test]
    fn filter_empty_prefix_returns_all() {
        let results = filter_commands("");
        assert_eq!(results.len(), COMMANDS.len());
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
        assert_eq!(complete("sw"), Some("/switch"));
    }

    #[test]
    fn complete_returns_none_for_no_match() {
        assert_eq!(complete("xyz"), None);
    }

    #[test]
    fn complete_full_name_returns_itself() {
        assert_eq!(complete("switch"), Some("/switch"));
    }

    #[test]
    fn complete_empty_returns_first_command() {
        let result = complete("");
        assert!(result.is_some());
    }
}
