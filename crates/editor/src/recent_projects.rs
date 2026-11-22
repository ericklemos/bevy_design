use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use projects::Project;
use redb::{Database, DatabaseError, ReadableDatabase, ReadableTable, TableDefinition, TableError};

const RECENT: TableDefinition<u64, (&str, &str)> = TableDefinition::new("recent_projects");
const LIMIT: usize = 20;
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

pub(crate) struct History(PathBuf);

pub(crate) struct RecentProject {
    pub(crate) path: PathBuf,
    pub(crate) name: String,
}

impl History {
    pub(crate) fn local() -> Result<Self> {
        let dirs = directories::ProjectDirs::from("", "", "bevy-editor")
            .ok_or_else(|| io::Error::other("diretório de dados do usuário indisponível"))?;
        Ok(Self(dirs.data_local_dir().join("user.redb")))
    }

    // Release the file lock between operations so editors in distinct processes can share it.
    fn database(&self) -> Result<Database> {
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent)?;
        }
        for _ in 0..4 {
            match Database::create(&self.0) {
                Err(DatabaseError::DatabaseAlreadyOpen) => thread::sleep(Duration::from_millis(10)),
                result => return Ok(result?),
            }
        }
        Ok(Database::create(&self.0)?)
    }

    pub(crate) fn list(&self) -> Result<Vec<RecentProject>> {
        let database = self.database()?;
        let transaction = database.begin_read()?;
        let table = match transaction.open_table(RECENT) {
            Ok(table) => table,
            Err(TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        table
            .iter()?
            .rev()
            .map(|row| {
                let (_, value) = row?;
                let (path, name) = value.value();
                Ok(RecentProject {
                    path: path.into(),
                    name: name.into(),
                })
            })
            .collect()
    }

    pub(crate) fn record(&self, project: &Project) -> Result<()> {
        let path = project.root().to_str().ok_or_else(|| {
            io::Error::other("o caminho do projeto deve estar em UTF-8 para entrar nos recentes")
        })?;
        let database = self.database()?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(RECENT)?;
            let mut keys = Vec::new();
            let mut duplicate = None;
            for row in table.iter()? {
                let (id, value) = row?;
                keys.push(id.value());
                if value.value().0 == path {
                    duplicate = Some(id.value());
                }
            }
            let next = keys.last().copied().unwrap_or_default() + 1;
            if let Some(id) = duplicate {
                table.remove(id)?;
                keys.retain(|key| *key != id);
            }
            for id in keys.iter().take((keys.len() + 1).saturating_sub(LIMIT)) {
                table.remove(*id)?;
            }
            table.insert(next, (path, project.name()))?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn forget(&self, path: &Path) -> Result<()> {
        let database = self.database()?;
        let transaction = database.begin_write()?;
        {
            let mut table = transaction.open_table(RECENT)?;
            let mut found = None;
            for row in table.iter()? {
                let (id, value) = row?;
                if Path::new(value.value().0) == path {
                    found = Some(id.value());
                    break;
                }
            }
            if let Some(id) = found {
                table.remove(id)?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_projects_persist_and_reopening_moves_project_to_top() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("data/user.redb");
        let history = History(db.clone());
        assert!(history.list().unwrap().is_empty());
        let a = Project::create(&temp.path().join("a"), "Árvore").unwrap();
        let b = Project::create(&temp.path().join("b"), "Barco").unwrap();
        history.record(&a).unwrap();
        history.record(&b).unwrap();
        assert_eq!(history.list().unwrap()[0].path, b.root());
        let reopened = Project::open(&temp.path().join("./a")).unwrap();
        history.record(&reopened).unwrap();

        let history = History(db);
        let entries = history.list().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "Árvore");
        assert_eq!(entries[0].path, reopened.root());
        history.forget(reopened.root()).unwrap();
        assert_eq!(history.list().unwrap()[0].path, b.root());
        assert!(Project::open(reopened.root()).is_ok());
    }

    #[test]
    fn history_keeps_only_the_most_recent_entries() {
        let temp = tempfile::tempdir().unwrap();
        let history = History(temp.path().join("user.redb"));
        for i in 0..=LIMIT {
            let name = i.to_string();
            history
                .record(&Project::create(&temp.path().join(&name), &name).unwrap())
                .unwrap();
        }
        let entries = history.list().unwrap();
        assert_eq!(entries.len(), LIMIT);
        assert_eq!(entries[0].name, LIMIT.to_string());
        assert_eq!(entries.last().unwrap().name, "1");
    }
}
