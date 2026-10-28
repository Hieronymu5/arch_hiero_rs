//! The app: its pages, the open diagram, autosave, undo and redo, versions, and changes made
//! outside the app.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use hiero_core::file::{diagram_to_zip, file_name};
use hiero_core::history::{DiagramSnapshot, UndoStack};
use hiero_core::model::{DiagramId, DiagramTypeId};
use hiero_core::stored::StoredDiagram;
use hiero_editor::DiagramEditor;
use hiero_editor::theme::Accent;
use hiero_store::{Record, Status, Store, StoreError, Summary};

use crate::editors::editor_for;
use crate::list_sort::ListSort;
use crate::worker::{Event, Request, Worker};

/// How long typing waits before it's saved.
const TYPING_DELAY: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Page {
    List,
    New,
    Details(DiagramId),
    Editor(DiagramId),
    History(DiagramId),
}

/// What the last action did, shown at the top of the page.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub message: String,
    pub error: bool,
}

impl Outcome {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error: true,
        }
    }
}

/// The diagram being edited.
pub struct OpenDiagram {
    pub record: StoredDiagram,
    pub revision: i64,
    pub editor: Box<dyn DiagramEditor>,
    pub status: Status,
    /// Set while an autosave hasn't been answered.
    pub saving: bool,
    pub save_failed: Option<String>,
    /// Such as "This diagram was changed outside the editor".
    pub notice: Option<String>,
}

/// A dialog over the page.
pub enum Dialog {
    DeleteDiagram {
        id: DiagramId,
        name: String,
    },
    SaveVersion {
        description: String,
        next: i64,
    },
    Revert {
        id: DiagramId,
        version: i64,
        unsaved: bool,
        next: i64,
        save_description: String,
        description: String,
    },
    DeleteVersion {
        id: DiagramId,
        version: i64,
    },
}

/// The fields of the Add diagram and Edit details forms.
#[derive(Debug, Clone, Default)]
pub struct DetailsForm {
    pub name: String,
    pub diagram_type: Option<DiagramTypeId>,
    pub description: String,
    pub colors: BTreeMap<String, String>,
    pub tried: bool,
    pub error: Option<String>,
}

pub struct App {
    pub store: Store,
    pub worker: Worker,
    pub page: Page,
    pub back: Vec<Page>,
    pub list: Vec<Summary>,
    /// How the list is sorted, after a column's header is clicked; unset, it's by name.
    pub list_sort: Option<ListSort>,
    pub outcome: Option<Outcome>,
    pub open: Option<OpenDiagram>,
    /// Each diagram's undo history, for as long as the app runs.
    pub undo: HashMap<DiagramId, UndoStack<DiagramSnapshot>>,
    pub dialog: Option<Dialog>,
    pub form: DetailsForm,
    pub save_due: Option<Instant>,
    pub theme: egui::ThemePreference,
    pub accent: Accent,
}

impl App {
    pub fn new(store: Store, worker: Worker, theme: egui::ThemePreference, accent: Accent) -> Self {
        let mut app = Self {
            store,
            worker,
            page: Page::List,
            back: Vec::new(),
            list: Vec::new(),
            list_sort: None,
            outcome: None,
            open: None,
            undo: HashMap::new(),
            dialog: None,
            form: DetailsForm::default(),
            save_due: None,
            theme,
            accent,
        };
        app.refresh_list();
        app
    }

    pub fn refresh_list(&mut self) {
        match self.store.list() {
            Ok(list) => self.list = list,
            Err(e) => {
                self.outcome = Some(Outcome::error(format!(
                    "The diagrams couldn't be loaded. {e}"
                )));
            }
        }
    }

    pub fn go(&mut self, page: Page) {
        if self.page != page {
            self.back.push(self.page.clone());
            self.page = page;
        }
    }

    pub fn go_back(&mut self) {
        self.page = self.back.pop().unwrap_or(Page::List);
        if self.page == Page::List {
            self.back.clear();
            self.flush_save();
            self.refresh_list();
        }
    }

    // Opening and saving

    /// Opens a diagram in the editor, keeping its undo history from earlier in the session.
    pub fn open_diagram(&mut self, id: &DiagramId) {
        if self.open.as_ref().is_some_and(|o| o.record.meta.id == *id) {
            self.go(Page::Editor(id.clone()));
            return;
        }
        self.flush_save();
        match self.load(id) {
            Ok(open) => {
                self.worker.send(Request::Reloaded {
                    id: id.clone(),
                    revision: open.revision,
                });
                self.open = Some(open);
                self.go(Page::Editor(id.clone()));
            }
            Err(e) => self.outcome = Some(Outcome::error(e)),
        }
    }

    fn load(&self, id: &DiagramId) -> Result<OpenDiagram, String> {
        let record = self
            .store
            .get(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| StoreError::NotFound.to_string())?;
        let editor = editor_for(&record.diagram)
            .map_err(|e| format!("The diagram couldn't be opened. {e}"))?;
        let status = self.store.status(id).map_err(|e| e.to_string())?;
        Ok(OpenDiagram {
            record: record.diagram,
            revision: record.revision,
            editor,
            status,
            saving: false,
            save_failed: None,
            notice: None,
        })
    }

    pub fn undo_stack(&mut self, id: &DiagramId) -> &mut UndoStack<DiagramSnapshot> {
        self.undo.entry(id.clone()).or_default()
    }

    /// Takes the editor's changes: each goes on the undo history, and the content is saved.
    pub fn collect_changes(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };
        if let Some(notice) = open.editor.take_notice() {
            open.notice = Some(notice);
        }
        let changes = open.editor.take_changes();
        if changes.is_empty() {
            return;
        }
        let id = open.record.meta.id.clone();
        let mut typing = false;
        let mut befores = Vec::new();
        for change in changes {
            let mut snapshot = DiagramSnapshot::of(&open.record);
            snapshot.content = change.before;
            typing |= change.group.is_some();
            befores.push((snapshot, change.group));
            // The record's content moves on after each.
        }
        open.record.content = open.editor.content();
        open.notice = None;
        let stack = self.undo.entry(id).or_default();
        for (snapshot, group) in befores {
            match group {
                Some(g) => stack.push_keyed(snapshot, Some(g)),
                None => {
                    stack.break_group();
                    stack.push(snapshot);
                }
            }
        }
        let due = Instant::now() + if typing { TYPING_DELAY } else { Duration::ZERO };
        self.save_due = Some(self.save_due.map_or(due, |d| d.min(due)));
    }

    /// Saves the open diagram if it's due.
    pub fn save_if_due(&mut self) {
        if self.save_due.is_some_and(|due| Instant::now() >= due) {
            self.flush_save();
        }
    }

    /// Saves the open diagram now if there's anything waiting.
    pub fn flush_save(&mut self) {
        if self.save_due.take().is_none() {
            return;
        }
        if let Some(open) = &mut self.open {
            open.saving = true;
            self.worker.send(Request::Save {
                diagram: open.record.clone(),
                expected: open.revision,
            });
        }
    }

    /// Puts `snapshot` into the open diagram, as undo, redo, and outside changes do.
    fn show_snapshot(&mut self, snapshot: &DiagramSnapshot) {
        let Some(open) = &mut self.open else {
            return;
        };
        snapshot.apply_to(&mut open.record);
        if let Err(e) = open.editor.replace_content(&open.record.content) {
            open.save_failed = Some(e.to_string());
        }
        open.editor.set_colors(&open.record.colors);
    }

    pub fn undo(&mut self) {
        self.step(true);
    }

    pub fn redo(&mut self) {
        self.step(false);
    }

    fn step(&mut self, back: bool) {
        let Some(open) = &self.open else {
            return;
        };
        let id = open.record.meta.id.clone();
        let current = DiagramSnapshot::of(&open.record);
        let stack = self.undo.entry(id).or_default();
        let next = if back {
            stack.undo(current)
        } else {
            stack.redo(current)
        };
        if let Some(snapshot) = next {
            self.show_snapshot(&snapshot);
            if let Some(open) = &mut self.open {
                open.editor.set_announcement(if back {
                    "Undone.".into()
                } else {
                    "Redone.".into()
                });
            }
            self.save_due = Some(Instant::now());
        }
    }

    pub fn can_undo(&self) -> bool {
        self.open
            .as_ref()
            .and_then(|o| self.undo.get(&o.record.meta.id))
            .is_some_and(UndoStack::can_undo)
    }

    pub fn can_redo(&self) -> bool {
        self.open
            .as_ref()
            .and_then(|o| self.undo.get(&o.record.meta.id))
            .is_some_and(UndoStack::can_redo)
    }

    /// Handles what the store thread says.
    pub fn handle_events(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            match event {
                Event::Saved { id, revision } => {
                    if let Some(open) = self.open.as_mut().filter(|o| o.record.meta.id == id) {
                        open.revision = revision;
                        open.saving = false;
                        open.save_failed = None;
                        if let Ok(status) = self.store.status(&id) {
                            open.status = status;
                        }
                    }
                }
                Event::SaveFailed { id, message } => {
                    if let Some(open) = self.open.as_mut().filter(|o| o.record.meta.id == id) {
                        open.saving = false;
                        open.save_failed = Some(message);
                    }
                }
                Event::Conflict { id } => self.conflict(&id),
                Event::External => {
                    self.refresh_list();
                    self.reload_if_changed();
                }
            }
        }
    }

    /// Our save was overtaken by a change made elsewhere: show theirs, and keep ours on Redo.
    fn conflict(&mut self, id: &DiagramId) {
        let Some(open) = self.open.as_ref().filter(|o| o.record.meta.id == *id) else {
            return;
        };
        let mine = DiagramSnapshot::of(&open.record);
        let Ok(Some(record)) = self.store.get(id) else {
            return;
        };
        self.undo_stack(id).push_redo(mine);
        self.accept_external(record, "This diagram was changed outside the editor, so it was reloaded. Your last change is on Redo.");
    }

    /// Shows a change made outside the app, as one undo step.
    fn reload_if_changed(&mut self) {
        let Some(open) = &self.open else {
            return;
        };
        let id = open.record.meta.id.clone();
        if open.saving || self.save_due.is_some() {
            return;
        }
        match self.store.get(&id) {
            Ok(Some(record)) if record.revision != open.revision => {
                let before = DiagramSnapshot::of(&open.record);
                let stack = self.undo_stack(&id);
                stack.break_group();
                stack.push(before);
                self.accept_external(record, "This diagram was changed outside the editor, such as by an agent. Undo takes the change back out.");
                if let Some(open) = &mut self.open {
                    open.editor
                        .set_announcement("Changes from an agent.".into());
                }
            }
            Ok(None) => {
                self.open = None;
                self.page = Page::List;
                self.outcome = Some(Outcome::error(
                    "The diagram was deleted outside the editor.",
                ));
            }
            _ => {}
        }
    }

    fn accept_external(&mut self, record: Record, notice: &str) {
        let id = record.diagram.meta.id.clone();
        self.show_snapshot(&DiagramSnapshot::of(&record.diagram));
        if let Some(open) = &mut self.open {
            open.record.meta = record.diagram.meta.clone();
            open.revision = record.revision;
            open.saving = false;
            open.notice = Some(notice.to_owned());
            if let Ok(status) = self.store.status(&id) {
                open.status = status;
            }
        }
        self.worker.send(Request::Reloaded {
            id,
            revision: record.revision,
        });
    }

    // Diagrams

    pub fn create(&mut self) {
        let form = self.form.clone();
        let Some(diagram_type) = form.diagram_type else {
            return;
        };
        let diagram =
            hiero_registry::new_diagram(diagram_type, &form.name, Some(form.description.clone()));
        match self.worker.call(|reply| Request::Create { diagram, reply }) {
            Ok(record) => {
                self.refresh_list();
                self.back.clear();
                self.page = Page::List;
                self.open_diagram(&record.diagram.meta.id);
            }
            Err(e) => self.form.error = Some(format!("The diagram couldn't be saved. {e}")),
        }
    }

    /// Applies the Edit details form to the open diagram, as one undo step.
    pub fn save_details(&mut self) {
        let form = self.form.clone();
        let Some(open) = &mut self.open else {
            return;
        };
        let id = open.record.meta.id.clone();
        let before = DiagramSnapshot::of(&open.record);
        let ops = hiero_registry::ops(open.record.meta.diagram_type);
        open.record.meta.name = form.name.trim().to_owned();
        open.record.meta.description =
            Some(form.description.trim().to_owned()).filter(|d| !d.is_empty());
        open.record.colors = ops.normalize_colors(&form.colors);
        open.editor.set_colors(&open.record.colors);
        if DiagramSnapshot::of(&open.record) != before {
            let stack = self.undo_stack(&id);
            stack.break_group();
            stack.push(before);
            self.save_due = Some(Instant::now());
        }
        self.flush_save();
        self.go_back();
    }

    pub fn delete(&mut self, id: &DiagramId, name: &str) {
        match self.worker.call(|reply| Request::Delete {
            id: id.clone(),
            reply,
        }) {
            Ok(()) => {
                if self.open.as_ref().is_some_and(|o| o.record.meta.id == *id) {
                    self.open = None;
                }
                self.undo.remove(id);
                self.outcome = Some(Outcome::ok(format!("Deleted “{name}”.")));
            }
            Err(e) => {
                self.outcome = Some(Outcome::error(format!("“{name}” couldn't be deleted. {e}")));
            }
        }
        self.refresh_list();
    }

    /// Asks for a zip and imports it, over the diagram with its id or as a new one.
    pub fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Diagram", &["zip"])
            .set_title("Import diagram")
            .pick_file()
        else {
            return;
        };
        let file = path
            .file_name()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        let result = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| hiero_registry::read_diagram_file(&bytes).map_err(|e| e.0));
        let diagram = match result {
            Ok(d) => d,
            Err(reason) => {
                self.outcome = Some(Outcome::error(format!("{file} wasn’t imported. {reason}")));
                return;
            }
        };
        let id = diagram.meta.id.clone();
        let before = self.store.get(&id).ok().flatten();
        match self.worker.call(|reply| Request::Import { diagram, reply }) {
            Ok(imported) => {
                let name = imported.record.diagram.meta.name.clone();
                if let Some(before) = before {
                    let stack = self.undo_stack(&id);
                    stack.break_group();
                    stack.push(DiagramSnapshot::of(&before.diagram));
                }
                if self.open.as_ref().is_some_and(|o| o.record.meta.id == id) {
                    self.accept_external(
                        imported.record.clone(),
                        "This diagram was replaced by an import. Undo brings back what it was.",
                    );
                }
                self.outcome = Some(Outcome::ok(if imported.replaced {
                    format!("Updated “{name}” from {file}.")
                } else {
                    format!("Imported “{name}” from {file} as a new diagram.")
                }));
            }
            Err(e) => self.outcome = Some(Outcome::error(format!("{file} wasn’t imported. {e}"))),
        }
        self.refresh_list();
    }

    // Versions

    pub fn status_of(&self, id: &DiagramId) -> Option<Status> {
        if let Some(open) = self.open.as_ref().filter(|o| o.record.meta.id == *id) {
            return Some(open.status);
        }
        self.store.status(id).ok()
    }

    /// The number the next saved version will get.
    pub fn next_version(&self, id: &DiagramId) -> i64 {
        self.store
            .list_versions(id)
            .ok()
            .and_then(|v| v.first().map(|f| f.version))
            .unwrap_or(0)
            + 1
    }

    /// Records the open diagram as a new version.
    pub fn save_version(&mut self, description: &str) {
        self.save_due = None;
        let Some(open) = &mut self.open else {
            return;
        };
        let diagram = open.record.clone();
        let expected = open.revision;
        let description = Some(description.trim().to_owned()).filter(|d| !d.is_empty());
        match self.worker.call(|reply| Request::SaveVersion {
            diagram,
            expected,
            description,
            reply,
        }) {
            Ok((record, version)) => {
                open.revision = record.revision;
                open.saving = false;
                if let Ok(status) = self.store.status(&record.diagram.meta.id) {
                    open.status = status;
                }
                open.editor.set_announcement(match version {
                    Some(v) => format!("Saved as version {v}."),
                    None => "Already saved.".into(),
                });
            }
            Err(e) => open.save_failed = Some(format!("The version couldn't be saved. {e}")),
        }
        self.refresh_list();
    }

    /// Reverts a diagram to a version, recording that as a new version; undoable.
    pub fn revert(&mut self, id: &DiagramId, version: i64, description: &str) {
        let before = self
            .open
            .as_ref()
            .filter(|o| o.record.meta.id == *id)
            .map(|o| DiagramSnapshot::of(&o.record))
            .or_else(|| {
                self.store
                    .get(id)
                    .ok()
                    .flatten()
                    .map(|r| DiagramSnapshot::of(&r.diagram))
            });
        self.flush_save();
        let description = Some(description.trim().to_owned()).filter(|d| !d.is_empty());
        match self.worker.call(|reply| Request::Revert {
            id: id.clone(),
            version,
            description,
            reply,
        }) {
            Ok((record, new_version)) => {
                if let Some(before) = before {
                    let stack = self.undo_stack(id);
                    stack.break_group();
                    stack.push(before);
                }
                if self.open.as_ref().is_some_and(|o| o.record.meta.id == *id) {
                    self.accept_external(record, "");
                    if let Some(open) = &mut self.open {
                        open.notice = None;
                        open.editor.set_announcement(format!(
                            "Reverted to v{version}, saved as v{new_version}."
                        ));
                    }
                }
                self.outcome = Some(Outcome::ok(format!(
                    "Reverted to v{version}, saved as v{new_version}."
                )));
            }
            Err(e) => {
                self.outcome = Some(Outcome::error(format!(
                    "The diagram couldn't be reverted. {e}"
                )));
            }
        }
        self.refresh_list();
    }

    pub fn delete_version(&mut self, id: &DiagramId, version: i64) {
        match self.worker.call(|reply| Request::DeleteVersion {
            id: id.clone(),
            version,
            reply,
        }) {
            Ok(()) => self.outcome = Some(Outcome::ok(format!("Deleted v{version}."))),
            Err(e) => {
                self.outcome = Some(Outcome::error(format!(
                    "v{version} couldn't be deleted. {e}"
                )));
            }
        }
        if let Some(open) = self.open.as_mut().filter(|o| o.record.meta.id == *id)
            && let Ok(status) = self.store.status(id)
        {
            open.status = status;
        }
        self.refresh_list();
    }

    // Exports

    /// Asks where to save `bytes`, starting with `name`.
    fn save_file(&mut self, name: &str, extension: &str, bytes: &[u8]) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(file_name(name, extension))
            .add_filter(extension, &[extension])
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                let shown = path
                    .file_name()
                    .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
                self.outcome = Some(Outcome::ok(format!("Saved {shown}.")));
            }
            Err(e) => {
                self.outcome = Some(Outcome::error(format!("The file couldn't be saved. {e}")));
            }
        }
    }

    fn current(&self, id: &DiagramId) -> Option<StoredDiagram> {
        if let Some(open) = self.open.as_ref().filter(|o| o.record.meta.id == *id) {
            return Some(open.record.clone());
        }
        self.store.get(id).ok().flatten().map(|r| r.diagram)
    }

    pub fn export(&mut self, id: &DiagramId, version: Option<i64>, format: ExportFormat) {
        let diagram = match version {
            Some(v) => self.store.get_version(id, v).ok().flatten(),
            None => self.current(id),
        };
        let Some(diagram) = diagram else {
            self.outcome = Some(Outcome::error("That diagram doesn't exist."));
            return;
        };
        let name = match version {
            Some(v) => format!("{} v{v}", diagram.meta.name),
            None => diagram.meta.name.clone(),
        };
        let ops = hiero_registry::ops(diagram.meta.diagram_type);
        let result: Result<Vec<u8>, String> = match format {
            ExportFormat::Zip => Ok(diagram_to_zip(&diagram)),
            ExportFormat::Svg => ops
                .to_svg(&diagram)
                .map(String::into_bytes)
                .map_err(|e| e.0),
            ExportFormat::Drawio => ops
                .to_drawio(&diagram)
                .map(String::into_bytes)
                .map_err(|e| e.0),
        };
        match result {
            Ok(bytes) => self.save_file(&name, format.extension(), &bytes),
            Err(e) => {
                self.outcome = Some(Outcome::error(format!(
                    "The diagram couldn't be exported. {e}"
                )));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Zip,
    Svg,
    Drawio,
}

impl ExportFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::Svg => "svg",
            Self::Drawio => "drawio",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Zip => "Diagram file (.zip)",
            Self::Svg => "SVG image (.svg)",
            Self::Drawio => "draw.io file (.drawio)",
        }
    }
}
