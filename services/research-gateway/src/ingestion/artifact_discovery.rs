use std::fs;
use std::path::{Path, PathBuf};

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

const DISCOVERY_ROOTS: &[&str] = &[".planning", "docs", "_bmad", "_bmad-output", ".bmad"];

pub fn discover_artifact_sources(repo_root: &Path) -> ArtifactDiscoveryManifest {
    let mut files = Vec::new();
    for root in DISCOVERY_ROOTS {
        collect_markdown_files(repo_root.join(root), &mut files);
    }

    let mut sources: Vec<ArtifactSource> = files
        .into_iter()
        .filter_map(|absolute| {
            let relative = absolute.strip_prefix(repo_root).ok()?;
            let relative = path_to_unix(relative);
            classify_intent_source(&relative).map(|kind| ArtifactSource {
                relative_path: relative,
                kind,
            })
        })
        .collect();

    sources.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    ArtifactDiscoveryManifest { sources }
}

pub fn telemetry_paths(manifest: &ArtifactDiscoveryManifest) -> Vec<String> {
    manifest
        .sources
        .iter()
        .map(|source| source.relative_path.clone())
        .collect()
}

fn collect_markdown_files(path: PathBuf, files: &mut Vec<PathBuf>) {
    if !path.exists() {
        return;
    }

    if path.is_file() {
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            files.push(path);
        }
        return;
    }

    let mut entries = match fs::read_dir(&path) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return,
    };
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        collect_markdown_files(entry.path(), files);
    }
}

fn classify_intent_source(relative_path: &str) -> Option<ArtifactSourceKind> {
    if is_generated_chatter(relative_path) {
        return None;
    }

    let lowered = relative_path.to_ascii_lowercase();
    if lowered.contains("prd") {
        return Some(ArtifactSourceKind::Prd);
    }
    if lowered.contains("architecture") || lowered.contains("arch") {
        return Some(ArtifactSourceKind::Architecture);
    }
    if lowered.contains("roadmap") {
        return Some(ArtifactSourceKind::Roadmap);
    }
    if lowered.contains("story") || lowered.contains("stories") {
        return Some(ArtifactSourceKind::Story);
    }
    None
}

fn is_generated_chatter(relative_path: &str) -> bool {
    let lowered = relative_path.to_ascii_lowercase();
    lowered.contains("/state.md")
        || lowered.contains("summary")
        || lowered.contains("telemetry")
        || lowered.contains("changelog")
        || lowered.contains("deferred-items")
        || lowered.contains("/logs/")
}

fn path_to_unix(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
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
