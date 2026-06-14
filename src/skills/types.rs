use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Skill {
    pub id: String,          // filename stem, e.g. "code-review"
    pub name: String,        // from frontmatter
    pub description: String, // from frontmatter
    #[allow(dead_code)]
    pub path: PathBuf, // absolute path to the .md file
}
