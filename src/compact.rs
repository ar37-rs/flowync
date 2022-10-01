use crate::{error::Compact, AsyncSuspender};
use core::{
    fmt::{self, Debug, Formatter},
    sync::atomic::{AtomicBool, Ordering},
    task::Waker,
};
use std::{
    mem,
    sync::{Condvar, Mutex},
};
use std::{sync::Arc, thread};

enum CompactTypeOpt<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    Channel(S),
    Success(R),
    Error(Compact<E>),
    None,
}

impl<S, R, E> Default for CompactTypeOpt<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn default() -> Self {
        Self::None
    }
}

impl<S, R, E> Debug for CompactTypeOpt<S, R, E>
where
    S: Send + Debug,
    R: Send + Debug,
    E: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Channel(s) => f.debug_tuple("Channel").field(s).finish(),
            Self::Success(r) => f.debug_tuple("Success").field(r).finish(),
            Self::Error(e) => f.debug_tuple("Error").field(e).finish(),
            Self::None => write!(f, "None"),
        }
    }
}

impl<S, R, E> CompactTypeOpt<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn take(&mut self) -> Self {
        mem::take(self)
    }
}

struct InnerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    activated: AtomicBool,
    result_ready: AtomicBool,
    channel_present: AtomicBool,
    mtx: Mutex<CompactTypeOpt<S, R, E>>,
    cvar: Condvar,
    canceled: AtomicBool,
}

impl<S, R, E> Debug for InnerState<S, R, E>
where
    S: Send + Debug,
    R: Send + Debug,
    E: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerState")
            .field("result_ready", &self.result_ready)
            .field("channel_present", &self.channel_present)
            .field("mtx", &self.mtx)
            .field("cvar", &self.cvar)
            .field("canceled", &self.canceled)
            .field("activated", &self.activated)
            .finish()
    }
}

impl<S, R, E> Drop for InnerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn drop(&mut self) {}
}

/// State of the `CompactFlower`
pub struct CompactFlowerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    state: Arc<InnerState<S, R, E>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R, E> Debug for CompactFlowerState<S, R, E>
where
    S: Send + Debug,
    R: Send + Debug,
    E: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactFlowerState")
            .field("state", &self.state)
            .field("async_suspender", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

impl<S, R, E> CompactFlowerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    /// Get ID of the `CompactFlower`.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Cancel `CompactFlower`.
    ///
    /// will do nothing if not explicitly configured on the `CompactHandle`.
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::Relaxed);
    }

    /// Check if the `CompactFlower` is canceled
    pub fn is_canceled(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Check if the current `CompactFlower` is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }
}

impl<S, R, E> Clone for CompactFlowerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }
}

impl<S, R, E> Drop for CompactFlowerState<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn drop(&mut self) {}
}

/// A handle for the CompactFlower
pub struct CompactHandle<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    state: Arc<InnerState<S, R, E>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R, E> CompactHandle<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    /// Get ID of the `CompactFlower`.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Activate current `CompactFlower`
    pub fn activate(&self) {
        self.state.activated.store(true, Ordering::Relaxed);
    }

    /// Check if the current `CompactFlower` is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }

    /// Check if the current `CompactFlower` should be canceled
    pub fn should_cancel(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Send current progress value
    pub fn send(&self, s: S) {
        let mut mtx = self.state.mtx.lock().unwrap();
        {
            *mtx = CompactTypeOpt::Channel(s);
            self.state.channel_present.store(true, Ordering::Relaxed);
            self.async_suspender.1.store(false, Ordering::Relaxed);
        }
        drop(self.state.cvar.wait(mtx));
    }

    /// Send current progress value asynchronously.
    pub async fn send_async(&self, s: S) {
        {
            *self.state.mtx.lock().unwrap() = CompactTypeOpt::Channel(s);
            self.async_suspender.1.store(true, Ordering::Relaxed);
            self.state.channel_present.store(true, Ordering::Relaxed);
        }
        AsyncSuspender {
            inner: self.async_suspender.clone(),
        }
        .await
    }

    /// Set the `Ok` value of the `Result`.
    pub fn success(&self, r: R) {
        *self.state.mtx.lock().unwrap() = CompactTypeOpt::Success(r);
        self.state.result_ready.store(true, Ordering::Relaxed);
    }

    /// Set the `Err` value of the `Result`.
    pub fn error(&self, e: E) {
        *self.state.mtx.lock().unwrap() = CompactTypeOpt::Error(Compact::Suppose(e));
        self.state.result_ready.store(true, Ordering::Relaxed);
    }
}

impl<S, R, E> Drop for CompactHandle<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn drop(&mut self) {
        if thread::panicking() && !self.state.result_ready.load(Ordering::Relaxed) {
            self.state.channel_present.store(false, Ordering::Relaxed);
            let err = format!(
                "the compact flower handle with id: {} error panicked!",
                self.id
            );
            *self.state.mtx.lock().unwrap() = CompactTypeOpt::Error(Compact::Panicked(err));
            self.state.result_ready.store(true, Ordering::Relaxed);
        }
    }
}

impl<S, R, E> Debug for CompactHandle<S, R, E>
where
    S: Send + Debug,
    R: Send + Debug,
    E: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactHandle")
            .field("state", &self.state)
            .field("awaiting", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

pub enum CompactFinalizer<'a, S: Send, R: Send, E: Send> {
    Try(&'a CompactFlower<S, R, E>),
}

impl<S, R, E> CompactFinalizer<'_, S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    /// Try finalize `Result` of the `CompactFlower` (this fn will be called if only `Result` is available).
    pub fn finalize(self, f: impl FnOnce(Result<R, Compact<E>>)) {
        let Self::Try(compact_flower) = self;
        if compact_flower.state.result_ready.load(Ordering::Relaxed) {
            let result = move || {
                let result = compact_flower.state.mtx.lock().unwrap().take();
                compact_flower
                    .state
                    .result_ready
                    .store(false, Ordering::Relaxed);
                compact_flower
                    .state
                    .activated
                    .store(false, Ordering::Relaxed);
                result
            };
            let result = result();
            if let CompactTypeOpt::Success(value) = result {
                f(Ok(value))
            } else if let CompactTypeOpt::Error(err_type) = result {
                f(Err(err_type))
            }
        }
    }
}

/// A `Flower` with composable `Error` type.
///
/// Where:
///
/// `S` = type of the sender spsc channel value
///
/// `R` = type of `Ok` value of the `Result` (`Result<R, Compact<E>>`) and `E` is the type of `Compact` error cause.
///
/// # Quick Example:
///
///```
/// use flowync::{error::{Compact, IOError}, CompactFlower};
///
/// enum ErrMessage {
///    Segfault(String),
///    Other,
/// }
///
/// type TestCompactFlower = CompactFlower<u32, String, ErrMessage>;
///
/// fn fetch_things(id: usize) -> Result<String, IOError> {
///     let result =
///         Ok::<String, IOError>(format!("the compact flower with id: {} successfully completed fetching.", id));
///     let success = result?;
///     Ok(success)
/// }
///
/// fn main() {
///     let compact_flower: TestCompactFlower = CompactFlower::new(1);
///     std::thread::spawn({
///         let handle = compact_flower.handle();
///         // Activate
///         handle.activate();
///         move || {
///             for i in 0..10 {
///                 // Send current value through channel, will block the spawned thread
///                 // until the option value successfully being polled in the main thread.
///                 handle.send(i);
///                 // or handle.send_async(i).await; can be used from any multithreaded async runtime,
///             }
///             match fetch_things(handle.id()) {
///                 Ok(value) => {
///                     handle.success(value);
///                 }
///                 Err(e) => handle.error(ErrMessage::Segfault(e.to_string())),
///             }
///         }
///     });
///
///     let mut exit = false;
///
///     loop {
///         // Check if the compact flower is_active()
///         // and will deactivate itself if the result value successfully received.
///         if compact_flower.is_active() {
///             // another logic goes here...
///             // e.g:
///             // notify_loading_fn();
///
///             compact_flower
///                 .poll(|channel| {
///                     if let Some(value) = channel {
///                         println!("{}", value);
///                     }
///                 })
///                 .finalize(|result| {
///                     match result {
///                         Ok(value) => println!("{}", value),
///                         Err(Compact::Suppose(ErrMessage::Segfault(msg))) => {
///                             println!("{}", msg)
///                         }
///                         Err(Compact::Suppose(err_msg)) => {
///                             if let ErrMessage::Other = err_msg {
///                                 // Handle if any
///                             }
///                         }
///                         Err(Compact::Panicked(_msg)) => {
///                             // Handle things if stuff unexpectedly panicked at runtime.
///                         }
///                     }
///
///                     // Exit if finalized
///                     exit = true;
///                 });
///         }
///
///         if exit {
///             break;
///         }
///     }
/// }
/// ```
pub struct CompactFlower<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    state: Arc<InnerState<S, R, E>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R, E> CompactFlower<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(InnerState {
                activated: AtomicBool::new(false),
                result_ready: AtomicBool::new(false),
                channel_present: AtomicBool::new(false),
                mtx: Mutex::new(CompactTypeOpt::None),
                cvar: Condvar::new(),
                canceled: AtomicBool::new(false),
            }),
            async_suspender: Arc::new((Mutex::new(None), AtomicBool::new(false))),
            id,
        }
    }

    /// Get the ID.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the handle.
    pub fn handle(&self) -> CompactHandle<S, R, E> {
        self.state.canceled.store(false, Ordering::Relaxed);
        CompactHandle {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }

    /// Get the state
    ///
    /// Since `CompactFlower` itself is uncloneable to avoid data races, this is an alternative `fn` for `self.clone()`
    pub fn state(&self) -> CompactFlowerState<S, R, E> {
        self.state.canceled.store(false, Ordering::Relaxed);
        CompactFlowerState {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }

    /// Cancel `CompactFlower`.
    ///
    /// will do nothing if not explicitly configured on the `CompactHandle`.
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::Relaxed);
    }

    /// Check if the `CompactFlower` is canceled
    pub fn is_canceled(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Check if the current `CompactFlower` is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }

    /// Check if `Result` value of the `CompactFlower` is ready
    pub fn result_is_ready(&self) -> bool {
        self.state.result_ready.load(Ordering::Relaxed)
    }

    /// Check if channel value of the `CompactFlower` is present
    pub fn channel_is_present(&self) -> bool {
        self.state.channel_present.load(Ordering::Relaxed)
    }

    /// Try get the `Result` of the `CompactFlower` and ignore channel value (if any).
    ///
    /// Note: (this fn will be called if only `Result` is available)
    ///
    /// **Warning!** don't use this fn if channel value is important, use `extract fn` and then use `finalize fn` instead.
    pub fn try_result(&self, f: impl FnOnce(Result<R, Compact<E>>)) {
        if self.state.channel_present.load(Ordering::Relaxed) {
            self.state.cvar.notify_all();
            self.state.channel_present.store(false, Ordering::Relaxed)
        }
        if self.state.result_ready.load(Ordering::Relaxed) {
            let result = move || {
                let result = self.state.mtx.lock().unwrap().take();
                self.state.result_ready.store(false, Ordering::Relaxed);
                self.state.activated.store(false, Ordering::Relaxed);
                result
            };
            let result = result();
            if let CompactTypeOpt::Success(value) = result {
                f(Ok(value))
            } else if let CompactTypeOpt::Error(err_type) = result {
                f(Err(err_type))
            }
        }
    }

    /// Try extract channel value of the `CompactFlower` (this fn will be called if only channel value is available),
    ///
    /// and then `finalize` (must_use)
    pub fn extract(&self, f: impl FnOnce(S)) -> CompactFinalizer<'_, S, R, E> {
        if self.state.channel_present.load(Ordering::Relaxed) {
            let channel = move || {
                let channel = self.state.mtx.lock().unwrap().take();
                self.state.channel_present.store(false, Ordering::Relaxed);
                if self.async_suspender.1.load(Ordering::Relaxed) {
                    let mut mg_opt_waker = self.async_suspender.0.lock().unwrap();
                    self.async_suspender.1.store(false, Ordering::Relaxed);
                    if let Some(waker) = mg_opt_waker.take() {
                        waker.wake();
                    }
                } else {
                    self.state.cvar.notify_all();
                }
                channel
            };

            if let CompactTypeOpt::Channel(value) = channel() {
                f(value)
            }
        }

        CompactFinalizer::Try(self)
    }

    /// Poll channel value of the `CompactFlower`, and then `finalize` (must_use)
    pub fn poll(&self, f: impl FnOnce(Option<S>)) -> CompactFinalizer<'_, S, R, E> {
        if self.state.channel_present.load(Ordering::Relaxed) {
            let channel = move || {
                let channel = self.state.mtx.lock().unwrap().take();
                self.state.channel_present.store(false, Ordering::Relaxed);
                if self.async_suspender.1.load(Ordering::Relaxed) {
                    let mut mg_opt_waker = self.async_suspender.0.lock().unwrap();
                    self.async_suspender.1.store(false, Ordering::Relaxed);
                    if let Some(waker) = mg_opt_waker.take() {
                        waker.wake();
                    }
                } else {
                    self.state.cvar.notify_all();
                }
                if let CompactTypeOpt::Channel(value) = channel {
                    Some(value)
                } else {
                    None
                }
            };
            let channel = channel();
            f(channel)
        } else {
            f(None)
        }

        CompactFinalizer::Try(self)
    }
}

impl<S, R, E> Debug for CompactFlower<S, R, E>
where
    S: Send + Debug,
    R: Send + Debug,
    E: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompactFlower")
            .field("state", &self.state)
            .field("async_suspender", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

impl<S, R, E> Drop for CompactFlower<S, R, E>
where
    S: Send,
    R: Send,
    E: Send,
{
    fn drop(&mut self) {}
}
