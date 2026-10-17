//! Session ownership, cleanup, and recovery through the public facade.

use std::io;
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use jilva::{Backend, OutputCommand, TerminalCapabilities, TestDoubleBackend};

#[test]
fn enter_and_leave_round_trip_cleanly() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    assert_eq!(backend.name(), "test-double");
    assert!(!backend.is_entered());
    backend.enter().expect("enter succeeds");
    assert!(backend.is_entered());
    assert!(backend.raw_mode_active());
    assert!(backend.alternate_screen_active());
    assert!(backend.mouse_capture_active());
    assert_eq!(
        backend.size().expect("size reports inside a session"),
        (80, 24)
    );
    backend.leave().expect("leave succeeds");
    assert!(!backend.is_entered());
    assert!(!backend.raw_mode_active());
    assert!(!backend.alternate_screen_active());
    assert!(!backend.mouse_capture_active());
    assert_eq!(backend.log(), ["enter", "leave"]);
}

#[test]
fn double_enter_fails_and_preserves_the_existing_snapshot_and_modes() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    backend.enter().expect("first enter succeeds");
    let snapshot = backend.capabilities();
    assert_eq!(
        backend.enter().expect_err("re-enter must fail").kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(backend.is_entered());
    assert!(backend.raw_mode_active());
    assert!(backend.alternate_screen_active());
    assert_eq!(backend.capabilities(), snapshot);
    backend.leave().expect("original session still restores");
    assert_eq!(backend.restoration_attempts(), 1);
}

#[test]
fn leave_is_idempotent_before_setup_and_after_successful_cleanup() {
    let mut backend = TestDoubleBackend::default();
    backend.leave().expect("leave before setup is safe");
    backend.enter().expect("enter succeeds");
    backend.leave().expect("first cleanup succeeds");
    backend.leave().expect("second cleanup is safe");
    assert!(!backend.is_entered());
    assert_eq!(backend.log(), ["enter", "leave"]);
    assert_eq!(backend.restoration_attempts(), 3);
}

#[test]
fn repeated_sessions_advance_snapshot_identity_without_stale_work() {
    let mut backend = TestDoubleBackend::with_script(
        TerminalCapabilities::conservative(0),
        vec![TestDoubleBackend::key('a')],
    );
    for expected_id in 1..=3 {
        backend.enter().expect("session enters");
        assert_eq!(backend.capabilities().snapshot_id, expected_id);
        if expected_id == 1 {
            assert_eq!(backend.pending(), 1);
        } else {
            assert_eq!(backend.read().expect("prior input was discarded"), None);
            assert!(backend.buffered_commands().is_empty());
        }
        backend.script(TestDoubleBackend::key('b'));
        backend
            .submit(&[OutputCommand::print_segment("unflushed")])
            .expect("output buffers");
        backend.leave().expect("session restores");
        assert_eq!(
            backend.pending(),
            0,
            "ready and staged input must both be discarded"
        );
        assert!(backend.buffered_commands().is_empty());
    }
    assert_eq!(backend.restoration_attempts(), 3);
    assert!(
        backend.captured_commands().is_empty(),
        "teardown never paints pending output"
    );
}

#[test]
fn partial_initialization_rolls_back_enabled_modes_and_can_recover() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    backend.fail_enter_partial(io::ErrorKind::PermissionDenied);
    assert_eq!(
        backend.enter().expect_err("partial setup fails").kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(!backend.is_entered());
    assert!(!backend.raw_mode_active());
    assert!(!backend.alternate_screen_active());
    assert!(!backend.mouse_capture_active());
    assert_eq!(backend.snapshot_id(), 0);
    assert_eq!(
        backend.call_log(),
        [
            "enter:partial-failure(PermissionDenied)",
            "rollback:mouse-capture",
            "rollback:alternate-screen",
            "rollback:raw-mode"
        ]
    );
    backend
        .leave()
        .expect("leave after complete rollback is idempotent");
    backend.enter().expect("recovery enter succeeds");
    assert!(backend.is_entered());
    assert_eq!(backend.snapshot_id(), 1);
}

#[test]
fn failed_rollback_blocks_reenter_until_cleanup_retries_the_dirty_mode() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    backend.fail_enter_partial(io::ErrorKind::ConnectionAborted);
    backend.fail_rollback_once(io::ErrorKind::BrokenPipe);
    let error = backend
        .enter()
        .expect_err("initial setup error is preserved");
    assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
    assert!(error.to_string().contains("ConnectionAborted"));
    assert!(error.to_string().contains("rollback failed (BrokenPipe)"));
    assert!(!backend.is_entered());
    assert!(backend.mouse_capture_active(), "first restoration failed");
    assert!(
        !backend.raw_mode_active(),
        "later restoration was still attempted"
    );
    assert!(
        !backend.alternate_screen_active(),
        "last restoration was still attempted"
    );
    assert_eq!(
        backend
            .enter()
            .expect_err("new setup must not cover dirty modes")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    backend.leave().expect("remaining restoration succeeds");
    assert!(!backend.mouse_capture_active());
    backend.enter().expect("clean session can enter");
}

#[test]
fn atomic_enter_failure_enables_no_modes_and_mints_no_snapshot() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    backend.fail_enter_once(io::ErrorKind::ConnectionRefused);
    assert_eq!(
        backend.enter().expect_err("injected enter fails").kind(),
        io::ErrorKind::ConnectionRefused
    );
    assert!(!backend.is_entered());
    assert!(!backend.raw_mode_active());
    assert!(!backend.alternate_screen_active());
    assert!(!backend.mouse_capture_active());
    assert_eq!(backend.snapshot_id(), 0);
    assert_eq!(backend.call_log(), ["enter:failed(ConnectionRefused)"]);
    backend.leave().expect("leave is still safe");
    backend
        .enter()
        .expect("one-shot setup failure is recoverable");
}

#[test]
fn input_error_consumes_nothing_and_cleanup_discards_stale_input() {
    let mut backend = TestDoubleBackend::with_script(
        TerminalCapabilities::conservative(0),
        vec![TestDoubleBackend::key('a')],
    );
    backend.enter().expect("enter succeeds");
    backend.fail_read_once(io::ErrorKind::ConnectionReset);
    assert_eq!(
        backend.read().expect_err("read failure is injected").kind(),
        io::ErrorKind::ConnectionReset
    );
    assert_eq!(backend.pending(), 1);
    assert!(backend.consumed_events().is_empty());
    backend
        .leave()
        .expect("cleanup after an input failure succeeds");
    assert!(!backend.raw_mode_active());
    backend.enter().expect("next session enters");
    assert_eq!(backend.read().expect("stale input was cleared"), None);
}

#[test]
fn output_error_discards_attempted_batch_and_allows_fresh_resubmission() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    let frame = vec![OutputCommand::print_segment("frame")];
    backend.submit(&frame).expect("submit buffers");
    backend.fail_flush_once(io::ErrorKind::BrokenPipe);
    assert_eq!(
        backend.flush().expect_err("flush fails").kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(backend.buffered_commands().is_empty());
    assert!(backend.captured_commands().is_empty());
    backend
        .flush()
        .expect("empty retry cannot replay the failed batch");
    assert!(backend.captured_commands().is_empty());
    backend
        .submit(&frame)
        .expect("caller resubmits a fresh complete batch");
    backend.flush().expect("new batch commits");
    assert_eq!(backend.captured_commands(), frame);
    backend
        .leave()
        .expect("cleanup after output failure succeeds");
    assert!(!backend.raw_mode_active());
}

#[test]
fn failed_cleanup_attempts_later_modes_and_retains_the_failed_mode_for_retry() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    backend.enter().expect("enter succeeds");
    backend.fail_leave_once(io::ErrorKind::Interrupted);
    assert_eq!(
        backend.leave().expect_err("cleanup fails").kind(),
        io::ErrorKind::Interrupted
    );
    assert!(
        !backend.is_entered(),
        "cleanup makes the session unusable even on failure"
    );
    assert!(backend.mouse_capture_active());
    assert!(!backend.raw_mode_active());
    assert!(!backend.alternate_screen_active());
    assert!(backend.size().is_err());
    assert!(backend.poll(Duration::ZERO).is_err());
    assert!(backend.read().is_err());
    assert!(backend.submit(&[OutputCommand::ShowCursor]).is_err());
    assert!(backend.flush().is_err());
    assert!(
        backend.enter().is_err(),
        "dirty mode ownership must be cleaned first"
    );
    backend.leave().expect("retry restores the remaining mode");
    assert!(!backend.is_entered());
    assert!(!backend.mouse_capture_active());
    assert_eq!(backend.restoration_attempts(), 2);
}

struct RestoreGuard<'a>(&'a mut TestDoubleBackend);

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.leave();
    }
}

#[test]
fn panic_unwind_restores_all_enabled_modes() {
    let mut backend = TestDoubleBackend::new(TestDoubleBackend::full_profile());
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        backend.enter().expect("enter succeeds");
        let _guard = RestoreGuard(&mut backend);
        panic!("simulated application panic");
    }));
    assert!(outcome.is_err());
    assert!(!backend.is_entered());
    assert!(!backend.raw_mode_active());
    assert!(!backend.alternate_screen_active());
    assert!(!backend.mouse_capture_active());
    assert_eq!(backend.restoration_attempts(), 1);
    assert_eq!(backend.log(), ["enter", "leave"]);
}

#[test]
fn drop_with_failing_cleanup_never_unwinds() {
    let mut backend = TestDoubleBackend::default();
    backend.enter().expect("enter succeeds");
    backend.fail_leave(io::ErrorKind::Other);
    assert!(std::panic::catch_unwind(AssertUnwindSafe(|| drop(backend))).is_ok());
}

#[test]
fn operations_require_a_live_session_before_setup_and_after_cleanup() {
    let mut backend = TestDoubleBackend::default();
    for after_cleanup in [false, true] {
        if after_cleanup {
            backend.enter().expect("enter succeeds");
            backend.leave().expect("leave succeeds");
        }
        assert_eq!(
            backend.size().expect_err("size requires a session").kind(),
            io::ErrorKind::NotConnected
        );
        assert_eq!(
            backend
                .poll(Duration::ZERO)
                .expect_err("poll requires a session")
                .kind(),
            io::ErrorKind::NotConnected
        );
        assert_eq!(
            backend.read().expect_err("read requires a session").kind(),
            io::ErrorKind::NotConnected
        );
        assert_eq!(
            backend
                .submit(&[OutputCommand::print_segment("x")])
                .expect_err("submit requires a session")
                .kind(),
            io::ErrorKind::NotConnected
        );
        assert_eq!(
            backend
                .flush()
                .expect_err("flush requires a session")
                .kind(),
            io::ErrorKind::NotConnected
        );
    }
    assert!(backend.buffered_commands().is_empty());
    assert!(backend.captured_commands().is_empty());
}
