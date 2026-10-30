//! SQLite storage for diagrams. One database holds every diagram's current record (saved after
//! every change) and its versions (saved when the user asks). It knows nothing about diagram
//! types: type-specific fields live in JSON `props` columns.
//!
//! The GUI and the MCP server may open the same file at once. Every write takes the revision
//! it expects, so a change made elsewhere in the meantime fails with [`StoreError::Conflict`]
//! rather than being overwritten.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod locations;

use hiero_core::file::{diagram_from_json, diagram_to_json};
use hiero_core::history::{DiagramSnapshot, content_hash};
use hiero_core::model::{
    ConnectionId, DiagramId, DiagramMeta, DiagramTypeId, ElementId, now_rfc3339,
};
use hiero_core::stored::{StoredConnection, StoredContent, StoredDiagram, StoredElement};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use rusqlite_migration::{M, Migrations};
use serde_json::{Map, Value};

/// What went wrong reading or writing the store.
#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Migration(String),
    /// No diagram (or version) with that id.
    NotFound,
    /// The diagram changed since it was read, such as by an agent.
    Conflict,
    /// An import over a diagram of another type.
    TypeMismatch {
        name: String,
        existing: DiagramTypeId,
        incoming: DiagramTypeId,
    },
    /// Stored data that couldn't be read.
    Corrupt(String),
    /// A file that isn't an Arch Hiero database, such as another app's.
    NotOurs(PathBuf),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(e) => write!(f, "The database couldn't be used: {e}"),
            Self::Migration(e) => write!(f, "The database couldn't be upgraded: {e}"),
            Self::NotFound => f.write_str("That diagram doesn't exist. It may have been deleted."),
            Self::Conflict => {
                f.write_str("The diagram was changed somewhere else in the meantime.")
            }
            Self::TypeMismatch {
                name,
                existing,
                incoming,
            } => write!(
                f,
                "“{name}” is a {} diagram, but the file holds a {} diagram.",
                existing.label().to_lowercase(),
                incoming.label().to_lowercase()
            ),
            Self::Corrupt(e) => write!(f, "Stored data couldn't be read: {e}"),
            Self::NotOurs(path) => {
                write!(f, "{} isn't an Arch Hiero database.", path.display())
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// A diagram as stored, with the revision to pass back when changing it.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub diagram: StoredDiagram,
    pub revision: i64,
}

/// Whether a diagram's current record matches its latest version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub saved: bool,
    /// The newest version's number. Unset when every version was deleted.
    pub latest: Option<i64>,
}

impl Status {
    /// The version tag: "Release · v4" when the diagram matches its latest version, "Draft · v4"
    /// when it has changed since, or "Draft · no versions".
    pub fn label(self) -> String {
        match (self.saved, self.latest) {
            (true, Some(v)) => format!("Release · v{v}"),
            (false, Some(v)) => format!("Draft · v{v}"),
            (_, None) => "Draft · no versions".into(),
        }
    }
}

/// A row in the list of diagrams.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub meta: DiagramMeta,
    pub status: Status,
    pub revision: i64,
}

/// A row in a diagram's history.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionSummary {
    pub version: i64,
    pub saved_at: String,
    pub description: Option<String>,
    /// Whether it has the same content as the current record.
    pub matches_current: bool,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub record: Record,
    /// Whether it replaced a diagram with the same id, rather than adding one.
    pub replaced: bool,
}

const MIGRATION_1: &str = r"
CREATE TABLE diagrams (
  id           TEXT PRIMARY KEY,
  type         TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component')),
  name         TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description  TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors       TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash TEXT NOT NULL,
  revision     INTEGER NOT NULL DEFAULT 0,
  next_version INTEGER NOT NULL DEFAULT 1,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
) STRICT;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;

CREATE TABLE elements (
  diagram_id  TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  id          TEXT NOT NULL,
  ord         INTEGER NOT NULL,
  kind        TEXT NOT NULL,
  label       TEXT NOT NULL,
  description TEXT,
  parent_id   TEXT,
  x REAL NOT NULL,
  y REAL NOT NULL,
  width REAL,
  height REAL,
  props       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),
  PRIMARY KEY (diagram_id, id)
) STRICT;

CREATE TABLE connections (
  diagram_id TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  id         TEXT NOT NULL,
  ord        INTEGER NOT NULL,
  source_id  TEXT NOT NULL,
  target_id  TEXT NOT NULL,
  props      TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),
  PRIMARY KEY (diagram_id, id)
) STRICT;

CREATE TABLE diagram_versions (
  diagram_id   TEXT NOT NULL REFERENCES diagrams(id) ON DELETE CASCADE,
  version      INTEGER NOT NULL CHECK (version >= 1),
  saved_at     TEXT NOT NULL,
  description  TEXT CHECK (description IS NULL OR length(description) <= 200),
  content_hash TEXT NOT NULL,
  snapshot     TEXT NOT NULL CHECK (json_valid(snapshot)),
  PRIMARY KEY (diagram_id, version)
) STRICT;

CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
";

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(MIGRATION_1)])
}

/// Where the database lives: the platform's data directory, such as
/// `~/Library/Application Support/ArchHiero/arch-hiero.sqlite3` on macOS.
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "ArchHiero")
        .map(|dirs| dirs.data_dir().join("arch-hiero.sqlite3"))
}

/// The sample diagrams, one of each type, which the diagram list offers while it's empty.
pub fn seed_diagrams() -> Vec<StoredDiagram> {
    let seed: Value = serde_json::from_str(include_str!("seed.json")).expect("the seed is JSON");
    seed.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|d| diagram_from_json(d).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a database is empty or already holds diagrams. A file that isn't SQLite at all
/// fails the query, so it isn't ours either.
fn is_ours(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT count(*) = 0 OR count(*) FILTER (WHERE name = 'diagrams') = 1
         FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )
    .unwrap_or(false)
}

/// The diagram store.
pub struct Store {
    conn: Connection,
}

fn colors_json(colors: &BTreeMap<String, String>) -> Option<String> {
    (!colors.is_empty()).then(|| serde_json::to_string(colors).expect("colours serialise"))
}

fn props_json(props: &Map<String, Value>) -> String {
    serde_json::to_string(props).expect("props serialise")
}

fn read_props(text: &str) -> Result<Map<String, Value>> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(StoreError::Corrupt(
            "properties aren't a JSON object".into(),
        )),
    }
}

fn read_type(text: &str) -> Result<DiagramTypeId> {
    DiagramTypeId::parse(text).ok_or_else(|| StoreError::Corrupt(format!("unknown type {text}")))
}

impl Store {
    /// Opens (creating if need be) the database at `path`, upgrading its schema.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| {
                StoreError::Migration(format!("can't create {}: {e}", dir.display()))
            })?;
        }
        let conn = Connection::open(path)?;
        // Another app's database would otherwise get our tables added to it.
        if !is_ours(&conn) {
            return Err(StoreError::NotOurs(path.to_owned()));
        }
        Self::setup(conn)
    }

    /// A database that lives only in memory, for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations()
            .to_latest(&mut conn)
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        Ok(Self { conn })
    }

    /// A counter that changes when another connection commits, such as the MCP server's.
    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }

    /// Adds the sample diagrams when there are no diagrams yet, so they're never added twice.
    /// Returns how many were added.
    pub fn load_samples(&mut self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM diagrams", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(0);
        }
        let seeds = seed_diagrams();
        let n = seeds.len();
        for diagram in seeds {
            self.create(&diagram)?;
        }
        Ok(n)
    }

    /// Every diagram, sorted by name, with its Release/Draft status.
    pub fn list(&self) -> Result<Vec<Summary>> {
        let mut statement = self.conn.prepare(
            "SELECT d.id, d.type, d.name, d.description, d.created_at, d.updated_at, d.revision,
                    d.content_hash,
                    (SELECT v.content_hash FROM diagram_versions v WHERE v.diagram_id = d.id
                     ORDER BY v.version DESC LIMIT 1),
                    (SELECT MAX(v.version) FROM diagram_versions v WHERE v.diagram_id = d.id)
             FROM diagrams d ORDER BY d.name COLLATE NOCASE, d.id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<i64>>(9)?,
            ))
        })?;
        rows.map(|row| {
            let (id, ty, name, description, created, updated, revision, hash, latest_hash, latest) =
                row?;
            Ok(Summary {
                meta: DiagramMeta {
                    id: DiagramId(id),
                    diagram_type: read_type(&ty)?,
                    name,
                    description,
                    created_at: created,
                    updated_at: updated,
                },
                status: Status {
                    saved: latest_hash.as_deref() == Some(hash.as_str()),
                    latest,
                },
                revision,
            })
        })
        .collect()
    }

    /// A diagram's current record.
    pub fn get(&self, id: &DiagramId) -> Result<Option<Record>> {
        read_record(&self.conn, id)
    }

    /// A diagram's Release/Draft status.
    pub fn status(&self, id: &DiagramId) -> Result<Status> {
        status_of(&self.conn, id)
    }

    /// Adds a diagram, recording it as version 1.
    pub fn create(&mut self, diagram: &StoredDiagram) -> Result<Record> {
        let tx = self.conn.transaction()?;
        let now = now_rfc3339();
        let mut diagram = diagram.clone();
        if diagram.meta.created_at.is_empty() {
            diagram.meta.created_at.clone_from(&now);
        }
        diagram.meta.updated_at.clone_from(&now);
        insert_diagram(&tx, &diagram)?;
        insert_version(&tx, &diagram, None)?;
        let record = read_record(&tx, &diagram.meta.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(record)
    }

    /// Replaces a diagram's current record with `diagram`: its name, description, colours,
    /// and content. The type and creation date are kept. Fails with `Conflict` when
    /// `expected_revision` is given and the stored one differs.
    pub fn save(
        &mut self,
        diagram: &StoredDiagram,
        expected_revision: Option<i64>,
    ) -> Result<Record> {
        let tx = self.conn.transaction()?;
        let current = read_record(&tx, &diagram.meta.id)?.ok_or(StoreError::NotFound)?;
        if expected_revision.is_some_and(|r| r != current.revision) {
            return Err(StoreError::Conflict);
        }
        let mut next = current.diagram.clone();
        DiagramSnapshot::of(diagram).apply_to(&mut next);
        write_record(&tx, &next, current.revision)?;
        let record = read_record(&tx, &next.meta.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(record)
    }

    /// Deletes a diagram and its versions.
    pub fn delete(&mut self, id: &DiagramId) -> Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM diagrams WHERE id = ?1", params![id.0])?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Saves a diagram from a file: it replaces the diagram with the same id, every field of
    /// it, or else is added with that id. An existing diagram of another type is refused.
    pub fn import(&mut self, diagram: &StoredDiagram) -> Result<Imported> {
        let existing = self.get(&diagram.meta.id)?;
        match existing {
            Some(existing) => {
                if existing.diagram.meta.diagram_type != diagram.meta.diagram_type {
                    return Err(StoreError::TypeMismatch {
                        name: existing.diagram.meta.name,
                        existing: existing.diagram.meta.diagram_type,
                        incoming: diagram.meta.diagram_type,
                    });
                }
                let record = self.save(diagram, None)?;
                Ok(Imported {
                    record,
                    replaced: true,
                })
            }
            None => {
                let mut fresh = diagram.clone();
                fresh.meta.created_at = String::new();
                let record = self.create(&fresh)?;
                Ok(Imported {
                    record,
                    replaced: false,
                })
            }
        }
    }

    /// Records the current record as a new version. Returns its number, or `None` when the
    /// latest version already has the same content.
    pub fn save_version(
        &mut self,
        id: &DiagramId,
        description: Option<&str>,
    ) -> Result<Option<i64>> {
        let tx = self.conn.transaction()?;
        let record = read_record(&tx, id)?.ok_or(StoreError::NotFound)?;
        if status_of(&tx, id)?.saved {
            return Ok(None);
        }
        let version = insert_version(&tx, &record.diagram, description)?;
        tx.commit()?;
        Ok(Some(version))
    }

    /// A diagram's versions, newest first.
    pub fn list_versions(&self, id: &DiagramId) -> Result<Vec<VersionSummary>> {
        let hash: Option<String> = self
            .conn
            .query_row(
                "SELECT content_hash FROM diagrams WHERE id = ?1",
                params![id.0],
                |r| r.get(0),
            )
            .optional()?;
        let hash = hash.ok_or(StoreError::NotFound)?;
        let mut statement = self.conn.prepare(
            "SELECT version, saved_at, description, content_hash FROM diagram_versions
             WHERE diagram_id = ?1 ORDER BY version DESC",
        )?;
        let rows = statement.query_map(params![id.0], |row| {
            Ok(VersionSummary {
                version: row.get(0)?,
                saved_at: row.get(1)?,
                description: row.get(2)?,
                matches_current: row.get::<_, String>(3)? == hash,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// A version's content, as a diagram with the diagram's id and type.
    pub fn get_version(&self, id: &DiagramId, version: i64) -> Result<Option<StoredDiagram>> {
        let snapshot: Option<String> = self
            .conn
            .query_row(
                "SELECT snapshot FROM diagram_versions WHERE diagram_id = ?1 AND version = ?2",
                params![id.0, version],
                |r| r.get(0),
            )
            .optional()?;
        snapshot
            .map(|text| {
                let value: Value =
                    serde_json::from_str(&text).map_err(|e| StoreError::Corrupt(e.to_string()))?;
                diagram_from_json(&value).map_err(|e| StoreError::Corrupt(e.0))
            })
            .transpose()
    }

    /// Replaces the current record with version `version`'s content and records that as a new
    /// version, described as `description` or "Reverted to vN". Later versions are kept.
    /// Returns the new version's number.
    pub fn revert_to_version(
        &mut self,
        id: &DiagramId,
        version: i64,
        description: Option<&str>,
    ) -> Result<(Record, i64)> {
        let old = self.get_version(id, version)?.ok_or(StoreError::NotFound)?;
        let tx = self.conn.transaction()?;
        let current = read_record(&tx, id)?.ok_or(StoreError::NotFound)?;
        let mut next = current.diagram.clone();
        DiagramSnapshot::of(&old).apply_to(&mut next);
        write_record(&tx, &next, current.revision)?;
        let default = format!("Reverted to v{version}");
        let new_version = insert_version(&tx, &next, Some(description.unwrap_or(&default)))?;
        let record = read_record(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok((record, new_version))
    }

    /// Deletes one version. Its number isn't used again.
    pub fn delete_version(&mut self, id: &DiagramId, version: i64) -> Result<()> {
        let changed = self.conn.execute(
            "DELETE FROM diagram_versions WHERE diagram_id = ?1 AND version = ?2",
            params![id.0, version],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

fn status_of(conn: &Connection, id: &DiagramId) -> Result<Status> {
    conn.query_row(
        "SELECT d.content_hash = (SELECT v.content_hash FROM diagram_versions v
                                  WHERE v.diagram_id = d.id ORDER BY v.version DESC LIMIT 1),
                (SELECT MAX(v.version) FROM diagram_versions v WHERE v.diagram_id = d.id)
         FROM diagrams d WHERE d.id = ?1",
        params![id.0],
        |row| {
            Ok(Status {
                saved: row.get::<_, Option<bool>>(0)?.unwrap_or(false),
                latest: row.get(1)?,
            })
        },
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

fn read_record(conn: &Connection, id: &DiagramId) -> Result<Option<Record>> {
    let row = conn
        .query_row(
            "SELECT type, name, description, colors, created_at, updated_at, revision
             FROM diagrams WHERE id = ?1",
            params![id.0],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((ty, name, description, colors, created_at, updated_at, revision)) = row else {
        return Ok(None);
    };
    let colors: BTreeMap<String, String> = match colors {
        Some(text) => {
            serde_json::from_str(&text).map_err(|e| StoreError::Corrupt(e.to_string()))?
        }
        None => BTreeMap::new(),
    };
    let mut elements = Vec::new();
    {
        let mut statement = conn.prepare_cached(
            "SELECT id, kind, label, description, parent_id, x, y, width, height, props
             FROM elements WHERE diagram_id = ?1 ORDER BY ord",
        )?;
        let mut rows = statement.query(params![id.0])?;
        while let Some(row) = rows.next()? {
            elements.push(StoredElement {
                id: ElementId(row.get(0)?),
                kind: row.get(1)?,
                label: row.get(2)?,
                description: row.get(3)?,
                parent_id: row.get::<_, Option<String>>(4)?.map(ElementId),
                x: row.get(5)?,
                y: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                props: read_props(&row.get::<_, String>(9)?)?,
            });
        }
    }
    let mut connections = Vec::new();
    {
        let mut statement = conn.prepare_cached(
            "SELECT id, source_id, target_id, props FROM connections
             WHERE diagram_id = ?1 ORDER BY ord",
        )?;
        let mut rows = statement.query(params![id.0])?;
        while let Some(row) = rows.next()? {
            connections.push(StoredConnection {
                id: ConnectionId(row.get(0)?),
                source_id: ElementId(row.get(1)?),
                target_id: ElementId(row.get(2)?),
                props: read_props(&row.get::<_, String>(3)?)?,
            });
        }
    }
    Ok(Some(Record {
        diagram: StoredDiagram {
            meta: DiagramMeta {
                id: id.clone(),
                diagram_type: read_type(&ty)?,
                name,
                description,
                created_at,
                updated_at,
            },
            colors,
            content: StoredContent {
                elements,
                connections,
            },
        },
        revision,
    }))
}

fn insert_diagram(tx: &Transaction<'_>, diagram: &StoredDiagram) -> Result<()> {
    let m = &diagram.meta;
    tx.execute(
        "INSERT INTO diagrams (id, type, name, description, colors, content_hash, revision,
                               next_version, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 1, ?7, ?8)",
        params![
            m.id.0,
            m.diagram_type.as_str(),
            m.name,
            m.description,
            colors_json(&diagram.colors),
            content_hash(diagram),
            m.created_at,
            m.updated_at
        ],
    )?;
    insert_content(tx, &m.id, &diagram.content)
}

fn insert_content(tx: &Transaction<'_>, id: &DiagramId, content: &StoredContent) -> Result<()> {
    let mut element = tx.prepare_cached(
        "INSERT INTO elements (diagram_id, id, ord, kind, label, description, parent_id, x, y,
                               width, height, props)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;
    for (ord, e) in content.elements.iter().enumerate() {
        element.execute(params![
            id.0,
            e.id.0,
            ord as i64,
            e.kind,
            e.label,
            e.description,
            e.parent_id.as_ref().map(|p| &p.0),
            e.x,
            e.y,
            e.width,
            e.height,
            props_json(&e.props)
        ])?;
    }
    let mut connection = tx.prepare_cached(
        "INSERT INTO connections (diagram_id, id, ord, source_id, target_id, props)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (ord, c) in content.connections.iter().enumerate() {
        connection.execute(params![
            id.0,
            c.id.0,
            ord as i64,
            c.source_id.0,
            c.target_id.0,
            props_json(&c.props)
        ])?;
    }
    Ok(())
}

/// Writes `diagram` over the current record, which is at `revision`.
fn write_record(tx: &Transaction<'_>, diagram: &StoredDiagram, revision: i64) -> Result<()> {
    let m = &diagram.meta;
    let changed = tx.execute(
        "UPDATE diagrams SET name = ?2, description = ?3, colors = ?4, content_hash = ?5,
                revision = revision + 1, updated_at = ?6
         WHERE id = ?1 AND revision = ?7",
        params![
            m.id.0,
            m.name,
            m.description,
            colors_json(&diagram.colors),
            content_hash(diagram),
            now_rfc3339(),
            revision
        ],
    )?;
    if changed == 0 {
        return Err(StoreError::Conflict);
    }
    tx.execute(
        "DELETE FROM elements WHERE diagram_id = ?1",
        params![m.id.0],
    )?;
    tx.execute(
        "DELETE FROM connections WHERE diagram_id = ?1",
        params![m.id.0],
    )?;
    insert_content(tx, &m.id, &diagram.content)
}

/// Records `diagram` as the next version, returning its number.
fn insert_version(
    tx: &Transaction<'_>,
    diagram: &StoredDiagram,
    description: Option<&str>,
) -> Result<i64> {
    let id = &diagram.meta.id;
    let version: i64 = tx.query_row(
        "UPDATE diagrams SET next_version = next_version + 1 WHERE id = ?1
         RETURNING next_version - 1",
        params![id.0],
        |r| r.get(0),
    )?;
    let description = description.map(str::trim).filter(|d| !d.is_empty());
    tx.execute(
        "INSERT INTO diagram_versions (diagram_id, version, saved_at, description, content_hash,
                                       snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id.0,
            version,
            now_rfc3339(),
            description,
            content_hash(diagram),
            serde_json::to_string(&diagram_to_json(diagram)).expect("snapshots serialise")
        ],
    )?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diagram(id: &str, ty: DiagramTypeId) -> StoredDiagram {
        StoredDiagram {
            meta: DiagramMeta {
                id: id.into(),
                diagram_type: ty,
                name: "Overview".into(),
                description: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            colors: BTreeMap::new(),
            content: StoredContent {
                elements: vec![
                    StoredElement::from_json(&json!({"id": "a", "kind": "actor", "label": "A", "x": 40, "y": 40})).unwrap(),
                    StoredElement::from_json(&json!({"id": "b", "kind": "application", "label": "B", "description": "d", "x": 240, "y": 40, "cloudHosted": true})).unwrap(),
                ],
                connections: vec![
                    StoredConnection::from_json(&json!({"id": "c", "sourceId": "a", "targetId": "b", "label": "uses"})).unwrap(),
                ],
            },
        }
    }

    #[test]
    fn create_list_get_round_trip() {
        let mut store = Store::open_in_memory().unwrap();
        let created = store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        assert_eq!(created.revision, 0);
        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].status,
            Status {
                saved: true,
                latest: Some(1)
            }
        );
        let got = store.get(&"d1".into()).unwrap().unwrap();
        assert_eq!(
            got.diagram.content,
            diagram("d1", DiagramTypeId::Conceptual).content
        );
        assert!(!got.diagram.meta.created_at.is_empty());
    }

    #[test]
    fn saving_changes_status_and_checks_revisions() {
        let mut store = Store::open_in_memory().unwrap();
        let record = store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        let mut changed = record.diagram.clone();
        changed.meta.name = "Renamed".into();
        let saved = store.save(&changed, Some(record.revision)).unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(store.status(&"d1".into()).unwrap().label(), "Draft · v1");
        // A stale revision is refused.
        assert!(matches!(
            store.save(&changed, Some(0)),
            Err(StoreError::Conflict)
        ));
        // Changing it back matches version 1 again.
        changed.meta.name = "Overview".into();
        store.save(&changed, Some(1)).unwrap();
        assert!(store.status(&"d1".into()).unwrap().saved);
    }

    #[test]
    fn versions_revert_forwards_and_numbers_are_not_reused() {
        let mut store = Store::open_in_memory().unwrap();
        let record = store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        let mut changed = record.diagram.clone();
        changed.content.elements.pop();
        changed.content.connections.clear();
        store.save(&changed, None).unwrap();
        assert_eq!(
            store.save_version(&"d1".into(), Some("  Fewer  ")).unwrap(),
            Some(2)
        );
        assert_eq!(
            store.save_version(&"d1".into(), None).unwrap(),
            None,
            "already saved"
        );
        let (reverted, version) = store.revert_to_version(&"d1".into(), 1, None).unwrap();
        assert_eq!(version, 3);
        assert_eq!(reverted.diagram.content.elements.len(), 2);
        let versions = store.list_versions(&"d1".into()).unwrap();
        let numbers: Vec<i64> = versions.iter().map(|v| v.version).collect();
        assert_eq!(numbers, [3, 2, 1]);
        assert_eq!(versions[0].description.as_deref(), Some("Reverted to v1"));
        assert_eq!(versions[1].description.as_deref(), Some("Fewer"));
        assert!(versions[0].matches_current && versions[2].matches_current);
        assert!(store.status(&"d1".into()).unwrap().saved);
        // Delete the newest; the next save is v4, not v3.
        store.delete_version(&"d1".into(), 3).unwrap();
        store.delete_version(&"d1".into(), 2).unwrap();
        let mut again = store.get(&"d1".into()).unwrap().unwrap().diagram;
        again.meta.name = "Again".into();
        store.save(&again, None).unwrap();
        assert_eq!(store.save_version(&"d1".into(), None).unwrap(), Some(4));
    }

    #[test]
    fn every_version_deleted_reads_unsaved() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        store.delete_version(&"d1".into(), 1).unwrap();
        assert_eq!(
            store.status(&"d1".into()).unwrap().label(),
            "Draft · no versions"
        );
    }

    #[test]
    fn imports_replace_or_add_and_refuse_type_changes() {
        let mut store = Store::open_in_memory().unwrap();
        let first = store
            .import(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        assert!(!first.replaced);
        let mut again = diagram("d1", DiagramTypeId::Conceptual);
        again.meta.name = "Imported".into();
        let second = store.import(&again).unwrap();
        assert!(second.replaced);
        assert_eq!(second.record.diagram.meta.name, "Imported");
        let error = store
            .import(&diagram("d1", DiagramTypeId::Physical))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "“Imported” is a conceptual diagram, but the file holds a physical diagram."
        );
    }

    #[test]
    fn the_type_cannot_be_changed() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        let result = store
            .conn
            .execute("UPDATE diagrams SET type = 'physical' WHERE id = 'd1'", []);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("type can't be changed")
        );
    }

    #[test]
    fn deleting_cascades() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        store.delete(&"d1".into()).unwrap();
        let count = |table: &str| -> i64 {
            store
                .conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(
            count("elements") + count("connections") + count("diagram_versions"),
            0
        );
        assert!(matches!(
            store.delete(&"d1".into()),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn samples_load_once() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.load_samples().unwrap(), 3);
        assert_eq!(store.load_samples().unwrap(), 0);
        let names: Vec<String> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|s| s.meta.name)
            .collect();
        assert_eq!(
            names,
            [
                "ARCH Hiero MCP Capabilities",
                "Standard 3 -Tier Application",
                "Website Example - Active - Active"
            ]
        );
    }

    #[test]
    fn two_connections_see_each_others_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite3");
        let mut gui = Store::open(&path).unwrap();
        let mut agent = Store::open(&path).unwrap();
        let record = gui
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        let before = gui.data_version().unwrap();
        let mut changed = record.diagram.clone();
        changed.meta.name = "By agent".into();
        agent.save(&changed, Some(record.revision)).unwrap();
        assert_ne!(gui.data_version().unwrap(), before);
        // The GUI's stale write is refused rather than overwriting the agent's.
        let mut mine = record.diagram.clone();
        mine.meta.name = "By me".into();
        assert!(matches!(
            gui.save(&mine, Some(record.revision)),
            Err(StoreError::Conflict)
        ));
        assert_eq!(
            gui.get(&"d1".into()).unwrap().unwrap().diagram.meta.name,
            "By agent"
        );
    }

    #[test]
    fn opens_only_its_own_databases() {
        let dir = tempfile::tempdir().unwrap();
        // A new file, and one already ours, open.
        let ours = dir.path().join("ours.sqlite3");
        drop(Store::open(&ours).unwrap());
        Store::open(&ours).unwrap();
        // Another app's database and a file that isn't SQLite are left alone.
        let theirs = dir.path().join("theirs.sqlite3");
        Connection::open(&theirs)
            .unwrap()
            .execute_batch("CREATE TABLE notes (text TEXT)")
            .unwrap();
        assert!(matches!(Store::open(&theirs), Err(StoreError::NotOurs(_))));
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, "not a database, just some words in a file").unwrap();
        assert!(matches!(Store::open(&text), Err(StoreError::NotOurs(_))));
        assert_eq!(
            std::fs::read_to_string(&text).unwrap(),
            "not a database, just some words in a file"
        );
    }
}
