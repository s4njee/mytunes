use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use mt_app::{
    import::{ImportSettings, LibraryImporter},
    library::{AppError, DesktopLibrary},
};
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode, ModifyKind},
};
use tauri::{AppHandle, Manager, State, Wry, async_runtime};
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder};
use tauri_specta::Event;

use crate::events::{DesktopChannels, LibraryChanged, ScanUpdate};

pub const ADD_FOLDER: &str = "add_folder_to_library";
const QUIET_PERIOD: Duration = Duration::from_secs(2);

enum Preference {
    Read,
    CopyFiles(bool),
    AddWatch(PathBuf),
    RemoveWatch(PathBuf),
}

enum Job {
    Import(Vec<PathBuf>),
    Changed(PathBuf),
    Preference(
        Preference,
        async_runtime::Sender<Result<ImportSettings, AppError>>,
    ),
    Stop,
}

pub struct ImportService(mpsc::Sender<Job>);

impl ImportService {
    pub fn start(app: &AppHandle, db_path: &Path, media: PathBuf) -> Result<Self, AppError> {
        let importer = LibraryImporter::open(db_path, media.clone())?;
        let data = db_path.parent().ok_or(AppError::Unavailable)?.to_owned();
        let (send, receive) = mpsc::channel();
        let mut worker = Worker {
            app: app.clone(),
            importer,
            send: send.clone(),
            data,
            media,
            watches: HashMap::new(),
            pending: HashMap::new(),
        };
        std::thread::Builder::new()
            .name("library-import".into())
            .spawn(move || worker.run(&receive))
            .map_err(|_| AppError::Unavailable)?;
        Ok(Self(send))
    }

    pub fn enqueue(&self, paths: Vec<PathBuf>) {
        let _ = self.0.send(Job::Import(paths));
    }

    pub fn stop(&self) {
        let _ = self.0.send(Job::Stop);
    }

    async fn preference(&self, preference: Preference) -> Result<ImportSettings, AppError> {
        let (send, mut receive) = async_runtime::channel(1);
        self.0
            .send(Job::Preference(preference, send))
            .map_err(|_| AppError::Unavailable)?;
        receive.recv().await.ok_or(AppError::Unavailable)?
    }
}

struct Worker {
    app: AppHandle,
    importer: LibraryImporter,
    send: mpsc::Sender<Job>,
    data: PathBuf,
    media: PathBuf,
    watches: HashMap<PathBuf, RecommendedWatcher>,
    pending: HashMap<PathBuf, Instant>,
}

impl Worker {
    fn run(&mut self, receive: &mpsc::Receiver<Job>) {
        for path in self.importer.settings().watched_folders.clone() {
            match self.watcher(&path) {
                Ok(watcher) => {
                    self.watches.insert(path.clone(), watcher);
                    self.schedule(path);
                }
                Err(error) => eprintln!("could not restore watch on {}: {error}", path.display()),
            }
        }
        loop {
            let next = self.pending.values().min().copied();
            let job = match next {
                Some(deadline) => {
                    receive.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => receive
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            };
            match job {
                Ok(Job::Import(paths)) => self.scan(&paths),
                Ok(Job::Changed(path)) => {
                    if self.watches.contains_key(&path) {
                        self.schedule(path);
                    }
                }
                Ok(Job::Preference(preference, reply)) => {
                    let _ = reply.try_send(self.apply_preference(preference));
                }
                Ok(Job::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let now = Instant::now();
                    let mut ready = Vec::new();
                    self.pending.retain(|folder, deadline| {
                        if *deadline > now {
                            return true;
                        }
                        ready.push(folder.clone());
                        false
                    });
                    self.scan(&ready);
                }
            }
        }
    }

    fn scan(&mut self, paths: &[PathBuf]) {
        let channels = self.app.state::<DesktopChannels>();
        let update = match self.importer.scan(paths, |progress| {
            let _ = channels.scan.publish(ScanUpdate::Scanning { progress });
        }) {
            Ok(report) => {
                let library = self.app.state::<Arc<DesktopLibrary>>();
                if let Ok(revision) = library.invalidate_views() {
                    let _ = LibraryChanged { revision }.emit(&self.app);
                }
                ScanUpdate::Finished {
                    duplicates: report.duplicates,
                    errors: report.file_errors.len() + report.walk_errors.len(),
                }
            }
            Err(error) => {
                eprintln!("library import: {error}");
                ScanUpdate::Failed
            }
        };
        let _ = channels.scan.publish(update);
    }

    fn schedule(&mut self, path: PathBuf) {
        self.pending.insert(path, Instant::now() + QUIET_PERIOD);
    }

    fn watcher(&self, path: &Path) -> Result<RecommendedWatcher, notify::Error> {
        let send = self.send.clone();
        let root = path.to_owned();
        let media = self.media.clone();
        let data = self.data.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) if relevant(&event, &root, &media, &data) => {
                    let _ = send.send(Job::Changed(root.clone()));
                }
                Ok(_) => {}
                Err(error) => eprintln!("folder watch on {}: {error}", root.display()),
            })?;
        watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(watcher)
    }

    fn apply_preference(&mut self, preference: Preference) -> Result<ImportSettings, AppError> {
        let mut settings = self.importer.settings().clone();
        match preference {
            Preference::Read => {}
            Preference::CopyFiles(copy_files) => {
                settings.copy_files = copy_files;
                self.importer.save(settings.clone())?;
            }
            Preference::AddWatch(path) => {
                let root = path.canonicalize().map_err(|_| AppError::WatchFolder)?;
                if !root.is_dir() || root.to_str().is_none() {
                    return Err(AppError::WatchFolder);
                }
                if !self.watches.contains_key(&root) {
                    let watcher = self.watcher(&root).map_err(|error| {
                        eprintln!("could not watch {}: {error}", root.display());
                        AppError::WatchFolder
                    })?;
                    if !settings.watched_folders.contains(&root) {
                        settings.watched_folders.push(root.clone());
                        settings.watched_folders.sort();
                        self.importer.save(settings.clone())?;
                    }
                    self.watches.insert(root.clone(), watcher);
                    self.schedule(root);
                }
            }
            Preference::RemoveWatch(path) => {
                settings.watched_folders.retain(|folder| folder != &path);
                self.importer.save(settings.clone())?;
                self.watches.remove(&path);
                self.pending.remove(&path);
            }
        }
        Ok(settings)
    }
}

fn relevant(event: &notify::Event, root: &Path, media: &Path, data: &Path) -> bool {
    if event.need_rescan() {
        return true;
    }
    match event.kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => {}
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => return false,
        _ => {}
    }
    event.paths.iter().any(|path| {
        if path.starts_with(media) && !root.starts_with(media) {
            return false;
        }
        path.parent() != Some(data)
            || !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(
                    "library.sqlite"
                        | "library.sqlite-wal"
                        | "library.sqlite-shm"
                        | "library.sqlite-journal"
                )
            )
    })
}

fn picker(app: &AppHandle, title: &str) -> FileDialogBuilder<Wry> {
    let dialog = app.dialog().file().set_title(title);
    match app.get_webview_window("main") {
        Some(window) => dialog.set_parent(&window),
        None => dialog,
    }
}

pub fn pick_import_folder(app: &AppHandle) -> Result<(), AppError> {
    if app.try_state::<ImportService>().is_none() {
        return Err(AppError::Unavailable);
    }
    let handle = app.clone();
    picker(app, "Add Folder to Library").pick_folder(move |selected| {
        if let Some(path) = selected.and_then(|selected| selected.into_path().ok()) {
            handle.state::<ImportService>().enqueue(vec![path]);
        }
    });
    Ok(())
}

/// Open the native folder picker and queue the selected folder for import.
#[tauri::command]
#[specta::specta]
pub fn import_choose_folder(app: AppHandle) -> Result<(), AppError> {
    pick_import_folder(&app)
}

/// Read the persisted copy preference and watched folders.
#[tauri::command]
#[specta::specta]
pub async fn import_settings_get(
    service: State<'_, ImportService>,
) -> Result<ImportSettings, AppError> {
    service.preference(Preference::Read).await
}

/// Set whether newly added files are copied to the managed media folder.
#[tauri::command]
#[specta::specta]
pub async fn import_set_copy(
    service: State<'_, ImportService>,
    copy_files: bool,
) -> Result<ImportSettings, AppError> {
    service.preference(Preference::CopyFiles(copy_files)).await
}

/// Choose a folder with the native picker and start watching it.
#[tauri::command]
#[specta::specta]
pub async fn import_add_watch(
    app: AppHandle,
    service: State<'_, ImportService>,
) -> Result<Option<ImportSettings>, AppError> {
    let selected = async_runtime::spawn_blocking(move || {
        picker(&app, "Choose Watched Folder").blocking_pick_folder()
    })
    .await
    .map_err(|_| AppError::Unavailable)?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|_| AppError::WatchFolder)?;
    service
        .preference(Preference::AddWatch(path))
        .await
        .map(Some)
}

/// Stop watching a folder and discard its pending rescan.
#[tauri::command]
#[specta::specta]
pub async fn import_remove_watch(
    service: State<'_, ImportService>,
    path: PathBuf,
) -> Result<ImportSettings, AppError> {
    service.preference(Preference::RemoveWatch(path)).await
}
