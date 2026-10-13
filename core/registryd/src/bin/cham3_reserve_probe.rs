//! Restrictive-reserve probe (M2-005 review R03): exercises the journal store's
//! preallocated reserve on an actual filesystem.
//!
//! `cham3_reserve_probe <root> allocate-only` creates a journal under `<root>` and
//! reports whether its reserve is preallocated with real blocks.
//!
//! `cham3_reserve_probe <root> full-disk` additionally fills the filesystem that holds
//! `<root>` with an unrelated writer until `ENOSPC`, then shows that a restrictive
//! append becomes durable from the reserve, that a non-restrictive append is refused
//! while the reserve cannot be refilled, that restrictive appends go on, and that the
//! first non-restrictive append after the filler is removed refills the reserve. It
//! refuses to run unless `<root>` is on a tmpfs of at most 16 MiB (a disposable
//! filesystem: `docker run --tmpfs /full-disk:rw,size=2m,...`); it never fills an
//! ordinary filesystem.
//!
//! Records are synthetic registry records; the store validates nothing (validation is
//! the registry's). Exit status: 0 verified, 2 a check failed, 3 refused to run.
#![forbid(unsafe_code)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cham3_registryd::journal::{
    JournalFileName, ReserveBytes, ReserveFileName, journal_file_t, journal_store_t, out_of_space,
    store_error_t, system_io_t,
};
use cham3_registryd::record::{
    lifecycle_record_t, parse_journal, record_body_t, registry_record_t,
};

const Uid: u32 = 1001;
/// Largest tmpfs the full-disk mode agrees to fill.
const MaxFillBytes: u64 = 16 * 1024 * 1024;
const FillChunk: usize = 4096;

enum failure_t {
    check(String),
    refused(String),
}

fn check(message: impl Into<String>) -> failure_t {
    failure_t::check(message.into())
}

fn store_failure(error: store_error_t) -> failure_t {
    check(format!("store error {error:?}"))
}

fn io_failure(error: std::io::Error) -> failure_t {
    check(format!("io error {error}"))
}

fn record(sequence: u64) -> Vec<u8> {
    registry_record_t {
        sequence,
        time: "2026-10-13T09:00:00Z".to_owned(),
        body: record_body_t::lifecycle(lifecycle_record_t::restriction {
            grant_ids: vec![[0x5a; 16]],
        }),
    }
    .encode()
}

fn allocated_bytes(path: &Path) -> Result<u64, failure_t> {
    Ok(fs::metadata(path)
        .map_err(io_failure)?
        .blocks()
        .saturating_mul(512))
}

/// The tmpfs mount holding `path` and its declared size, from /proc/self/mounts.
fn small_tmpfs_mount(path: &Path) -> Result<PathBuf, failure_t> {
    let canonical = fs::canonicalize(path).map_err(io_failure)?;
    let mounts = fs::read_to_string("/proc/self/mounts").map_err(io_failure)?;
    let mut best: Option<(PathBuf, String, String)> = None;
    for line in mounts.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_), Some(point), Some(kind), Some(options)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let point = PathBuf::from(point.replace("\\040", " "));
        if canonical.starts_with(&point)
            && best
                .as_ref()
                .is_none_or(|(current, _, _)| point.as_os_str().len() > current.as_os_str().len())
        {
            best = Some((point, kind.to_owned(), options.to_owned()));
        }
    }
    let (point, kind, options) =
        best.ok_or_else(|| failure_t::refused("no mount found for the root".to_owned()))?;
    if kind != "tmpfs" {
        return Err(failure_t::refused(format!(
            "{} is {kind}, not tmpfs",
            point.display()
        )));
    }
    let size = options
        .split(',')
        .find_map(|option| option.strip_prefix("size="))
        .and_then(parse_size)
        .ok_or_else(|| failure_t::refused("tmpfs without a size= option".to_owned()))?;
    if size > MaxFillBytes {
        return Err(failure_t::refused(format!(
            "tmpfs of {size} bytes exceeds the {MaxFillBytes}-byte limit"
        )));
    }
    Ok(point)
}

fn parse_size(text: &str) -> Option<u64> {
    let (digits, factor) = match text.strip_suffix(['k', 'K']) {
        Some(digits) => (digits, 1024u64),
        None => match text.strip_suffix(['m', 'M']) {
            Some(digits) => (digits, 1024 * 1024),
            None => match text.strip_suffix(['g', 'G']) {
                Some(digits) => (digits, 1024 * 1024 * 1024),
                None => (text, 1),
            },
        },
    };
    digits.parse::<u64>().ok()?.checked_mul(factor)
}

struct probe_t {
    store: journal_store_t,
    journal: journal_file_t,
    root: PathBuf,
}

fn open(root: &Path) -> Result<probe_t, failure_t> {
    fs::create_dir_all(root).map_err(io_failure)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(io_failure)?;
    let owner = fs::metadata(root).map_err(io_failure)?.uid();
    let store = journal_store_t::open(root, owner, &system_io_t).map_err(store_failure)?;
    let journal = store
        .create_journal(Uid, &[0x11; 32], &record(1), &system_io_t)
        .map_err(store_failure)?;
    Ok(probe_t {
        store,
        journal,
        root: root.to_path_buf(),
    })
}

fn report_reserve(probe: &probe_t) -> Result<(), failure_t> {
    let reserve = probe
        .root
        .join("users")
        .join(Uid.to_string())
        .join(ReserveFileName);
    let length = fs::metadata(&reserve).map_err(io_failure)?.len();
    let blocks = allocated_bytes(&reserve)?;
    println!("reserve_bytes {length}");
    println!("reserve_allocated_bytes {blocks}");
    println!("reserve_promised_bytes {ReserveBytes}");
    if length != ReserveBytes || blocks < ReserveBytes {
        return Err(check("the reserve is not preallocated with real blocks"));
    }
    println!("reserve_preallocated");
    let _ = &probe.store;
    Ok(())
}

fn append(probe: &mut probe_t, restrictive: bool) -> Result<u64, store_error_t> {
    let sequence = probe
        .journal
        .next_sequence()
        .ok_or(store_error_t::too_large)?;
    probe
        .journal
        .append(&record(sequence), restrictive, &system_io_t)
}

fn full_disk(probe: &mut probe_t) -> Result<(), failure_t> {
    let mount = small_tmpfs_mount(&probe.root)?;
    println!("tmpfs_mount {}", mount.display());
    let journal_path = probe
        .root
        .join("users")
        .join(Uid.to_string())
        .join(JournalFileName);
    // Align the journal so that the next frame needs a new block.
    let frame = journal_file_t::frame_length(record(1).len()).ok_or_else(|| check("frame"))?;
    let mut aligned = false;
    for _ in 0..64 {
        let length = fs::metadata(&journal_path).map_err(io_failure)?.len();
        if 4096u64.saturating_sub(length % 4096) < frame {
            aligned = true;
            break;
        }
        append(probe, false).map_err(store_failure)?;
    }
    if !aligned {
        return Err(check("could not align the journal to a block boundary"));
    }
    // An unrelated writer fills the filesystem.
    let filler_path = mount.join("cham3_reserve_probe_filler");
    let mut filler = fs::File::create(&filler_path).map_err(io_failure)?;
    let mut filled: u64 = 0;
    loop {
        match filler.write_all(&[0u8; FillChunk]) {
            Ok(()) => {
                filled = filled.saturating_add(4096);
                if filled > MaxFillBytes.saturating_mul(2) {
                    let _ = fs::remove_file(&filler_path);
                    return Err(failure_t::refused(
                        "the filesystem did not fill up".to_owned(),
                    ));
                }
            }
            Err(error) if out_of_space(&error) => break,
            Err(error) => {
                let _ = fs::remove_file(&filler_path);
                return Err(io_failure(error));
            }
        }
    }
    filler.sync_all().map_err(io_failure)?;
    println!("filler_bytes {filled}");
    let result = exhausted(probe);
    // The unrelated writer closes and removes its file: space returns.
    drop(filler);
    let _ = fs::remove_file(&filler_path);
    result?;
    // Space is back: the next non-restrictive append refills the reserve.
    let sequence = append(probe, false).map_err(store_failure)?;
    println!("append_after_space_returned ok seq={sequence}");
    if probe.journal.reserve_bytes() != ReserveBytes {
        return Err(check("the reserve was not refilled"));
    }
    report_reserve(probe)?;
    println!("reserve_replenished");
    let bytes = fs::read(&journal_path).map_err(io_failure)?;
    let parsed =
        parse_journal(&bytes, Uid).map_err(|defect| check(format!("journal {defect:?}")))?;
    println!("journal_records {}", parsed.records.len());
    if u64::try_from(parsed.records.len()).ok() != Some(probe.journal.record_count()) {
        return Err(check("the journal does not hold the expected records"));
    }
    println!("journal_chained");
    Ok(())
}

/// With the filesystem full: a restrictive append takes the reserve, a non-restrictive
/// append is refused, a further restrictive append still succeeds.
fn exhausted(probe: &mut probe_t) -> Result<(), failure_t> {
    let sequence = append(probe, true).map_err(|error| {
        check(format!(
            "restrictive append on the full filesystem failed: {error:?}"
        ))
    })?;
    println!("restrictive_append_on_full_filesystem ok seq={sequence}");
    println!(
        "reserve_bytes_after_release {}",
        probe.journal.reserve_bytes()
    );
    if probe.journal.reserve_bytes() != 0 {
        return Err(check("the reserve was not released"));
    }
    match append(probe, false) {
        Err(store_error_t::reserve_depleted) => println!("admission_refused reserve_depleted"),
        other => {
            return Err(check(format!(
                "a non-restrictive append must be refused while the reserve is depleted: {other:?}"
            )));
        }
    }
    let sequence = append(probe, true)
        .map_err(|error| check(format!("second restrictive append failed: {error:?}")))?;
    println!("second_restrictive_append ok seq={sequence}");
    if probe.journal.failed() {
        return Err(check("the journal was marked failed"));
    }
    Ok(())
}

fn run(root: &Path, mode: &str) -> Result<(), failure_t> {
    let mut probe = open(root)?;
    report_reserve(&probe)?;
    match mode {
        "allocate-only" => Ok(()),
        "full-disk" => full_disk(&mut probe),
        other => Err(failure_t::refused(format!("unknown mode {other}"))),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(root), Some(mode), None) = (arguments.first(), arguments.get(1), arguments.get(2))
    else {
        eprintln!("usage: cham3_reserve_probe <root> allocate-only|full-disk");
        return ExitCode::from(3);
    };
    let root = PathBuf::from(root);
    let outcome = run(&root, mode);
    let _ = fs::remove_dir_all(&root);
    match outcome {
        Ok(()) => {
            println!("reserve_verified");
            ExitCode::SUCCESS
        }
        Err(failure_t::check(message)) => {
            println!("check_failed {message}");
            ExitCode::from(2)
        }
        Err(failure_t::refused(message)) => {
            println!("refusing_to_fill {message}");
            ExitCode::from(3)
        }
    }
}
