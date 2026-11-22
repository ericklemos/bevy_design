use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const MANIFEST_FILE: &str = "project.ron";
const VERSION: u32 = 1;
const INITIAL_SCENE: &str = "main.scn.ron";
const GAME_FILES: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../templates/game/Cargo.toml")),
    ("src/main.rs", include_str!("../templates/game/main.rs")),
    (".gitignore", include_str!("../templates/game/gitignore")),
    ("README.md", include_str!("../templates/game/README.md")),
    ("assets/.gitkeep", ""),
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    name: String,
    startup_scene: String,
}

#[derive(Debug)]
pub struct Project {
    root: PathBuf,
    manifest: Manifest,
}

impl Project {
    /// Creates a standalone Bevy game in a new directory, without modifying existing folders.
    pub fn create(root: &Path, name: &str) -> io::Result<Self> {
        let project = Self {
            root: std::path::absolute(root)?,
            manifest: Manifest {
                version: VERSION,
                name: name.trim().into(),
                startup_scene: INITIAL_SCENE.into(),
            },
        };
        project.validate_manifest()?;
        let mut manifest =
            ron::ser::to_string_pretty(&project.manifest, ron::ser::PrettyConfig::default())
                .map_err(io::Error::other)?;
        manifest.push('\n');

        at(&project.root, fs::create_dir(&project.root))?;
        let assets = project.assets();
        let scenes = project.scenes();
        let scene = scenes.join(INITIAL_SCENE);
        let manifest_path = project.root.join(MANIFEST_FILE);
        at(&assets, fs::create_dir(&assets))?;
        at(&scenes, fs::create_dir(&scenes))?;
        let src = project.root.join("src");
        at(&src, fs::create_dir(&src))?;
        for (relative, contents) in GAME_FILES {
            let path = project.root.join(relative);
            at(&path, fs::write(&path, contents))?;
        }
        at(&scene, fs::write(&scene, "(resources: {}, entities: {})\n"))?;
        at(&manifest_path, fs::write(&manifest_path, manifest))?;
        Ok(project)
    }

    pub fn open(root: &Path) -> io::Result<Self> {
        let root = at(root, root.canonicalize())?;
        let path = root.join(MANIFEST_FILE);
        let bytes = at(&path, fs::read(&path))?;
        let manifest = ron::de::from_bytes(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
        let project = Self {
            root,
            manifest: at(&path, manifest)?,
        };
        at(&path, project.validate_manifest())?;
        for dir in [project.assets(), project.scenes()] {
            if !at(&dir, fs::metadata(&dir))?.is_dir() {
                return Err(invalid(format!("{} não é um diretório", dir.display())));
            }
        }
        let scene = project.scenes().join(project.startup_scene());
        if !at(&scene, fs::metadata(&scene))?.is_file() {
            return Err(invalid(format!("{} não é um arquivo", scene.display())));
        }
        Ok(project)
    }

    fn validate_manifest(&self) -> io::Result<()> {
        if self.manifest.version != VERSION {
            return Err(invalid(format!(
                "versão de projeto não suportada: {}",
                self.manifest.version
            )));
        }
        if self.name().trim().is_empty() || self.name().chars().any(char::is_control) {
            return Err(invalid("informe um nome de projeto válido"));
        }
        let scene = self.startup_scene();
        if !scene.ends_with(".scn.ron")
            || scene.contains(['\\', ':', '#'])
            || scene.chars().any(char::is_control)
            || scene.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(invalid(
                "startup_scene deve ser um caminho .scn.ron relativo a scenes/",
            ));
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn scenes(&self) -> PathBuf {
        self.root.join("scenes")
    }

    pub fn startup_scene(&self) -> &str {
        &self.manifest.startup_scene
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn at<T>(path: &Path, result: io::Result<T>) -> io::Result<T> {
    result.map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reopens_a_project() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("Meu jogo");
        Project::create(&root, " Meu jogo ").unwrap();
        let project = Project::open(&root).unwrap();
        assert_eq!(project.root(), root.canonicalize().unwrap());
        assert_eq!(project.name(), "Meu jogo");
        assert!(project.assets().is_dir());
        assert!(project.scenes().join(project.startup_scene()).is_file());
        for (relative, contents) in GAME_FILES {
            assert_eq!(fs::read_to_string(root.join(relative)).unwrap(), *contents);
        }
    }

    #[test]
    fn does_not_overwrite_an_existing_project() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("jogo");
        Project::create(&root, "Original").unwrap();
        let source = root.join("src/main.rs");
        fs::write(&source, "// código do jogo\n").unwrap();
        assert_eq!(
            Project::create(&root, "Outro").unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(Project::open(&root).unwrap().name(), "Original");
        assert_eq!(fs::read_to_string(source).unwrap(), "// código do jogo\n");
    }

    #[test]
    fn opens_legacy_projects_without_a_cargo_manifest() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("legado");
        let project = Project::create(&root, "Legado").unwrap();
        for (relative, _) in GAME_FILES {
            fs::remove_file(root.join(relative)).unwrap();
        }
        assert_eq!(Project::open(&root).unwrap().root(), project.root());
    }

    #[test]
    fn rejects_invalid_name_and_scene_outside_project() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("jogo");
        assert!(Project::create(&root, " ").is_err());
        assert!(!root.exists());
        Project::create(&root, "Jogo").unwrap();
        fs::write(
            root.join(MANIFEST_FILE),
            r#"(version: 1, name: "Jogo", startup_scene: "../main.scn.ron")"#,
        )
        .unwrap();
        assert_eq!(
            Project::open(&root).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
