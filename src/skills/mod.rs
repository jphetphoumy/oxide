mod loader;
mod types;

pub use loader::discover_skills;
pub use types::Skill;

pub const SKILLS_DIR: &str = ".agents/skills";
