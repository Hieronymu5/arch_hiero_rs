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

use hiero_core::file::{SubDiagram, diagram_from_json, diagram_to_json};
use hiero_core::geometry::Size;
use hiero_core::history::{DiagramSnapshot, content_hash};
use hiero_core::model::{
    ConnectionId, DiagramId, DiagramMeta, DiagramTypeId, ElementId, LineWeight, now_rfc3339,
};
use hiero_core::stored::{StoredConnection, StoredContent, StoredDiagram, StoredElement};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use rusqlite_migration::{M, Migrations};
use serde_json::{Map, Value};

/// The element property that names the diagram an element opens.
const SUB_DIAGRAM: &str = "subDiagramId";

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

    /// Which version an export of the current diagram shows, for its legend: "Version 4"
    /// when it matches its latest version, "Draft after version 4" when it has changed since,
    /// or "Draft" before its first.
    pub fn version_text(self) -> String {
        match (self.saved, self.latest) {
            (true, Some(v)) => format!("Version {v}"),
            (false, Some(v)) => format!("Draft after version {v}"),
            (_, None) => "Draft".into(),
        }
    }
}

/// A row in the list of diagrams.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub meta: DiagramMeta,
    pub status: Status,
    pub revision: i64,
    /// The diagram this one was added as a sub diagram of, if any. Deleting that one deletes
    /// this one with it.
    pub parent: Option<DiagramId>,
    /// The kind of diagram within its type, such as a C4 diagram's level, when it isn't the
    /// type's first.
    pub subtype: Option<String>,
}

impl Summary {
    /// The type's name with the diagram's sub type, such as "C4 - Container".
    pub fn type_label(&self) -> String {
        self.meta.diagram_type.label_with(self.subtype.as_deref())
    }
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

/// How thick elements' lines are; unset means the type's default.
const MIGRATION_2: &str = r"
ALTER TABLE diagrams ADD COLUMN line_weight TEXT
  CHECK (line_weight IS NULL OR line_weight IN ('thin', 'thick'));
";

/// Allows a medium line weight. SQLite can't change a column's CHECK, so the column is
/// replaced, keeping its values.
const MIGRATION_3: &str = r"
ALTER TABLE diagrams ADD COLUMN line_weight_3 TEXT
  CHECK (line_weight_3 IS NULL OR line_weight_3 IN ('thin', 'medium', 'thick'));
UPDATE diagrams SET line_weight_3 = line_weight;
ALTER TABLE diagrams DROP COLUMN line_weight;
ALTER TABLE diagrams RENAME COLUMN line_weight_3 TO line_weight;
";

/// The diagram each diagram is a sub diagram of. Deleting a diagram deletes its sub diagrams,
/// and theirs. It was filled in here from the elements' `subDiagramId`; since then it's set
/// only when a diagram is added as a sub diagram, and elements' links don't change it.
const MIGRATION_4: &str = r"
ALTER TABLE diagrams ADD COLUMN parent_diagram_id TEXT
  REFERENCES diagrams(id) ON DELETE CASCADE;
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
CREATE INDEX elements_by_sub_diagram ON elements(json_extract(props, '$.subDiagramId'))
  WHERE json_extract(props, '$.subDiagramId') IS NOT NULL;
UPDATE diagrams SET parent_diagram_id = (
  SELECT MIN(e.diagram_id) FROM elements e
  WHERE json_extract(e.props, '$.subDiagramId') = diagrams.id AND e.diagram_id <> diagrams.id
);
";

/// The size of each diagram's drawing area; unset means the default size.
const MIGRATION_5: &str = r"
ALTER TABLE diagrams ADD COLUMN canvas_width REAL CHECK (canvas_width IS NULL OR canvas_width > 0);
ALTER TABLE diagrams ADD COLUMN canvas_height REAL
  CHECK (canvas_height IS NULL OR canvas_height > 0);
";

/// Allows the C4 model's three types. SQLite can't change a column's CHECK, so the table is
/// rebuilt, keeping its rows; foreign keys are off while it is, so the rows that refer to
/// diagrams stay. Its trigger and indexes go with the old table and are made again.
const MIGRATION_6: &str = r"
CREATE TABLE diagrams_6 (
  id                TEXT PRIMARY KEY,
  type              TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component',
                                                  'c4-context', 'c4-container', 'c4-component')),
  name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description       TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors            TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash      TEXT NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 0,
  next_version      INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  line_weight       TEXT CHECK (line_weight IS NULL OR line_weight IN ('thin', 'medium', 'thick')),
  parent_diagram_id TEXT REFERENCES diagrams(id) ON DELETE CASCADE,
  canvas_width      REAL CHECK (canvas_width IS NULL OR canvas_width > 0),
  canvas_height     REAL CHECK (canvas_height IS NULL OR canvas_height > 0)
) STRICT;
INSERT INTO diagrams_6 (id, type, name, description, colors, content_hash, revision,
                        next_version, created_at, updated_at, line_weight, parent_diagram_id,
                        canvas_width, canvas_height)
  SELECT id, type, name, description, colors, content_hash, revision, next_version, created_at,
         updated_at, line_weight, parent_diagram_id, canvas_width, canvas_height
  FROM diagrams;
DROP TABLE diagrams;
ALTER TABLE diagrams_6 RENAME TO diagrams;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;
CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
";

/// Replaces the three C4 types with one, whose diagrams choose their level as a sub type kept
/// in a new column. Diagrams of the three old types are deleted, with their versions; a sub
/// diagram of one stays, as a diagram of its own. The table is rebuilt to change its CHECK, as
/// migration 6 did.
const MIGRATION_7: &str = r"
UPDATE diagrams SET parent_diagram_id = NULL
  WHERE parent_diagram_id IN (SELECT id FROM diagrams
                              WHERE type IN ('c4-context', 'c4-container', 'c4-component'));
DELETE FROM elements WHERE diagram_id IN (
  SELECT id FROM diagrams WHERE type IN ('c4-context', 'c4-container', 'c4-component'));
DELETE FROM connections WHERE diagram_id IN (
  SELECT id FROM diagrams WHERE type IN ('c4-context', 'c4-container', 'c4-component'));
DELETE FROM diagram_versions WHERE diagram_id IN (
  SELECT id FROM diagrams WHERE type IN ('c4-context', 'c4-container', 'c4-component'));
DELETE FROM diagrams WHERE type IN ('c4-context', 'c4-container', 'c4-component');

CREATE TABLE diagrams_7 (
  id                TEXT PRIMARY KEY,
  type              TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component',
                                                  'c4')),
  name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description       TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors            TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash      TEXT NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 0,
  next_version      INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  line_weight       TEXT CHECK (line_weight IS NULL OR line_weight IN ('thin', 'medium', 'thick')),
  parent_diagram_id TEXT REFERENCES diagrams(id) ON DELETE CASCADE,
  canvas_width      REAL CHECK (canvas_width IS NULL OR canvas_width > 0),
  canvas_height     REAL CHECK (canvas_height IS NULL OR canvas_height > 0),
  subtype           TEXT CHECK (subtype IS NULL OR length(subtype) > 0)
) STRICT;
INSERT INTO diagrams_7 (id, type, name, description, colors, content_hash, revision,
                        next_version, created_at, updated_at, line_weight, parent_diagram_id,
                        canvas_width, canvas_height)
  SELECT id, type, name, description, colors, content_hash, revision, next_version, created_at,
         updated_at, line_weight, parent_diagram_id, canvas_width, canvas_height
  FROM diagrams;
DROP TABLE diagrams;
ALTER TABLE diagrams_7 RENAME TO diagrams;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;
CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
";

/// Allows the flowchart type. The table is rebuilt to change its CHECK, as migration 6 did.
const MIGRATION_8: &str = r"
CREATE TABLE diagrams_8 (
  id                TEXT PRIMARY KEY,
  type              TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component',
                                                  'c4', 'flowchart')),
  name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description       TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors            TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash      TEXT NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 0,
  next_version      INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  line_weight       TEXT CHECK (line_weight IS NULL OR line_weight IN ('thin', 'medium', 'thick')),
  parent_diagram_id TEXT REFERENCES diagrams(id) ON DELETE CASCADE,
  canvas_width      REAL CHECK (canvas_width IS NULL OR canvas_width > 0),
  canvas_height     REAL CHECK (canvas_height IS NULL OR canvas_height > 0),
  subtype           TEXT CHECK (subtype IS NULL OR length(subtype) > 0)
) STRICT;
INSERT INTO diagrams_8 (id, type, name, description, colors, content_hash, revision,
                        next_version, created_at, updated_at, line_weight, parent_diagram_id,
                        canvas_width, canvas_height, subtype)
  SELECT id, type, name, description, colors, content_hash, revision, next_version, created_at,
         updated_at, line_weight, parent_diagram_id, canvas_width, canvas_height, subtype
  FROM diagrams;
DROP TABLE diagrams;
ALTER TABLE diagrams_8 RENAME TO diagrams;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;
CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
";

/// Allows the sequence type. The table is rebuilt to change its CHECK, as migration 8 did.
const MIGRATION_9: &str = r"
CREATE TABLE diagrams_9 (
  id                TEXT PRIMARY KEY,
  type              TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component',
                                                  'c4', 'flowchart', 'sequence')),
  name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description       TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors            TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash      TEXT NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 0,
  next_version      INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  line_weight       TEXT CHECK (line_weight IS NULL OR line_weight IN ('thin', 'medium', 'thick')),
  parent_diagram_id TEXT REFERENCES diagrams(id) ON DELETE CASCADE,
  canvas_width      REAL CHECK (canvas_width IS NULL OR canvas_width > 0),
  canvas_height     REAL CHECK (canvas_height IS NULL OR canvas_height > 0),
  subtype           TEXT CHECK (subtype IS NULL OR length(subtype) > 0)
) STRICT;
INSERT INTO diagrams_9 (id, type, name, description, colors, content_hash, revision,
                        next_version, created_at, updated_at, line_weight, parent_diagram_id,
                        canvas_width, canvas_height, subtype)
  SELECT id, type, name, description, colors, content_hash, revision, next_version, created_at,
         updated_at, line_weight, parent_diagram_id, canvas_width, canvas_height, subtype
  FROM diagrams;
DROP TABLE diagrams;
ALTER TABLE diagrams_9 RENAME TO diagrams;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;
CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
";

/// Allows the high level conceptual type. The table is rebuilt to change its CHECK, as
/// migration 8 did.
const MIGRATION_10: &str = r"
CREATE TABLE diagrams_10 (
  id                TEXT PRIMARY KEY,
  type              TEXT NOT NULL CHECK (type IN ('conceptual', 'physical', 'deployed-component',
                                                  'c4', 'flowchart', 'sequence',
                                                  'high-level-conceptual')),
  name              TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
  description       TEXT CHECK (description IS NULL OR length(description) <= 1000),
  colors            TEXT CHECK (colors IS NULL OR json_valid(colors)),
  content_hash      TEXT NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 0,
  next_version      INTEGER NOT NULL DEFAULT 1,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL,
  line_weight       TEXT CHECK (line_weight IS NULL OR line_weight IN ('thin', 'medium', 'thick')),
  parent_diagram_id TEXT REFERENCES diagrams(id) ON DELETE CASCADE,
  canvas_width      REAL CHECK (canvas_width IS NULL OR canvas_width > 0),
  canvas_height     REAL CHECK (canvas_height IS NULL OR canvas_height > 0),
  subtype           TEXT CHECK (subtype IS NULL OR length(subtype) > 0)
) STRICT;
INSERT INTO diagrams_10 (id, type, name, description, colors, content_hash, revision,
                         next_version, created_at, updated_at, line_weight, parent_diagram_id,
                         canvas_width, canvas_height, subtype)
  SELECT id, type, name, description, colors, content_hash, revision, next_version, created_at,
         updated_at, line_weight, parent_diagram_id, canvas_width, canvas_height, subtype
  FROM diagrams;
DROP TABLE diagrams;
ALTER TABLE diagrams_10 RENAME TO diagrams;

CREATE TRIGGER diagrams_type_is_fixed BEFORE UPDATE OF type ON diagrams
WHEN NEW.type <> OLD.type
BEGIN SELECT RAISE(ABORT, 'A diagram''s type can''t be changed.'); END;
CREATE INDEX diagrams_by_name ON diagrams(name COLLATE NOCASE);
CREATE INDEX diagrams_by_parent ON diagrams(parent_diagram_id);
";

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(MIGRATION_1),
        M::up(MIGRATION_2),
        M::up(MIGRATION_3),
        M::up(MIGRATION_4),
        M::up(MIGRATION_5),
        M::up(MIGRATION_6),
        M::up(MIGRATION_7),
        M::up(MIGRATION_8),
        M::up(MIGRATION_9),
        M::up(MIGRATION_10),
    ])
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

/// `diagrams`, each with the diagram it's a sub diagram of: the first other one with an
/// element that opens it. They're put in an order that adds each after its parent; one whose
/// parents would loop has none.
fn with_parents(diagrams: Vec<StoredDiagram>) -> Vec<(StoredDiagram, Option<DiagramId>)> {
    let mut parent_of: BTreeMap<DiagramId, DiagramId> = BTreeMap::new();
    for d in &diagrams {
        let opened = d
            .content
            .elements
            .iter()
            .filter_map(|e| e.props.get(SUB_DIAGRAM)?.as_str());
        for target in opened {
            let target = DiagramId(target.to_owned());
            if target != d.meta.id && diagrams.iter().any(|o| o.meta.id == target) {
                parent_of.entry(target).or_insert_with(|| d.meta.id.clone());
            }
        }
    }
    let mut pending = diagrams;
    let mut ordered: Vec<(StoredDiagram, Option<DiagramId>)> = Vec::new();
    while !pending.is_empty() {
        let added = |id: &DiagramId| ordered.iter().any(|(d, _)| d.meta.id == *id);
        let ready = pending
            .iter()
            .position(|d| parent_of.get(&d.meta.id).is_none_or(added));
        // Only parents that loop are left: the first is added on its own.
        let (index, parent) = match ready {
            Some(i) => (i, parent_of.get(&pending[i].meta.id).cloned()),
            None => (0, None),
        };
        ordered.push((pending.remove(index), parent));
    }
    ordered
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
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // Off while upgrading, so a migration can rebuild a table others refer to, then
        // checked before they're on again.
        conn.pragma_update(None, "foreign_keys", "OFF")?;
        migrations()
            .to_latest(&mut conn)
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        let broken: i64 =
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        if broken > 0 {
            return Err(StoreError::Migration(format!(
                "{broken} rows refer to rows that don't exist"
            )));
        }
        conn.pragma_update(None, "foreign_keys", "ON")?;
        break_parent_loops(&conn)?;
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
        for (diagram, parent) in with_parents(seeds) {
            self.create_under(&diagram, parent.as_ref())?;
        }
        Ok(n)
    }

    /// Every diagram, sorted by name, with its Release/Draft status and the diagram it belongs to.
    pub fn list(&self) -> Result<Vec<Summary>> {
        let mut statement = self.conn.prepare(
            "SELECT d.id, d.type, d.name, d.description, d.created_at, d.updated_at, d.revision,
                    d.content_hash,
                    (SELECT v.content_hash FROM diagram_versions v WHERE v.diagram_id = d.id
                     ORDER BY v.version DESC LIMIT 1),
                    (SELECT MAX(v.version) FROM diagram_versions v WHERE v.diagram_id = d.id),
                    d.parent_diagram_id, d.subtype
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
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })?;
        rows.map(|row| {
            let (
                id,
                ty,
                name,
                description,
                created,
                updated,
                revision,
                hash,
                latest_hash,
                latest,
                parent,
                subtype,
            ) = row?;
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
                parent: parent.map(DiagramId),
                subtype,
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
        self.create_under(diagram, None)
    }

    /// Adds a diagram as a sub diagram of `parent`, which deleting deletes it with, or on its
    /// own, recording it as version 1.
    pub fn create_under(
        &mut self,
        diagram: &StoredDiagram,
        parent: Option<&DiagramId>,
    ) -> Result<Record> {
        let tx = self.conn.transaction()?;
        if let Some(parent) = parent
            && !exists(&tx, parent)?
        {
            return Err(StoreError::NotFound);
        }
        let now = now_rfc3339();
        let mut diagram = diagram.clone();
        if diagram.meta.created_at.is_empty() {
            diagram.meta.created_at.clone_from(&now);
        }
        diagram.meta.updated_at.clone_from(&now);
        insert_diagram(&tx, &diagram)?;
        if let Some(parent) = parent {
            tx.execute(
                "UPDATE diagrams SET parent_diagram_id = ?2 WHERE id = ?1",
                params![diagram.meta.id.0, parent.0],
            )?;
        }
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

    /// Deletes a diagram and its versions, with its sub diagrams and theirs. Returns every
    /// diagram deleted, the given one first.
    pub fn delete(&mut self, id: &DiagramId) -> Result<Vec<DiagramId>> {
        let tx = self.conn.transaction()?;
        let deleted = descendants(&tx, id)?;
        let changed = tx.execute("DELETE FROM diagrams WHERE id = ?1", params![id.0])?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// A diagram's sub diagrams, and theirs: what deleting it deletes with it.
    pub fn sub_diagrams(&self, id: &DiagramId) -> Result<Vec<DiagramId>> {
        Ok(descendants(&self.conn, id)?.into_iter().skip(1).collect())
    }

    /// A diagram's sub diagrams, and theirs, each with the diagram it's a sub diagram of:
    /// parents before their sub diagrams, and sorted by name among those as deep.
    pub fn sub_diagram_tree(&self, id: &DiagramId) -> Result<Vec<(DiagramId, DiagramId)>> {
        let mut statement = self.conn.prepare(
            "WITH RECURSIVE below(id, parent, depth) AS (
               SELECT id, parent_diagram_id, 0 FROM diagrams WHERE id = ?1
               UNION SELECT d.id, d.parent_diagram_id, b.depth + 1
                 FROM diagrams d JOIN below b ON d.parent_diagram_id = b.id
             )
             SELECT b.id, b.parent FROM below b JOIN diagrams d ON d.id = b.id
             WHERE b.depth > 0
             ORDER BY b.depth, d.name COLLATE NOCASE, b.id",
        )?;
        let rows = statement.query_map(params![id.0], |row| {
            Ok((DiagramId(row.get(0)?), DiagramId(row.get(1)?)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// A diagram's own sub diagrams, not theirs, with their names, sorted by name.
    pub fn children(&self, id: &DiagramId) -> Result<Vec<(DiagramId, String)>> {
        let mut statement = self.conn.prepare(
            "SELECT id, name FROM diagrams WHERE parent_diagram_id = ?1
             ORDER BY name COLLATE NOCASE, id",
        )?;
        let rows = statement.query_map(params![id.0], |row| {
            Ok((DiagramId(row.get(0)?), row.get(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Whether there's a diagram with this id.
    pub fn exists(&self, id: &DiagramId) -> Result<bool> {
        exists(&self.conn, id)
    }

    /// Stops `content`'s elements opening diagrams that don't exist. Returns whether any did.
    pub fn drop_dead_links(&self, content: &mut StoredContent) -> Result<bool> {
        drop_dead_links(&self.conn, content)
    }

    /// Saves a diagram from a file: it replaces the diagram with the same id, every field of
    /// it, or else is added with that id. An existing diagram of another type is refused.
    pub fn import(&mut self, diagram: &StoredDiagram) -> Result<Imported> {
        self.import_under(diagram, None)
    }

    /// Imports a diagram from a file, as `import` does, then the sub diagrams in the file
    /// with it, in order. One that's new is added as a sub diagram of its parent in the file;
    /// one that's here already is replaced and stays where it is. Nothing is imported when
    /// any of them would change a diagram's type. Returns what each import did, the
    /// diagram's first.
    pub fn import_with_sub_diagrams(
        &mut self,
        diagram: &StoredDiagram,
        subs: &[SubDiagram],
    ) -> Result<Vec<Imported>> {
        let all = std::iter::once(diagram).chain(subs.iter().map(|s| &s.diagram));
        for incoming in all {
            self.check_import_type(incoming)?;
        }
        let mut imported = vec![self.import(diagram)?];
        for sub in subs {
            imported.push(self.import_under(&sub.diagram, Some(&sub.parent))?);
        }
        Ok(imported)
    }

    fn check_import_type(&self, diagram: &StoredDiagram) -> Result<Option<Record>> {
        let existing = self.get(&diagram.meta.id)?;
        if let Some(existing) = &existing
            && existing.diagram.meta.diagram_type != diagram.meta.diagram_type
        {
            return Err(StoreError::TypeMismatch {
                name: existing.diagram.meta.name.clone(),
                existing: existing.diagram.meta.diagram_type,
                incoming: diagram.meta.diagram_type,
            });
        }
        Ok(existing)
    }

    /// Imports `diagram`, added under `parent` when it's new.
    fn import_under(
        &mut self,
        diagram: &StoredDiagram,
        parent: Option<&DiagramId>,
    ) -> Result<Imported> {
        match self.check_import_type(diagram)? {
            Some(_) => {
                let record = self.save(diagram, None)?;
                Ok(Imported {
                    record,
                    replaced: true,
                })
            }
            None => {
                let mut fresh = diagram.clone();
                fresh.meta.created_at = String::new();
                let record = self.create_under(&fresh, parent)?;
                Ok(Imported {
                    record,
                    replaced: false,
                })
            }
        }
    }

    /// Records the current record as a new version. Its elements stop opening diagrams that
    /// have been deleted first, in the current record too. Returns the version's number, or
    /// `None` when the latest version already has the same content.
    pub fn save_version(
        &mut self,
        id: &DiagramId,
        description: Option<&str>,
    ) -> Result<Option<i64>> {
        let tx = self.conn.transaction()?;
        let mut record = read_record(&tx, id)?.ok_or(StoreError::NotFound)?;
        if drop_dead_links(&tx, &mut record.diagram.content)? {
            write_record(&tx, &record.diagram, record.revision)?;
        }
        let version = if status_of(&tx, id)?.saved {
            None
        } else {
            Some(insert_version(&tx, &record.diagram, description)?)
        };
        tx.commit()?;
        Ok(version)
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
        drop_dead_links(&tx, &mut next.content)?;
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

/// A diagram and its sub diagrams, and theirs: the diagram first.
fn descendants(conn: &Connection, id: &DiagramId) -> Result<Vec<DiagramId>> {
    let mut statement = conn.prepare(
        "WITH RECURSIVE below(id) AS (
           SELECT id FROM diagrams WHERE id = ?1
           UNION SELECT d.id FROM diagrams d JOIN below b ON d.parent_diagram_id = b.id
         )
         SELECT id FROM below",
    )?;
    let ids = statement
        .query_map(params![id.0], |row| row.get(0).map(DiagramId))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

fn parent_of(conn: &Connection, id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT parent_diagram_id FROM diagrams WHERE id = ?1",
            params![id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

fn exists(conn: &Connection, id: &DiagramId) -> Result<bool> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM diagrams WHERE id = ?1")?
        .exists(params![id.0])?)
}

/// Stops `content`'s elements opening diagrams that don't exist. Returns whether any did.
fn drop_dead_links(conn: &Connection, content: &mut StoredContent) -> Result<bool> {
    let mut dropped = false;
    for element in &mut content.elements {
        let Some(link) = element.props.get(SUB_DIAGRAM) else {
            continue;
        };
        let live = match link.as_str() {
            Some(id) if !id.is_empty() => exists(conn, &DiagramId::from(id))?,
            _ => false,
        };
        if !live {
            element.props.remove(SUB_DIAGRAM);
            dropped = true;
        }
    }
    Ok(dropped)
}

/// Takes the parent away from one diagram in each loop of diagrams under each other, which
/// the migration's fill could make, so deleting a diagram never deletes what it's under.
fn break_parent_loops(conn: &Connection) -> Result<()> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM diagrams WHERE parent_diagram_id IS NOT NULL ORDER BY id")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in ids {
        let mut seen = vec![id.clone()];
        let mut at = id.clone();
        while let Some(parent) = parent_of(conn, &at)? {
            if parent == id {
                conn.execute(
                    "UPDATE diagrams SET parent_diagram_id = NULL WHERE id = ?1",
                    params![id],
                )?;
                break;
            }
            if seen.contains(&parent) {
                break;
            }
            seen.push(parent.clone());
            at = parent;
        }
    }
    Ok(())
}

fn read_record(conn: &Connection, id: &DiagramId) -> Result<Option<Record>> {
    let row = conn
        .query_row(
            "SELECT type, name, description, colors, created_at, updated_at, revision,
                    line_weight, canvas_width, canvas_height, subtype
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
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<f64>>(8)?,
                    row.get::<_, Option<f64>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )
        .optional()?;
    let Some((
        ty,
        name,
        description,
        colors,
        created_at,
        updated_at,
        revision,
        line_weight,
        canvas_width,
        canvas_height,
        subtype,
    )) = row
    else {
        return Ok(None);
    };
    let canvas = match (canvas_width, canvas_height) {
        (Some(width), Some(height)) => Some(Size::new(width, height)),
        (None, None) => None,
        _ => return Err(StoreError::Corrupt("A canvas has only one side.".into())),
    };
    let line_weight = match line_weight {
        Some(text) => Some(
            LineWeight::parse(&text)
                .ok_or_else(|| StoreError::Corrupt(format!("“{text}” isn't a line weight.")))?,
        ),
        None => None,
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
            line_weight,
            canvas,
            subtype,
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
        "INSERT INTO diagrams (id, type, name, description, colors, line_weight, content_hash,
                               revision, next_version, created_at, updated_at,
                               canvas_width, canvas_height, subtype)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 1, ?8, ?9, ?10, ?11, ?12)",
        params![
            m.id.0,
            m.diagram_type.as_str(),
            m.name,
            m.description,
            colors_json(&diagram.colors),
            diagram.line_weight.map(LineWeight::as_str),
            content_hash(diagram),
            m.created_at,
            m.updated_at,
            diagram.canvas.map(|c| c.width),
            diagram.canvas.map(|c| c.height),
            diagram.subtype,
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
        "UPDATE diagrams SET name = ?2, description = ?3, colors = ?4, line_weight = ?5,
                content_hash = ?6, revision = revision + 1, updated_at = ?7,
                canvas_width = ?9, canvas_height = ?10, subtype = ?11
         WHERE id = ?1 AND revision = ?8",
        params![
            m.id.0,
            m.name,
            m.description,
            colors_json(&diagram.colors),
            diagram.line_weight.map(LineWeight::as_str),
            content_hash(diagram),
            now_rfc3339(),
            revision,
            diagram.canvas.map(|c| c.width),
            diagram.canvas.map(|c| c.height),
            diagram.subtype,
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
            line_weight: None,
            canvas: None,
            subtype: None,
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
    fn diagrams_list_the_diagram_they_were_added_under() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        store
            .create_under(
                &diagram("d2", DiagramTypeId::Conceptual),
                Some(&"d1".into()),
            )
            .unwrap();
        let list = store.list().unwrap();
        let parent_of = |id: &str| {
            list.iter()
                .find(|s| s.meta.id.as_str() == id)
                .unwrap()
                .parent
                .clone()
        };
        assert_eq!(parent_of("d1"), None);
        assert_eq!(parent_of("d2"), Some("d1".into()));
        assert_eq!(
            store.children(&"d1".into()).unwrap(),
            [(DiagramId::from("d2"), "Overview".to_owned())]
        );
        assert!(matches!(
            store.create_under(
                &diagram("d3", DiagramTypeId::Conceptual),
                Some(&"gone".into())
            ),
            Err(StoreError::NotFound)
        ));
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
    fn line_weights_are_kept_in_records_and_versions() {
        let mut store = Store::open_in_memory().unwrap();
        let record = store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        let mut thin = record.diagram.clone();
        thin.set_line_weight(LineWeight::Thin);
        store.save(&thin, Some(record.revision)).unwrap();
        let got = store.get(&"d1".into()).unwrap().unwrap().diagram;
        assert_eq!(got.line_weight, Some(LineWeight::Thin));
        assert_eq!(store.status(&"d1".into()).unwrap().label(), "Draft · v1");
        assert_eq!(store.save_version(&"d1".into(), None).unwrap(), Some(2));
        let v2 = store.get_version(&"d1".into(), 2).unwrap().unwrap();
        assert_eq!(v2.line_weight(), LineWeight::Thin);
        let (reverted, _) = store.revert_to_version(&"d1".into(), 1, None).unwrap();
        assert_eq!(reverted.diagram.line_weight(), LineWeight::Thick);
    }

    #[test]
    fn canvas_sizes_are_kept_in_records_and_versions() {
        let mut store = Store::open_in_memory().unwrap();
        let record = store
            .create(&diagram("d1", DiagramTypeId::Conceptual))
            .unwrap();
        assert_eq!(record.diagram.canvas, None);
        let mut bigger = record.diagram.clone();
        bigger.set_canvas(Size::new(4800.0, 3200.0));
        store.save(&bigger, Some(record.revision)).unwrap();
        let got = store.get(&"d1".into()).unwrap().unwrap().diagram;
        assert_eq!(got.canvas, Some(Size::new(4800.0, 3200.0)));
        assert_eq!(store.status(&"d1".into()).unwrap().label(), "Draft · v1");
        assert_eq!(store.save_version(&"d1".into(), None).unwrap(), Some(2));
        let v2 = store.get_version(&"d1".into(), 2).unwrap().unwrap();
        assert_eq!(v2.canvas(), Size::new(4800.0, 3200.0));
        let (reverted, _) = store.revert_to_version(&"d1".into(), 1, None).unwrap();
        assert_eq!(reverted.diagram.canvas, None);
    }

    #[test]
    fn the_three_c4_types_give_way_to_one_with_a_sub_type() {
        // A database from before, with a diagram of an old C4 type that has a version and a
        // sub diagram, beside a diagram of another type.
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_version(&mut conn, 6).unwrap();
        conn.execute_batch(
            "INSERT INTO diagrams (id, type, name, content_hash, created_at, updated_at,
                                   line_weight, canvas_width, canvas_height)
             VALUES ('p', 'conceptual', 'Keep', '', '', '', 'thin', 3000, 2000),
                    ('old', 'c4-container', 'Old', '', '', '', NULL, NULL, NULL);
             INSERT INTO diagrams (id, type, name, content_hash, created_at, updated_at,
                                   parent_diagram_id)
             VALUES ('s', 'physical', 'Sub', '', '', '', 'old');
             INSERT INTO elements (diagram_id, id, ord, kind, label, x, y)
             VALUES ('s', 'e', 0, 'user', 'E', 0, 0),
                    ('old', 'e', 0, 'database', 'DB', 0, 0);
             INSERT INTO diagram_versions (diagram_id, version, saved_at, content_hash, snapshot)
             VALUES ('old', 1, '', '', '{}');",
        )
        .unwrap();
        let mut store = Store::setup(conn).unwrap();
        // The old C4 diagram is gone, with its elements and versions; the rest are kept, the
        // sub diagram now on its own.
        assert!(store.get(&"old".into()).unwrap().is_none());
        let count = |store: &Store, sql: &str| -> i64 {
            store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
        };
        assert_eq!(
            count(
                &store,
                "SELECT count(*) FROM elements WHERE diagram_id = 'old'"
            ),
            0
        );
        assert_eq!(
            count(
                &store,
                "SELECT count(*) FROM diagram_versions WHERE diagram_id = 'old'"
            ),
            0
        );
        let kept = store.get(&"p".into()).unwrap().unwrap().diagram;
        assert_eq!(kept.line_weight, Some(LineWeight::Thin));
        let sub = store.get(&"s".into()).unwrap().unwrap();
        assert_eq!(sub.diagram.content.elements.len(), 1);
        assert_eq!(store.list().unwrap().len(), 2);
        // A C4 diagram keeps its sub type in its record and its versions.
        let mut c4 = diagram("c", DiagramTypeId::C4);
        c4.content = StoredContent::default();
        let record = store.create(&c4).unwrap();
        assert_eq!(record.diagram.subtype(), "context");
        let mut container = record.diagram.clone();
        container.set_subtype("container");
        store.save(&container, Some(record.revision)).unwrap();
        let got = store.get(&"c".into()).unwrap().unwrap().diagram;
        assert_eq!(got.subtype.as_deref(), Some("container"));
        assert_eq!(got.type_label(), "C4 - Container");
        assert_eq!(store.status(&"c".into()).unwrap().label(), "Draft · v1");
        assert_eq!(store.save_version(&"c".into(), None).unwrap(), Some(2));
        let v2 = store.get_version(&"c".into(), 2).unwrap().unwrap();
        assert_eq!(v2.subtype(), "container");
        let (reverted, _) = store.revert_to_version(&"c".into(), 1, None).unwrap();
        assert_eq!(reverted.diagram.subtype, None);
        // A type still can't change.
        assert!(
            store
                .conn
                .execute("UPDATE diagrams SET type = 'c4' WHERE id = 'p'", [])
                .is_err()
        );
    }

    #[test]
    fn medium_line_weights_keep_what_the_old_column_held() {
        // A database from before medium existed, with a thin diagram in it.
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_version(&mut conn, 2).unwrap();
        conn.execute_batch(
            "INSERT INTO diagrams (id, type, name, content_hash, created_at, updated_at,
                                   line_weight)
             VALUES ('d1', 'conceptual', 'Old', '', '2026-01-01', '2026-01-01', 'thin')",
        )
        .unwrap();
        let mut store = Store::setup(conn).unwrap();
        let mut got = store.get(&"d1".into()).unwrap().unwrap();
        assert_eq!(got.diagram.line_weight, Some(LineWeight::Thin));
        got.diagram.set_line_weight(LineWeight::Medium);
        store.save(&got.diagram, Some(got.revision)).unwrap();
        let got = store.get(&"d1".into()).unwrap().unwrap().diagram;
        assert_eq!(got.line_weight, Some(LineWeight::Medium));
    }

    /// A diagram whose second element opens each of `opens`.
    fn opening(id: &str, opens: &[&str]) -> StoredDiagram {
        let mut d = diagram(id, DiagramTypeId::Conceptual);
        d.content.elements.truncate(1);
        for (i, sub) in opens.iter().enumerate() {
            let mut e = d.content.elements[0].clone();
            e.id = format!("x{i}").as_str().into();
            e.props.insert("subDiagramId".into(), json!(sub));
            d.content.elements.push(e);
        }
        d.content.connections.clear();
        d
    }

    fn parent(store: &Store, id: &str) -> Option<String> {
        parent_of(&store.conn, id).unwrap()
    }

    fn ids(store: &Store) -> Vec<String> {
        let mut ids: Vec<String> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|s| s.meta.id.0)
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn deleting_a_diagram_deletes_its_sub_diagrams_and_theirs() {
        let mut store = Store::open_in_memory().unwrap();
        store.create(&opening("top", &["mid"])).unwrap();
        store
            .create_under(&opening("mid", &["leaf"]), Some(&"top".into()))
            .unwrap();
        store
            .create_under(&opening("leaf", &[]), Some(&"mid".into()))
            .unwrap();
        store.create(&opening("other", &["mid"])).unwrap();
        assert_eq!(parent(&store, "mid").as_deref(), Some("top"));
        assert_eq!(parent(&store, "leaf").as_deref(), Some("mid"));
        let mut below = store.sub_diagrams(&"top".into()).unwrap();
        below.sort();
        assert_eq!(below, [DiagramId::from("leaf"), DiagramId::from("mid")]);
        let deleted = store.delete(&"top".into()).unwrap();
        assert_eq!(deleted[0], DiagramId::from("top"));
        assert_eq!(deleted.len(), 3);
        assert_eq!(ids(&store), ["other"]);
        let versions: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM diagram_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(versions, 1, "their versions go too");
    }

    #[test]
    fn links_never_change_which_diagram_a_sub_diagram_is_under() {
        let mut store = Store::open_in_memory().unwrap();
        let top = store.create(&opening("top", &["sub"])).unwrap();
        store
            .create_under(&opening("sub", &["top"]), Some(&"top".into()))
            .unwrap();
        store.create(&opening("other", &["sub", "top"])).unwrap();
        // Unlinking keeps it a sub diagram; another diagram linking it doesn't take it over.
        store
            .save(&opening("top", &[]), Some(top.revision))
            .unwrap();
        assert_eq!(parent(&store, "sub").as_deref(), Some("top"));
        assert_eq!(parent(&store, "top"), None);
        assert_eq!(parent(&store, "other"), None);
        // Deleting the linked diagram leaves the diagrams that linked it.
        store.delete(&"sub".into()).unwrap();
        assert_eq!(ids(&store), ["other", "top"]);
    }

    #[test]
    fn saving_a_version_drops_links_to_deleted_diagrams() {
        let mut store = Store::open_in_memory().unwrap();
        store.create(&opening("a", &["b", "gone", ""])).unwrap();
        store.create(&opening("b", &[])).unwrap();
        let links = |d: &StoredDiagram| -> Vec<Option<Value>> {
            d.content
                .elements
                .iter()
                .map(|e| e.props.get(SUB_DIAGRAM).cloned())
                .collect()
        };
        // Created as it came, links and all.
        let before = store.get(&"a".into()).unwrap().unwrap();
        assert_eq!(links(&before.diagram)[2], Some(json!("gone")));
        let version = store.save_version(&"a".into(), None).unwrap();
        assert_eq!(version, Some(2));
        let after = store.get(&"a".into()).unwrap().unwrap();
        assert_eq!(links(&after.diagram), [None, Some(json!("b")), None, None]);
        assert_eq!(after.revision, before.revision + 1);
        assert_eq!(
            links(&store.get_version(&"a".into(), 2).unwrap().unwrap()),
            links(&after.diagram)
        );
        assert!(store.status(&"a".into()).unwrap().saved);
        // Saving again finds nothing to do.
        assert_eq!(store.save_version(&"a".into(), None).unwrap(), None);
        // Reverting to the version with the dead link records it without.
        let (reverted, _) = store.revert_to_version(&"a".into(), 1, None).unwrap();
        assert_eq!(links(&reverted.diagram), links(&after.diagram));
    }

    #[test]
    fn parents_are_filled_in_from_links_made_before_the_column() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations().to_version(&mut conn, 3).unwrap();
        conn.execute_batch(
            "INSERT INTO diagrams (id, type, name, content_hash, created_at, updated_at)
             VALUES ('a', 'conceptual', 'A', '', '', ''), ('b', 'conceptual', 'B', '', '', ''),
                    ('c', 'conceptual', 'C', '', '', '');
             INSERT INTO elements (diagram_id, id, ord, kind, label, x, y, props)
             VALUES ('a', 'e', 0, 'actor', 'E', 0, 0, '{\"subDiagramId\": \"b\"}'),
                    ('b', 'e', 0, 'actor', 'E', 0, 0, '{\"subDiagramId\": \"a\"}'),
                    ('b', 'f', 1, 'actor', 'F', 0, 0, '{\"subDiagramId\": \"c\"}');",
        )
        .unwrap();
        let store = Store::setup(conn).unwrap();
        assert_eq!(parent(&store, "c").as_deref(), Some("b"));
        // a and b open each other: one of them is left at the top.
        let a = parent(&store, "a");
        let b = parent(&store, "b");
        assert!(a.is_none() != b.is_none(), "{a:?} {b:?}");
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
    fn imports_bring_sub_diagrams_under_their_parents() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .create(&diagram("top", DiagramTypeId::Conceptual))
            .unwrap();
        let mut named = |id: &str, name: &str, parent: &str| {
            let mut d = diagram(id, DiagramTypeId::Conceptual);
            d.meta.name = name.into();
            store
                .create_under(&d, Some(&DiagramId(parent.into())))
                .unwrap();
        };
        named("b", "Beta", "top");
        named("a", "Alpha", "top");
        named("a1", "Deep", "a");
        let tree = store.sub_diagram_tree(&DiagramId("top".into())).unwrap();
        let pairs: Vec<(&str, &str)> = tree.iter().map(|(i, p)| (i.as_str(), p.as_str())).collect();
        assert_eq!(pairs, [("a", "top"), ("b", "top"), ("a1", "a")]);
        // Into another database: the sub diagrams come back under their parents.
        let subs: Vec<SubDiagram> = tree
            .iter()
            .map(|(id, parent)| SubDiagram {
                parent: parent.clone(),
                diagram: store.get(id).unwrap().unwrap().diagram,
            })
            .collect();
        let top = store
            .get(&DiagramId("top".into()))
            .unwrap()
            .unwrap()
            .diagram;
        let mut other = Store::open_in_memory().unwrap();
        let imported = other.import_with_sub_diagrams(&top, &subs).unwrap();
        assert_eq!(imported.len(), 4);
        assert!(imported.iter().all(|i| !i.replaced));
        assert_eq!(other.sub_diagram_tree(&top.meta.id).unwrap(), tree);
        // Again: each is replaced.
        let again = other.import_with_sub_diagrams(&top, &subs).unwrap();
        assert!(again.iter().all(|i| i.replaced));
        // A sub diagram that would change type stops the whole import.
        let mut wrong = subs.clone();
        wrong[2].diagram.meta.diagram_type = DiagramTypeId::Physical;
        let mut renamed = top.clone();
        renamed.meta.name = "Renamed".into();
        assert!(matches!(
            other.import_with_sub_diagrams(&renamed, &wrong),
            Err(StoreError::TypeMismatch { .. })
        ));
        assert_eq!(
            other.get(&top.meta.id).unwrap().unwrap().diagram.meta.name,
            top.meta.name
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
    fn samples_have_medium_lines() {
        for sample in seed_diagrams() {
            assert_eq!(
                sample.line_weight(),
                LineWeight::Medium,
                "{}",
                sample.meta.name
            );
        }
        assert_eq!(seed_diagrams().len(), 8);
    }

    #[test]
    fn samples_load_once() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(store.load_samples().unwrap(), 8);
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
                "Arch Hiero - Containers",
                "Arch Hiero - MCP Capabilities",
                "Arch Hiero - MCP Server Components",
                "Arch Hiero - System Context",
                "Enterprise Architecture Overview",
                "Online Order",
                "Standard 3 -Tier Application",
                "Website Example - Active - Active"
            ]
        );
    }

    #[test]
    fn the_c4_samples_are_a_diagram_with_sub_diagrams() {
        let mut store = Store::open_in_memory().unwrap();
        store.load_samples().unwrap();
        let named = |name: &str| {
            store
                .list()
                .unwrap()
                .into_iter()
                .find(|s| s.meta.name == name)
                .unwrap()
        };
        let context = named("Arch Hiero - System Context");
        let containers = named("Arch Hiero - Containers");
        let components = named("Arch Hiero - MCP Server Components");
        assert_eq!(context.parent, None);
        assert_eq!(containers.parent, Some(context.meta.id.clone()));
        assert_eq!(components.parent, Some(containers.meta.id.clone()));
        assert_eq!(
            store.sub_diagrams(&context.meta.id).unwrap(),
            [containers.meta.id, components.meta.id]
        );
    }

    #[test]
    fn parents_come_first_and_loops_have_none() {
        let diagram = |id: &str, opens: &[&str]| {
            let mut d = diagram(id, DiagramTypeId::Conceptual);
            d.content.elements = opens
                .iter()
                .map(|o| {
                    StoredElement::from_json(&json!({
                        "id": format!("{id}-{o}"), "kind": "domain", "label": "D", "x": 0, "y": 0,
                        "subDiagramId": o
                    }))
                    .unwrap()
                })
                .collect();
            d
        };
        let ordered = with_parents(vec![
            diagram("child", &[]),
            diagram("top", &["child", "gone"]),
            diagram("a", &["b"]),
            diagram("b", &["a"]),
        ]);
        let pairs: Vec<(&str, Option<&str>)> = ordered
            .iter()
            .map(|(d, p)| (d.meta.id.as_str(), p.as_ref().map(DiagramId::as_str)))
            .collect();
        assert_eq!(
            pairs,
            [
                ("top", None),
                ("child", Some("top")),
                ("a", None),
                ("b", Some("a"))
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
