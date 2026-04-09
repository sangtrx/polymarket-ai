use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactSourceKind {
    Prd,
    Architecture,
    Story,
    Roadmap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSource {
    pub relative_path: String,
    pub kind: ArtifactSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtifactDiscoveryManifest {
    pub sources: Vec<ArtifactSource>,
}

pub fn discover_artifact_sources(_repo_root: &Path) -> ArtifactDiscoveryManifest {
    ArtifactDiscoveryManifest::default()
}

pub fn telemetry_paths(manifest: &ArtifactDiscoveryManifest) -> Vec<String> {
    manifest
        .sources
        .iter()
        .map(|source| source.relative_path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_workspace() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("artifact-discovery-{stamp}"));
        fs::create_dir_all(&dir).expect("temp root should be created");
        dir
    }

    fn write_markdown(root: &Path, relative: &str) {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("parent dirs should be created");
        }
        fs::write(target, "# Heading\n- [ ] requirement").expect("file should be written");
    }

    #[test]
    fn includes_only_prd_architecture_story_and_roadmap_files() {
        let workspace = test_workspace();
        write_markdown(&workspace, ".planning/PRD.md");
        write_markdown(&workspace, "docs/architecture.md");
        write_markdown(&workspace, ".planning/stories/story-1.md");
        write_markdown(&workspace, "docs/ROADMAP.md");
        write_markdown(&workspace, ".planning/notes.md");

        let manifest = discover_artifact_sources(&workspace);
        let paths: Vec<String> = manifest
            .sources
            .iter()
            .map(|source| source.relative_path.clone())
            .collect();

        assert_eq!(
            paths,
            vec![
                ".planning/PRD.md",
                ".planning/stories/story-1.md",
                "docs/ROADMAP.md",
                "docs/architecture.md",
            ]
        );
    }

    #[test]
    fn restricts_discovery_roots_to_planning_docs_and_bmad_dirs() {
        let workspace = test_workspace();
        write_markdown(&workspace, ".planning/PRD.md");
        write_markdown(&workspace, "docs/roadmap.md");
        write_markdown(&workspace, "_bmad/stories/story-2.md");
        write_markdown(&workspace, "_bmad-output/telemetry.md");
        write_markdown(&workspace, "outside/PRD.md");

        let manifest = discover_artifact_sources(&workspace);
        let paths: Vec<String> = manifest
            .sources
            .iter()
            .map(|source| source.relative_path.clone())
            .collect();

        assert!(paths.iter().all(|path| {
            path.starts_with(".planning/")
                || path.starts_with("docs/")
                || path.starts_with("_bmad/")
                || path.starts_with("_bmad-output/")
        }));
        assert!(!paths.iter().any(|path| path.starts_with("outside/")));
    }

    #[test]
    fn telemetry_excludes_generated_chatter_files() {
        let workspace = test_workspace();
        write_markdown(&workspace, ".planning/PRD.md");
        write_markdown(
            &workspace,
            "_bmad-output/telemetry/2026-04-09-run-summary.md",
        );
        write_markdown(&workspace, "docs/architecture.md");
        write_markdown(&workspace, ".planning/STATE.md");

        let manifest = discover_artifact_sources(&workspace);
        let telemetry = telemetry_paths(&manifest);

        assert!(telemetry.contains(&".planning/PRD.md".to_string()));
        assert!(telemetry.contains(&"docs/architecture.md".to_string()));
        assert!(!telemetry.iter().any(|path| path.contains("telemetry")));
        assert!(!telemetry.iter().any(|path| path.ends_with("STATE.md")));
    }
}
