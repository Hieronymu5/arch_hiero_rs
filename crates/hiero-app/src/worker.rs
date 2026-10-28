//! The store thread: it owns the connection that writes, so the UI never waits on the disk for
//! autosaves, and it watches for changes made by other processes, such as the MCP server.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use hiero_core::model::DiagramId;
use hiero_core::stored::StoredDiagram;
use hiero_store::{Imported, Record, Store, StoreError};

/// What the UI asks the store thread to do.
pub enum Request {
    /// Saves the current record, autosave style. Answered with an event.
    Save {
        diagram: StoredDiagram,
        expected: i64,
    },
    Create {
        diagram: StoredDiagram,
        reply: Sender<Result<Record, StoreError>>,
    },
    Delete {
        id: DiagramId,
        reply: Sender<Result<(), StoreError>>,
    },
    Import {
        diagram: StoredDiagram,
        reply: Sender<Result<Imported, StoreError>>,
    },
    /// Adds the sample diagrams, when there are none yet.
    LoadSamples {
        reply: Sender<Result<usize, StoreError>>,
    },
    /// Saves the current record now, then records it as a version.
    SaveVersion {
        diagram: StoredDiagram,
        expected: i64,
        description: Option<String>,
        reply: Sender<Result<(Record, Option<i64>), StoreError>>,
    },
    Revert {
        id: DiagramId,
        version: i64,
        description: Option<String>,
        reply: Sender<Result<(Record, i64), StoreError>>,
    },
    DeleteVersion {
        id: DiagramId,
        version: i64,
        reply: Sender<Result<(), StoreError>>,
    },
    /// Forgets what this thread knows about a diagram's revision, after the UI reloads it.
    Reloaded { id: DiagramId, revision: i64 },
}

/// What the store thread tells the UI.
pub enum Event {
    Saved {
        id: DiagramId,
        revision: i64,
    },
    SaveFailed {
        id: DiagramId,
        message: String,
    },
    /// The diagram changed elsewhere since the UI last read it.
    Conflict {
        id: DiagramId,
    },
    /// Another process changed the database.
    External,
}

/// The UI's handle on the store thread.
pub struct Worker {
    requests: Sender<Request>,
    pub events: Receiver<Event>,
}

impl Worker {
    pub fn start(path: PathBuf, ctx: egui::Context) -> Result<Self, StoreError> {
        let store = Store::open(&path)?;
        let (requests, inbox) = channel::<Request>();
        let (outbox, events) = channel::<Event>();
        std::thread::Builder::new()
            .name("store".into())
            .spawn(move || run(store, &inbox, &outbox, &ctx))
            .map_err(|e| StoreError::Migration(e.to_string()))?;
        Ok(Self { requests, events })
    }

    pub fn send(&self, request: Request) {
        let _ = self.requests.send(request);
    }

    /// Sends a request and waits for its answer.
    pub fn call<T>(
        &self,
        make: impl FnOnce(Sender<Result<T, StoreError>>) -> Request,
    ) -> Result<T, StoreError> {
        let (reply, answer) = channel();
        self.send(make(reply));
        answer
            .recv_timeout(Duration::from_secs(15))
            .map_err(|_| StoreError::Migration("the store didn't answer".into()))?
    }
}

fn run(mut store: Store, inbox: &Receiver<Request>, outbox: &Sender<Event>, ctx: &egui::Context) {
    // The revision each diagram is at after this thread's own last write, so a run of
    // autosaves doesn't conflict with itself.
    let mut known: HashMap<DiagramId, i64> = HashMap::new();
    let mut data_version = store.data_version().unwrap_or_default();
    loop {
        match inbox.recv_timeout(Duration::from_millis(500)) {
            Ok(request) => handle(&mut store, &mut known, request, outbox),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Drain whatever else is waiting before checking for outside changes.
        while let Ok(request) = inbox.try_recv() {
            handle(&mut store, &mut known, request, outbox);
        }
        if let Ok(version) = store.data_version()
            && version != data_version
        {
            data_version = version;
            let _ = outbox.send(Event::External);
            ctx.request_repaint();
            continue;
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn handle(
    store: &mut Store,
    known: &mut HashMap<DiagramId, i64>,
    request: Request,
    outbox: &Sender<Event>,
) {
    let remember = |known: &mut HashMap<DiagramId, i64>, record: &Record| {
        known.insert(record.diagram.meta.id.clone(), record.revision);
    };
    match request {
        Request::Save { diagram, expected } => {
            let id = diagram.meta.id.clone();
            let expected = known
                .get(&id)
                .copied()
                .map_or(expected, |k| k.max(expected));
            match store.save(&diagram, Some(expected)) {
                Ok(record) => {
                    remember(known, &record);
                    let _ = outbox.send(Event::Saved {
                        id,
                        revision: record.revision,
                    });
                }
                Err(StoreError::Conflict) => {
                    let _ = outbox.send(Event::Conflict { id });
                }
                Err(e) => {
                    let _ = outbox.send(Event::SaveFailed {
                        id,
                        message: e.to_string(),
                    });
                }
            }
        }
        Request::Create { diagram, reply } => {
            let result = store.create(&diagram);
            if let Ok(r) = &result {
                remember(known, r);
            }
            let _ = reply.send(result);
        }
        Request::Delete { id, reply } => {
            known.remove(&id);
            let _ = reply.send(store.delete(&id));
        }
        Request::Import { diagram, reply } => {
            let result = store.import(&diagram);
            if let Ok(i) = &result {
                remember(known, &i.record);
            }
            let _ = reply.send(result);
        }
        Request::LoadSamples { reply } => {
            let _ = reply.send(store.load_samples());
        }
        Request::SaveVersion {
            diagram,
            expected,
            description,
            reply,
        } => {
            let id = diagram.meta.id.clone();
            let expected = known
                .get(&id)
                .copied()
                .map_or(expected, |k| k.max(expected));
            let result = store.save(&diagram, Some(expected)).and_then(|record| {
                remember(known, &record);
                let version = store.save_version(&id, description.as_deref())?;
                Ok((record, version))
            });
            let _ = reply.send(result);
        }
        Request::Revert {
            id,
            version,
            description,
            reply,
        } => {
            let result = store.revert_to_version(&id, version, description.as_deref());
            if let Ok((r, _)) = &result {
                remember(known, r);
            }
            let _ = reply.send(result);
        }
        Request::DeleteVersion { id, version, reply } => {
            let _ = reply.send(store.delete_version(&id, version));
        }
        Request::Reloaded { id, revision } => {
            known.insert(id, revision);
        }
    }
}
