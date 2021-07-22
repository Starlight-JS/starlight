/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use atomic::{Atomic, Ordering};
use core::{
    cell::Cell,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicUsize},
};
use parking_lot::{lock_api::RawMutex, Condvar, Mutex, RawMutex as Lock};

use crate::vm::VirtualMachineRef;

/// Used to bring all threads attached to it to a safepoint such
/// that e.g. a garbage collection can be performed.
pub struct GlobalSafepoint {
    barrier: Barrier,
    mutators: Cell<*mut Mutator>,
    mutators_lock: Lock,
    mutator_of_this_thread: *mut Mutator,
    active_safepoint_scopes: Cell<usize>,
}

impl GlobalSafepoint {
    pub fn new() -> Self {
        Self {
            barrier: Barrier {
                mutex: Mutex::new(()),
                cv_resume: Condvar::new(),
                cv_stopped: Condvar::new(),
                stopped: AtomicUsize::new(0),
                armed: Cell::new(false),
            },
            mutators: Cell::new(null_mut()),
            mutators_lock: Lock::INIT,
            mutator_of_this_thread: null_mut(),
            active_safepoint_scopes: Cell::new(0),
        }
    }

    pub fn is_active(&self) -> bool {
        self.active_safepoint_scopes.get() > 0
    }
    pub fn add_mutator(&self, mutator: *mut Mutator, callback: impl FnOnce()) {
        self.mutators_lock.lock();
        let mut mutators = self.mutators.get();

        // Additional code protected from safepoint
        callback();

        unsafe {
            if !mutators.is_null() {
                (*mutators).prev = mutator;
            }
            (*mutator).prev = null_mut();
            (*mutator).next = mutators;
            self.mutators.set(mutator);
            self.mutators_lock.unlock();
        }
    }

    pub fn remove_mutator(&self, mutator: *mut Mutator, callback: impl FnOnce()) {
        self.mutators_lock.lock();
        let mut mutators = self.mutators.get();

        // Additional code protected from safepoint
        callback();

        // Remove list from doubly-linked list
        unsafe {
            if !(*mutator).next.is_null() {
                (*(*mutator).next).prev = (*mutator).prev;
            }
            if !(*mutator).prev.is_null() {
                (*(*mutator).prev).next = (*mutator).next;
            } else {
                self.mutators.set((*mutator).next); //*mutators = (*mutator).next;
            }
            self.mutators_lock.unlock();
        }
    }

    pub fn enter_safepoint_scope(&self) {
        if self.active_safepoint_scopes.get() > 1 {
            return;
        }
        self.active_safepoint_scopes
            .set(self.active_safepoint_scopes.get() + 1);

        self.mutators_lock.lock();
        let mutators = self.mutators.get();

        let mut running = 0;
        unsafe {
            let mut head = mutators;

            while !head.is_null() {
                let expected = (*head).state.load(atomic::Ordering::Relaxed);

                loop {
                    let new_state = if expected == MutatorState::Parked {
                        MutatorState::ParkedSafepointRequested
                    } else {
                        MutatorState::SafepointRequested
                    };

                    if (*head)
                        .state
                        .compare_exchange(
                            expected,
                            new_state,
                            atomic::Ordering::SeqCst,
                            atomic::Ordering::Relaxed,
                        )
                        .is_ok()
                    {
                        if expected == MutatorState::Running {
                            running += 1;
                        } else {
                            assert_eq!(expected, MutatorState::Parked);
                        }
                        break;
                    }
                }
                head = (*head).next;
            }
        }
        drop(mutators);
        self.barrier
            .wait_until_running_threads_in_safepoint(running);
    }

    pub fn leave_safepoint_scope(&self) {
        self.active_safepoint_scopes
            .set(self.active_safepoint_scopes.get() - 1);

        unsafe {
            let mut head = self.mutators.get();
            while !head.is_null() {
                // We transition both ParkedSafepointRequested and Safepoint states to
                // Parked. While this is probably intuitive for ParkedSafepointRequested,
                // this might be surprising for Safepoint though. SafepointSlowPath() will
                // later unpark that thread again. Going through Parked means that a
                // background thread doesn't need to be waked up before the main thread can
                // start the next safepoint.

                let old_state = (*head).state.swap(MutatorState::Parked, Ordering::AcqRel);
                assert!(
                    old_state == MutatorState::ParkedSafepointRequested
                        || old_state == MutatorState::Safepoint
                );
                head = (*head).next;
            }
            self.barrier.disarm();
            self.mutators_lock.unlock();
        }
    }

    pub fn wait_in_safepoint(&self) {
        self.barrier.wait_in_safepoint();
    }

    pub fn wait_in_unpark(&self) {
        self.barrier.wait_in_unpark();
    }

    pub fn notify_park(&self) {
        self.barrier.notify_park();
    }
}

struct Barrier {
    armed: Cell<bool>,
    mutex: Mutex<()>,
    cv_resume: Condvar,
    cv_stopped: Condvar,
    stopped: AtomicUsize,
}
impl Barrier {
    fn arm(&self) {
        let guard = self.mutex.lock();
        assert!(!self.armed.get());
        self.armed.set(true);
        self.stopped.store(0, atomic::Ordering::Relaxed);
        drop(guard);
    }

    fn disarm(&self) {
        let guard = self.mutex.lock();
        assert!(self.armed.get());
        self.armed.set(false);
        self.stopped.store(0, atomic::Ordering::Relaxed);

        drop(guard);
        self.cv_resume.notify_all();
    }

    fn wait_until_running_threads_in_safepoint(&self, running: usize) {
        let mut guard = self.mutex.lock();
        while self.stopped.load(atomic::Ordering::Relaxed) < running {
            self.cv_stopped.wait(&mut guard);
        }
        debug_assert_eq!(self.stopped.load(Ordering::Relaxed), running);
    }

    fn notify_park(&self) {
        let guard = self.mutex.lock();
        self.stopped.fetch_add(1, Ordering::Relaxed);
        self.cv_stopped.notify_one();
        drop(guard);
    }

    fn wait_in_safepoint(&self) {
        let mut guard = self.mutex.lock();
        self.stopped.fetch_add(1, Ordering::Relaxed);
        self.cv_stopped.notify_one();
        while self.armed.get() {
            self.cv_resume.wait(&mut guard);
        }
    }

    fn wait_in_unpark(&self) {
        let mut guard = self.mutex.lock();
        while self.armed.get() {
            self.cv_resume.wait(&mut guard);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutatorState {
    /// Threads in this state need to be stopped in a safepoint.
    Running,
    /// Thread was parked, which means that the thread is not allowed to access
    /// or manipulate the heap in any way.
    Parked,
    /// SafepointRequested is used for Running threads to force Safepoint() and
    /// Park() into the slow path.
    SafepointRequested,
    /// Thread was stopped in a safepoint.
    Safepoint,

    /// This state is used for Parked background threads and forces Unpark() into
    /// the slow path. It prevents Unpark() to succeed before the safepoint
    ///operation is finished.
    ParkedSafepointRequested,
}
/// Mutator is used by the GC to track all threads with heap access in order to
/// stop them before performing a collection. Mutators can be either Parked or
/// Running and are in Parked mode when initialized.
///  - Running: Thread is allowed to access the heap but needs to give the GC the
///            chance to run regularly by manually invoking [Mutator::safepoint]. The
///            thread can be parked using ParkedScope.
///  - Parked:  Heap access is not allowed, so the GC will not stop this thread
///            for a collection. Useful when threads do not need heap access for
///            some time or for blocking operations like locking a mutex.
pub struct Mutator {
    next: *mut Self,
    prev: *mut Self,
    safepoint_requested: AtomicBool,
    state: Atomic<MutatorState>,
    state_change: Condvar,
    state_mutex: Mutex<()>,
    vm: VirtualMachineRef,
    main_thread: bool,
}

impl Mutator {
    pub fn safepoint(&self) {
        let current = self.state.load(Ordering::Relaxed);
    }

    #[cold]
    fn safepoint_slowpath(&self) {
        if self.main_thread {
            let mut vm = self.vm;
            vm.gc.gc();
        } else {
            let expected = MutatorState::SafepointRequested;
            assert!(self
                .state
                .compare_exchange(
                    expected,
                    MutatorState::Safepoint,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok());
            self.vm.safepoint.wait_in_safepoint();
        }
    }

    pub fn unpark(&self) {
        if self
            .state
            .compare_exchange(
                MutatorState::Parked,
                MutatorState::Running,
                Ordering::SeqCst,
                Ordering::Relaxed,
            )
            .is_err()
        {
            self.unpark_slowpath();
        }
    }

    pub fn park(&self) {
        if self
            .state
            .compare_exchange(
                MutatorState::Running,
                MutatorState::Parked,
                Ordering::SeqCst,
                Ordering::Relaxed,
            )
            .is_err()
        {
            self.park_slowpath();
        }
    }
    fn park_slowpath(&self) {
        if self.main_thread {
            let mut vm = self.vm;
            loop {
                vm.gc.gc();
                if self
                    .state
                    .compare_exchange(
                        MutatorState::Running,
                        MutatorState::Parked,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
            }
        } else {
            assert!(self
                .state
                .compare_exchange(
                    MutatorState::SafepointRequested,
                    MutatorState::ParkedSafepointRequested,
                    Ordering::SeqCst,
                    Ordering::Relaxed
                )
                .is_ok());
            self.vm.safepoint.notify_park();
        }
    }
    fn unpark_slowpath(&self) {
        if self.main_thread {
            assert!(self
                .state
                .compare_exchange(
                    MutatorState::ParkedSafepointRequested,
                    MutatorState::SafepointRequested,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                )
                .is_ok());
            let mut vm = self.vm;
            vm.gc.gc();
        } else {
            loop {
                if self
                    .state
                    .compare_exchange(
                        MutatorState::Parked,
                        MutatorState::ParkedSafepointRequested,
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .is_err()
                {
                    self.vm.safepoint.wait_in_unpark();
                } else {
                    return;
                }
            }
        }
    }
}
