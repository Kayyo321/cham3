//! Unit and subprocess fault tests for the secret region adapter.
//!
//! Faulting cases (guard-page overrun/underrun and RLIMIT_MEMLOCK exhaustion) run in
//! a child copy of this test binary, selected by the CHAM3_SECURE_MEMORY_CHILD
//! environment variable, so the parent can observe the signal or exit status. These
//! tests must run unprivileged: a process with CAP_IPC_LOCK ignores RLIMIT_MEMLOCK.

use super::*;

use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const ChildModeVariable: &str = "CHAM3_SECURE_MEMORY_CHILD";
const SignalSegv: i32 = 11;
const ChildSuccessCode: i32 = 0;
const ChildWrongResultCode: i32 = 3;

static DropCount: AtomicUsize = AtomicUsize::new(0);
static DropAllZero: AtomicBool = AtomicBool::new(true);

pub(super) fn record_drop_zeroized(pages: &[u8]) {
    if pages.iter().any(|byte| *byte != 0) {
        DropAllZero.store(false, Ordering::SeqCst);
    }
    DropCount.fetch_add(1, Ordering::SeqCst);
}

// Trait-absence probes: an inherent associated const shadows the trait default only
// when the bound holds, so these evaluate to true exactly when the trait is
// implemented.
struct trait_probe_t<item_t>(core::marker::PhantomData<item_t>);
trait trait_probe_default_t {
    const ImplementsDebug: bool = false;
    const ImplementsClone: bool = false;
    const ImplementsDisplay: bool = false;
}
impl<item_t> trait_probe_default_t for trait_probe_t<item_t> {}
#[allow(dead_code)]
impl<item_t: fmt::Debug> trait_probe_t<item_t> {
    const ImplementsDebug: bool = true;
}
#[allow(dead_code)]
impl<item_t: Clone> trait_probe_t<item_t> {
    const ImplementsClone: bool = true;
}
#[allow(dead_code)]
impl<item_t: fmt::Display> trait_probe_t<item_t> {
    const ImplementsDisplay: bool = true;
}

// The child's own harness output is captured, never inherited, so the parent's
// suite summary is the only one the M2 harness parses.
fn run_child(mode: &str) -> std::process::Output {
    let current_exe = std::env::current_exe().expect("test binary path");
    Command::new(current_exe)
        .args([
            "--exact",
            "tests::child_entry",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ChildModeVariable, mode)
        .output()
        .expect("spawn child test process")
}

fn smaps_flags_for(address: usize) -> String {
    let smaps = std::fs::read_to_string("/proc/self/smaps").expect("read /proc/self/smaps");
    let mut inside = false;
    for line in smaps.lines() {
        if let Some((range, _)) = line.split_once(' ') {
            if let Some((start, end)) = range.split_once('-') {
                if let (Ok(start), Ok(end)) = (
                    usize::from_str_radix(start, 16),
                    usize::from_str_radix(end, 16),
                ) {
                    inside = start <= address && address < end;
                    continue;
                }
            }
        }
        if inside {
            if let Some(flags) = line.strip_prefix("VmFlags:") {
                return flags.trim().to_string();
            }
        }
    }
    panic!("no smaps mapping contains {address:#x}");
}

#[test]
fn child_entry() {
    // Entered only as a child of the fault tests below; without the variable this is
    // a no-op so the normal suite run does not fault.
    let Ok(mode) = std::env::var(ChildModeVariable) else {
        return;
    };
    match mode.as_str() {
        "overrun" => {
            let mut region = secret_region_t::new(32).unwrap();
            let end = region.as_mut_slice().as_mut_ptr().wrapping_add(32);
            // SAFETY: deliberately invalid write one byte past the secret to prove the
            // trailing guard page faults. Expected to terminate with SIGSEGV.
            unsafe { end.write_volatile(0xA5) };
            std::process::exit(ChildWrongResultCode);
        }
        "underrun" => {
            let mut region = secret_region_t::new(4096).unwrap();
            let before = region.as_mut_slice().as_mut_ptr().wrapping_sub(1);
            // SAFETY: deliberately invalid write one byte before a page-sized secret,
            // inside the leading guard page. Expected to terminate with SIGSEGV.
            unsafe { before.write_volatile(0xA5) };
            std::process::exit(ChildWrongResultCode);
        }
        "memlock_exhausted" => {
            rustix::process::setrlimit(
                rustix::process::Resource::Memlock,
                rustix::process::Rlimit {
                    current: Some(0),
                    maximum: Some(0),
                },
            )
            .unwrap();
            let first = secret_region_t::new(32);
            let second = secret_region_t::new(32);
            let ok = matches!(first, Err(secure_memory_error_t::lock_failed(_)))
                && matches!(second, Err(secure_memory_error_t::lock_failed(_)));
            std::process::exit(if ok {
                ChildSuccessCode
            } else {
                ChildWrongResultCode
            });
        }
        other => panic!("unknown child mode {other}"),
    }
}

#[test]
fn region_is_zero_filled_and_round_trips() {
    let mut region = secret_region_t::new(48).unwrap();
    assert_eq!(region.len(), 48);
    assert!(!region.is_empty());
    assert!(region.as_slice().iter().all(|byte| *byte == 0));
    region.as_mut_slice().copy_from_slice(&[0x5Au8; 48]);
    assert!(region.as_slice().iter().all(|byte| *byte == 0x5A));
}

#[test]
fn invalid_lengths_are_rejected() {
    assert_eq!(
        secret_region_t::new(0).err(),
        Some(secure_memory_error_t::invalid_length)
    );
    assert_eq!(
        secret_region_t::new(MaxRegionLength + 1).err(),
        Some(secure_memory_error_t::invalid_length)
    );
    assert!(secret_region_t::new(MaxRegionLength).is_ok());
}

#[test]
fn data_pages_are_locked_not_dumpable_and_wiped_on_fork() {
    let region = secret_region_t::new(64).unwrap();
    let (start, _) = region.data_pages_range();
    let flags = smaps_flags_for(start);
    let flag_list: Vec<&str> = flags.split_whitespace().collect();
    for required in ["rd", "wr", "lo", "dd", "wf"] {
        assert!(
            flag_list.contains(&required),
            "missing VmFlag {required} in [{flags}]"
        );
    }
}

#[test]
fn guard_pages_are_inaccessible() {
    let region = secret_region_t::new(64).unwrap();
    let (start, end) = region.data_pages_range();
    let page_size = rustix::param::page_size();
    for guard in [start - page_size, end] {
        let flags = smaps_flags_for(guard);
        let flag_list: Vec<&str> = flags.split_whitespace().collect();
        assert!(!flag_list.contains(&"rd"), "guard readable: [{flags}]");
        assert!(!flag_list.contains(&"wr"), "guard writable: [{flags}]");
    }
}

#[test]
fn drop_zeroizes_entire_data_capacity() {
    let before = DropCount.load(Ordering::SeqCst);
    {
        let mut region = secret_region_t::new(100).unwrap();
        region.as_mut_slice().fill(0xFF);
    }
    assert!(DropCount.load(Ordering::SeqCst) > before);
    assert!(DropAllZero.load(Ordering::SeqCst));
}

#[test]
fn overrun_hits_trailing_guard_page() {
    let output = run_child("overrun");
    assert_eq!(
        output.status.signal(),
        Some(SignalSegv),
        "child output {output:?}"
    );
}

#[test]
fn underrun_hits_leading_guard_page() {
    let output = run_child("underrun");
    assert_eq!(
        output.status.signal(),
        Some(SignalSegv),
        "child output {output:?}"
    );
}

#[test]
fn memlock_exhaustion_fails_closed() {
    assert_ne!(
        rustix::process::geteuid().as_raw(),
        0,
        "run M2 tests unprivileged: root/CAP_IPC_LOCK ignores RLIMIT_MEMLOCK"
    );
    let output = run_child("memlock_exhausted");
    assert_eq!(
        output.status.code(),
        Some(ChildSuccessCode),
        "child output {output:?}"
    );
}

// Compile-time guarantees: this test module fails to build if secret_region_t gains
// Debug, Clone or Display. The positive controls prove the probe detects each trait.
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(!trait_probe_t::<secret_region_t>::ImplementsDebug);
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(!trait_probe_t::<secret_region_t>::ImplementsClone);
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(!trait_probe_t::<secret_region_t>::ImplementsDisplay);
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(trait_probe_t::<secure_memory_error_t>::ImplementsDebug);
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(trait_probe_t::<secure_memory_error_t>::ImplementsClone);
#[allow(clippy::assertions_on_constants)] // intentional compile-time check
const _: () = assert!(trait_probe_t::<secure_memory_error_t>::ImplementsDisplay);

#[test]
fn secret_region_has_no_debug_clone_or_display() {
    let probes = [
        trait_probe_t::<secret_region_t>::ImplementsDebug,
        trait_probe_t::<secret_region_t>::ImplementsClone,
        trait_probe_t::<secret_region_t>::ImplementsDisplay,
    ];
    assert_eq!(std::hint::black_box(probes), [false; 3]);
    let controls = [
        trait_probe_t::<secure_memory_error_t>::ImplementsDebug,
        trait_probe_t::<secure_memory_error_t>::ImplementsClone,
        trait_probe_t::<secure_memory_error_t>::ImplementsDisplay,
    ];
    assert_eq!(std::hint::black_box(controls), [true; 3]);
}
