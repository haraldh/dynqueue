//! `dua`: a tiny parallel disk-usage tool, in the spirit of `du`.
//!
//! One `for_each_dyn` callback does everything: reading a directory enqueues its
//! entries, so new work is discovered by the same callback that consumes old
//! work. Filesystem errors are enqueued as work items too, which keeps them off
//! the panic path.
//!
//! ```sh
//! cargo run --release --example dua -- --dirs /usr
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use dynqueue::{DynQueueHandle, IntoDynQueue as _};

/// One unit of work. The item carries its own state, because the shared callback
/// cannot capture anything per item.
enum Task {
    /// Walk this directory. `total` accumulates the bytes of the files directly
    /// in it; when the walk finishes, the directory's total (its own bytes plus
    /// the totals its subdirectories handed in) is added to `parent`, which is
    /// `None` only for a requested root. `recurse` is turned off for the
    /// contents of a followed symlink, so `-L` cannot loop forever.
    Dir {
        path: PathBuf,
        total: &'static Total,
        parent: Option<&'static Total>,
        recurse: bool,
    },
    /// A failure while walking; processed as ordinary work so one unreadable
    /// directory does not abort the run.
    Error(String),
}

type Handle<'a> = DynQueueHandle<'a, Task, RwLock<Vec<Task>>>;

/// Bytes counted for one directory, plus what its subdirectories handed in.
struct Total {
    bytes: AtomicU64,
    children: AtomicU64,
}

impl Total {
    const fn new() -> Self {
        Total {
            bytes: AtomicU64::new(0),
            children: AtomicU64::new(0),
        }
    }

    fn own(&self, bytes: u64) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    fn subtotal(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed) + self.children.load(Ordering::Relaxed)
    }

    /// Called by a child walk once it is complete: `subtotal` is the child's
    /// full total, which grows both our tally and the per-directory entry.
    fn report_child(&'static self, subtotal: u64, path: &Path) {
        self.children.fetch_add(subtotal, Ordering::Relaxed);
        if let Some(totals) = TOTALS.lock().unwrap().as_ref() {
            totals.add(path, subtotal);
        }
    }
}

const NUM_SHARDS: usize = 64;

/// Sharded map from directory path to total bytes, used for `--dirs`. Every
/// directory gets exactly one entry, written once when its walk completes.
struct ShardedTotals([Mutex<HashMap<PathBuf, u64>>; NUM_SHARDS]);

impl ShardedTotals {
    fn new() -> Self {
        ShardedTotals(std::array::from_fn(|_| Mutex::new(HashMap::new())))
    }

    fn add(&self, path: &Path, bytes: u64) {
        let mut h = 0xcbf2_9ce4_8422_2325; // FNV-1a over the path bytes
        for byte in path.to_string_lossy().as_bytes() {
            h ^= u64::from(*byte);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        let shard = &self.0[(h as usize) % NUM_SHARDS];
        *shard.lock().unwrap().entry(path.to_path_buf()).or_default() += bytes;
    }

    fn dump(&self) -> Vec<(PathBuf, u64)> {
        self.0
            .iter()
            .flat_map(|shard| shard.lock().unwrap().clone())
            .collect()
    }
}

static TOTALS: Mutex<Option<ShardedTotals>> = Mutex::new(None);
static FILES: Total = Total::new(); // `bytes` = bytes of all regular files
static FILE_COUNT: AtomicUsize = AtomicUsize::new(0);
static DIR_COUNT: AtomicUsize = AtomicUsize::new(0);
static SYMLINK_BYTES: AtomicU64 = AtomicU64::new(0);
static SYMLINK_COUNT: AtomicUsize = AtomicUsize::new(0);
static ERROR_COUNT: AtomicUsize = AtomicUsize::new(0);
static FOLLOW: AtomicBool = AtomicBool::new(false);

/// The single callback shared by all workers: consume one task, maybe enqueue
/// more.
fn work(handle: Handle<'_>, task: Task) {
    match task {
        Task::Error(message) => {
            ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
            eprintln!("dua: {message}");
        }
        Task::Dir {
            path,
            total,
            parent,
            recurse,
        } => {
            match fs::read_dir(&path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let entry_path = entry.path();
                        let entry_type = match entry.file_type() {
                            Ok(file_type) => file_type,
                            Err(e) => {
                                handle.enqueue(Task::Error(format!("{entry_path:?}: {e}")));
                                continue;
                            }
                        };
                        if entry_type.is_symlink() {
                            match entry.metadata() {
                                Ok(meta) if FOLLOW.load(Ordering::Relaxed) && meta.is_dir() => {
                                    // Walk the target's own files, but never
                                    // descend through it: a symlink loop stays
                                    // one level deep.
                                    let child = Total::new();
                                    child.children.fetch_add(meta.len(), Ordering::Relaxed);
                                    handle.enqueue(Task::Dir {
                                        path: entry_path,
                                        total: Box::leak(Box::new(child)),
                                        parent: Some(total),
                                        recurse: false,
                                    });
                                }
                                Ok(meta) => {
                                    SYMLINK_BYTES.fetch_add(meta.len(), Ordering::Relaxed);
                                    SYMLINK_COUNT.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(e) => {
                                    handle.enqueue(Task::Error(format!("{entry_path:?}: {e}")));
                                }
                            }
                            continue;
                        }
                        match entry.metadata() {
                            Ok(_) if entry_type.is_dir() => {
                                DIR_COUNT.fetch_add(1, Ordering::Relaxed);
                                handle.enqueue(Task::Dir {
                                    path: entry_path,
                                    total: Box::leak(Box::new(Total::new())),
                                    parent: Some(total),
                                    recurse,
                                });
                            }
                            Ok(meta) if entry_type.is_file() => {
                                total.own(meta.len());
                                FILES.own(meta.len());
                                FILE_COUNT.fetch_add(1, Ordering::Relaxed);
                            }
                            Ok(_) => {} // fifos, sockets, devices: no size
                            Err(e) => handle.enqueue(Task::Error(format!("{entry_path:?}: {e}"))),
                        }
                    }
                    // Every subdirectory drain has already finished and
                    // reported in (the queue only went empty after all of
                    // them), so the total is final: publish it.
                    let subtotal = total.subtotal();
                    if let Some(totals) = TOTALS.lock().unwrap().as_ref() {
                        totals.add(&path, subtotal);
                    }
                    if let Some(parent) = parent {
                        parent.report_child(subtotal, &path);
                    }
                }
                Err(e) => handle.enqueue(Task::Error(format!("{path:?}: {e}"))),
            }
        }
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn main() -> ExitCode {
    let mut dirs = false;
    let mut follow = false;
    let mut paths: Vec<PathBuf> = Vec::new();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-d" | "--dirs" => dirs = true,
            "-L" | "--dereference" => follow = true,
            "-h" | "--help" => {
                println!(
                    "usage: dua [-d|--dirs] [-L|--dereference] [PATH ...]\n\
                     \n\
                     Summarize disk usage in parallel with dynqueue.\n\
                     -d, --dirs          also print the 20 largest directories\n\
                     -L, --dereference   follow symlinked directories\n\
                                         (may double-count or loop forever)\n\
                     default path: the current directory"
                );
                return ExitCode::SUCCESS;
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }
    if paths.is_empty() {
        paths.push(std::env::current_dir().expect("no current directory"));
    }
    FOLLOW.store(follow, Ordering::Relaxed);
    if dirs {
        *TOTALS.lock().unwrap() = Some(ShardedTotals::new());
    }

    // One accumulator per requested root, kept alive for the lifetime of the
    // run so tasks can reference it by reference.
    let mut totals: Vec<&'static Total> = Vec::new();
    let mut tasks = Vec::new();
    for path in &paths {
        let total = Box::leak(Box::new(Total::new()));
        totals.push(total);
        tasks.push(Task::Dir {
            path: path.clone(),
            total,
            parent: None,
            recurse: true,
        });
    }

    tasks.into_dyn_queue().for_each_dyn(work);

    for (path, total) in paths.iter().zip(&totals) {
        let bytes = total.subtotal();
        println!("{}\t{}", human(bytes), path.display());
    }
    let failed = ERROR_COUNT.load(Ordering::Relaxed) > 0;
    eprintln!(
        "{} files ({}), {} directories, {} symlinks ({}), {} errors",
        FILE_COUNT.load(Ordering::Relaxed),
        human(FILES.bytes.load(Ordering::Relaxed)),
        DIR_COUNT.load(Ordering::Relaxed),
        SYMLINK_COUNT.load(Ordering::Relaxed),
        human(SYMLINK_BYTES.load(Ordering::Relaxed)),
        ERROR_COUNT.load(Ordering::Relaxed),
    );

    if dirs {
        let mut entries = TOTALS
            .lock()
            .unwrap()
            .as_ref()
            .map(ShardedTotals::dump)
            .unwrap_or_default();
        entries.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.1));
        for (path, bytes) in entries.into_iter().take(20) {
            println!("{}\t{}", human(bytes), path.display());
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
