//! Guarded, locked, non-dumpable memory regions for cham3 secret material.
//!
//! This is the reviewed unsafe boundary for secret storage (PROJECT.md §4.5). It
//! exposes one safe type, [`secret_region_t`], whose memory is:
//!
//! * a private anonymous mapping, zero-initialized by the kernel;
//! * surrounded by one inaccessible (`PROT_NONE`) guard page on each side, with the
//!   secret right-aligned against the trailing guard so a one-byte overrun faults;
//! * locked with `mlock(2)`; allocation fails closed if locking fails;
//! * excluded from core dumps (`MADV_DONTDUMP`) and wiped in forked children
//!   (`MADV_WIPEONFORK`); allocation fails closed if either advice fails;
//! * zeroized with volatile writes, unlocked and unmapped on drop.
//!
//! # Safety invariants
//!
//! For every live `secret_region_t`:
//!
//! 1. `mapping .. mapping + mapping_length` is a mapping created by this module and
//!    not shared with any other object. It is unmapped exactly once, in `Drop`.
//! 2. `mapping_length == data_capacity + 2 * page_size`; the data pages start at
//!    `mapping + page_size` and are readable and writable; the first and last pages
//!    are `PROT_NONE` once construction succeeds.
//! 3. `data_offset + length == data_capacity`, `length > 0`, `length <= MaxRegionLength`,
//!    and all of these were computed with checked arithmetic, so the secret slice lies
//!    inside the data pages.
//! 4. Slices returned by [`secret_region_t::as_slice`] and
//!    [`secret_region_t::as_mut_slice`] borrow `self`, so they cannot outlive the
//!    mapping and Rust's aliasing rules hold (shared xor mutable).
//! 5. The type holds raw pointers, so it is neither `Send` nor `Sync`; a region is
//!    used and dropped by the thread that created it.
//!
//! # Limits
//!
//! These mechanisms reduce exposure; they do not protect against a compromised kernel
//! or a privileged process reading this process's memory (PROJECT.md §3.4). Copies a
//! caller makes outside the region (for example, inside a cryptographic library's
//! stack frames) are not covered; process-wide `mlockall`, non-dumpable state and core
//! limits (cham3_os_adapter) are required alongside this type. Zeroization runs in
//! `Drop`, which panic=abort or process death bypass; confidentiality then relies on
//! the kernel discarding the process's locked, non-dumpable pages.
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects,
        clippy::as_conversions
    )
)]

use core::ffi::c_void;
use core::fmt;
use core::ptr::NonNull;

use rustix::io::Errno;
use rustix::mm::{Advice, MapFlags, MprotectFlags, ProtFlags};
use zeroize::Zeroize;

/// Largest secret region accepted, in bytes.
pub const MaxRegionLength: usize = 1024 * 1024;

/// Reason a secret region could not be created. Contains no secret data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum secure_memory_error_t {
    /// Requested length was zero or larger than [`MaxRegionLength`].
    invalid_length,
    /// Size computation overflowed.
    size_overflow,
    /// `mmap(2)` failed.
    map_failed(Errno),
    /// A guard page could not be made inaccessible.
    guard_failed(Errno),
    /// `mlock(2)` failed (for example `RLIMIT_MEMLOCK` exhausted).
    lock_failed(Errno),
    /// `MADV_DONTDUMP` failed.
    dont_dump_failed(Errno),
    /// `MADV_WIPEONFORK` failed.
    wipe_on_fork_failed(Errno),
}

impl fmt::Display for secure_memory_error_t {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::invalid_length => formatter.write_str("invalid secret region length"),
            Self::size_overflow => formatter.write_str("secret region size overflow"),
            Self::map_failed(errno) => write!(formatter, "secret region mmap failed: {errno}"),
            Self::guard_failed(errno) => {
                write!(formatter, "secret region guard page failed: {errno}")
            }
            Self::lock_failed(errno) => write!(formatter, "secret region mlock failed: {errno}"),
            Self::dont_dump_failed(errno) => {
                write!(formatter, "secret region MADV_DONTDUMP failed: {errno}")
            }
            Self::wipe_on_fork_failed(errno) => {
                write!(formatter, "secret region MADV_WIPEONFORK failed: {errno}")
            }
        }
    }
}

impl std::error::Error for secure_memory_error_t {}

/// An exclusively owned, page-guarded, memory-locked, non-dumpable secret buffer.
///
/// Deliberately implements no `Clone`, `Copy`, `Debug`, `Display` or serialization.
pub struct secret_region_t {
    mapping: NonNull<c_void>,
    mapping_length: usize,
    data_pages: NonNull<u8>,
    data_capacity: usize,
    data_offset: usize,
    length: usize,
}

impl secret_region_t {
    /// Allocates a zero-filled secret region of exactly `length` bytes.
    ///
    /// Fails closed: if any protection step fails, the partial mapping is zeroized and
    /// released and an error is returned.
    pub fn new(length: usize) -> Result<Self, secure_memory_error_t> {
        if length == 0 || length > MaxRegionLength {
            return Err(secure_memory_error_t::invalid_length);
        }
        let page_size = rustix::param::page_size();
        let pages = length.div_ceil(page_size);
        let data_capacity = pages
            .checked_mul(page_size)
            .ok_or(secure_memory_error_t::size_overflow)?;
        let guard_total = page_size
            .checked_mul(2)
            .ok_or(secure_memory_error_t::size_overflow)?;
        let mapping_length = data_capacity
            .checked_add(guard_total)
            .ok_or(secure_memory_error_t::size_overflow)?;
        let data_offset = data_capacity
            .checked_sub(length)
            .ok_or(secure_memory_error_t::size_overflow)?;
        let tail_guard_offset = page_size
            .checked_add(data_capacity)
            .ok_or(secure_memory_error_t::size_overflow)?;

        // SAFETY: requests a new private anonymous mapping with a null address hint, so
        // the kernel selects an address that cannot overlap any existing Rust object.
        // The length is non-zero and was computed with checked arithmetic.
        let raw = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                mapping_length,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE,
            )
        }
        .map_err(secure_memory_error_t::map_failed)?;
        let mapping = NonNull::new(raw).ok_or(secure_memory_error_t::map_failed(Errno::NOMEM))?;
        // In-bounds by invariant 2: page_size < mapping_length. wrapping_add performs
        // no dereference and cannot wrap for an offset inside the mapping.
        let data_pages_raw = mapping.as_ptr().cast::<u8>().wrapping_add(page_size);
        let data_pages =
            NonNull::new(data_pages_raw).ok_or(secure_memory_error_t::map_failed(Errno::NOMEM))?;

        // From here on, Drop releases the mapping on every error path.
        let region = Self {
            mapping,
            mapping_length,
            data_pages,
            data_capacity,
            data_offset,
            length,
        };

        let tail_guard = mapping
            .as_ptr()
            .cast::<u8>()
            .wrapping_add(tail_guard_offset);
        // SAFETY: the first page of the mapping is owned by `region` and no reference
        // to it exists; making it inaccessible cannot invalidate any Rust borrow.
        unsafe { rustix::mm::mprotect(mapping.as_ptr(), page_size, MprotectFlags::empty()) }
            .map_err(secure_memory_error_t::guard_failed)?;
        // SAFETY: the last page (mapping + page_size + data_capacity) lies inside the
        // mapping by invariant 2 and no reference to it exists.
        unsafe {
            rustix::mm::mprotect(
                tail_guard.cast::<c_void>(),
                page_size,
                MprotectFlags::empty(),
            )
        }
        .map_err(secure_memory_error_t::guard_failed)?;
        // SAFETY: locks only the data pages owned by `region`; mlock does not access
        // the memory through Rust references.
        unsafe { rustix::mm::mlock(data_pages_raw.cast::<c_void>(), data_capacity) }
            .map_err(secure_memory_error_t::lock_failed)?;
        // SAFETY: advises only the data pages owned by `region`; MADV_DONTDUMP does
        // not change their contents.
        unsafe {
            rustix::mm::madvise(
                data_pages_raw.cast::<c_void>(),
                data_capacity,
                Advice::LinuxDontDump,
            )
        }
        .map_err(secure_memory_error_t::dont_dump_failed)?;
        // SAFETY: advises only the data pages owned by `region`; MADV_WIPEONFORK only
        // affects children created by fork(2), never this address space.
        unsafe {
            rustix::mm::madvise(
                data_pages_raw.cast::<c_void>(),
                data_capacity,
                Advice::LinuxWipeOnFork,
            )
        }
        .map_err(secure_memory_error_t::wipe_on_fork_failed)?;
        Ok(region)
    }

    /// Length of the secret in bytes (never zero).
    pub fn len(&self) -> usize {
        self.length
    }

    /// Always false: zero-length regions are rejected at construction.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Read access to the secret bytes.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: by invariants 2-3 the `length` bytes at data_ptr() lie inside the
        // readable, initialized (kernel zero-filled) data pages owned by `self`; the
        // returned slice borrows `self` immutably (invariant 4).
        unsafe { core::slice::from_raw_parts(self.data_ptr(), self.length) }
    }

    /// Write access to the secret bytes.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: as for as_slice; the slice borrows `self` mutably, so it is the only
        // live reference to these bytes (invariant 4).
        unsafe { core::slice::from_raw_parts_mut(self.data_ptr(), self.length) }
    }

    fn data_ptr(&self) -> *mut u8 {
        // In-bounds by invariant 3; no dereference.
        self.data_pages.as_ptr().wrapping_add(self.data_offset)
    }

    #[cfg(test)]
    fn data_pages_range(&self) -> (usize, usize) {
        let start = self.data_pages.as_ptr() as usize;
        (start, start + self.data_capacity)
    }
}

impl Drop for secret_region_t {
    fn drop(&mut self) {
        // SAFETY: the data pages are readable and writable (invariant 2) and owned by
        // `self`; `&mut self` guarantees no other reference exists while zeroizing.
        let pages = unsafe {
            core::slice::from_raw_parts_mut(self.data_pages.as_ptr(), self.data_capacity)
        };
        pages.zeroize();
        #[cfg(test)]
        tests::record_drop_zeroized(pages);
        // Errors are ignored: the pages are already zeroized, and munmap below
        // releases any lock regardless.
        // SAFETY: unlocks only this region's data pages.
        let _ = unsafe {
            rustix::mm::munlock(
                self.data_pages.as_ptr().cast::<c_void>(),
                self.data_capacity,
            )
        };
        // SAFETY: unmaps exactly the mapping created in new() (invariant 1). The
        // `pages` slice above is not used after this point and no other reference can
        // exist because Drop holds `&mut self`.
        let _ = unsafe { rustix::mm::munmap(self.mapping.as_ptr(), self.mapping_length) };
    }
}

#[cfg(test)]
mod tests;
