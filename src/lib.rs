#![deny(unsafe_code)]
use core::{
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use std::{
    error::Error,
    mem,
    sync::{Condvar, Mutex},
};
use std::{sync::Arc, thread};

enum TypeOpt<S, R>
where
    S: Send,
    R: Send,
{
    Channel(S),
    Success(R),
    Error(String),
    None,
}

impl<S, R> Default for TypeOpt<S, R>
where
    S: Send,
    R: Send,
{
    fn default() -> Self {
        Self::None
    }
}

impl<S, R> Debug for TypeOpt<S, R>
where
    S: Send + Debug,
    R: Send + Debug,
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

impl<S, R> TypeOpt<S, R>
where
    S: Send,
    R: Send,
{
    fn take(&mut self) -> Self {
        mem::take(self)
    }
}

struct InnerState<S, R>
where
    S: Send,
    R: Send,
{
    activated: AtomicBool,
    result_ready: AtomicBool,
    channel_present: AtomicBool,
    mtx: Mutex<TypeOpt<S, R>>,
    cvar: Condvar,
    canceled: AtomicBool,
}

impl<S, R> Debug for InnerState<S, R>
where
    S: Debug + Send,
    R: Debug + Send,
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

impl<S, R> Drop for InnerState<S, R>
where
    S: Send,
    R: Send,
{
    fn drop(&mut self) {}
}

/// State of the flower
pub struct FlowerState<S, R>
where
    S: Send,
    R: Send,
{
    state: Arc<InnerState<S, R>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R> Debug for FlowerState<S, R>
where
    S: Send + Debug,
    R: Send + Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowerState")
            .field("state", &self.state)
            .field("async_suspender", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

impl<S, R> FlowerState<S, R>
where
    S: Send,
    R: Send,
{
    /// Get ID of the flower.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Cancel flower.
    ///
    /// will do nothing if not explicitly configured on the `Handle`.
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::Relaxed);
    }

    /// Check if the flower is canceled
    pub fn is_canceled(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Check if the current flower is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }
}

impl<S, R> Clone for FlowerState<S, R>
where
    S: Send,
    R: Send,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }
}

impl<S, R> Drop for FlowerState<S, R>
where
    S: Send,
    R: Send,
{
    fn drop(&mut self) {}
}

struct AsyncSuspender {
    inner: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
}

impl Future for AsyncSuspender {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut mtx = self.inner.0.lock().unwrap();
        if !self.inner.1.load(Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            *mtx = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// A handle for the Flower
pub struct Handle<S, R>
where
    S: Send,
    R: Send,
{
    state: Arc<InnerState<S, R>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R> Handle<S, R>
where
    S: Send,
    R: Send,
{
    /// Get ID of the flower.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Activate current flower
    pub fn activate(&self) {
        self.state.activated.store(true, Ordering::Relaxed);
    }

    /// Check if the current flower is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }

    /// Check if the current flower should be canceled
    pub fn should_cancel(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Send current progress value
    pub fn send(&self, s: S) {
        let mut mtx = self.state.mtx.lock().unwrap();
        {
            *mtx = TypeOpt::Channel(s);
            self.state.channel_present.store(true, Ordering::Relaxed);
            self.async_suspender.1.store(false, Ordering::Relaxed);
        }
        let _ = self.state.cvar.wait(mtx);
    }

    /// Send current progress value asynchronously.
    pub async fn send_async(&self, s: S) {
        {
            *self.state.mtx.lock().unwrap() = TypeOpt::Channel(s);
            self.async_suspender.1.store(true, Ordering::Relaxed);
            self.state.channel_present.store(true, Ordering::Relaxed);
        }
        AsyncSuspender {
            inner: self.async_suspender.clone(),
        }
        .await
    }

    /// Set result value
    pub fn set_result(&self, r: Result<R, Box<dyn Error>>) {
        match r {
            Ok(val) => self.success(val),
            Err(e) => self.error(e),
        }
    }

    /// Set the Ok value of the result.
    pub fn success(&self, r: R) {
        *self.state.mtx.lock().unwrap() = TypeOpt::Success(r);
        self.state.result_ready.store(true, Ordering::Relaxed);
    }

    /// Set the Err value of the result.
    pub fn error(&self, e: impl ToString) {
        *self.state.mtx.lock().unwrap() = TypeOpt::Error(e.to_string());
        self.state.result_ready.store(true, Ordering::Relaxed);
    }
}

impl<S, R> Clone for Handle<S, R>
where
    S: Send,
    R: Send,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }
}

impl<S, R> Drop for Handle<S, R>
where
    S: Send,
    R: Send,
{
    fn drop(&mut self) {
        if thread::panicking() && !self.state.result_ready.load(Ordering::Relaxed) {
            self.error(format!(
                "the flower handle with id: {} error, the thread panicked maybe?",
                self.id
            ));
        }
    }
}

impl<S, R> Debug for Handle<S, R>
where
    S: Debug + Send,
    R: Debug + Send,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("state", &self.state)
            .field("awaiting", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

pub struct Extracted<'a, S: Send, R: Send>(&'a Flower<S, R>);

impl<S, R> Extracted<'_, S, R>
where
    S: Send,
    R: Send,
{
    /// Try finalize result of the flower.
    pub fn finalize(self, f: impl FnOnce(Result<R, String>)) {
        let flower = self.0;
        if flower.state.result_ready.load(Ordering::Relaxed) {
            let result = move || {
                let result = flower.state.mtx.lock().unwrap().take();
                flower.state.result_ready.store(false, Ordering::Relaxed);
                flower.state.activated.store(false, Ordering::Relaxed);
                result
            };
            let result = result();
            if let TypeOpt::Success(value) = result {
                f(Ok(value))
            } else if let TypeOpt::Error(value) = result {
                f(Err(value))
            }
        }
    }
}

/// Flow loosely and gracefully.
///
/// Where:
///
/// `S` = type of sender (`channel`) value
///
/// `R` = type of `Ok` value of the `Result` (`Result<R, String>`, and Err value always return `String`)
///
/// # Quick Example:
///
///```
/// use flowync::{Flower, IOError};
/// type TestFlower = Flower<u32, String>;
///
/// fn fetch_things(id: usize) -> Result<String, IOError> {
///     let result =
///         Ok::<String, IOError>(format!("the flower with id: {} successfully completed fetching.", id));
///     let success = result?;
///     Ok(success)
/// }
///
/// fn main() {
///     let flower: TestFlower = Flower::new(1);
///     std::thread::spawn({
///         let handle = flower.handle();
///         // Activate
///         handle.activate();
///         move || {
///             for i in 0..10 {
///                 // Send current value through channel, will block the spawned thread
///                 // until the option value successfully being polled in the main thread.
///                 handle.send(i);
///                 // or handle.send_async(i).await; can be used from any multithreaded async runtime,
///             }
///             let result = fetch_things(handle.id());
///             // Set result and then extract later.
///             handle.set_result(result)
///         }
///     });
///
///     let mut exit = false;
///
///     loop {
///         // Check if the flower is_active()
///         // and will deactivate itself if the result value successfully received.
///         if flower.is_active() {
///             // another logic goes here...
///             // e.g:
///             // notify_loading_fn();
///
///             flower
///                 .extract(|channel| {
///                     // Poll channel
///                     if let Some(value) = channel {
///                         println!("{}", value);
///                     }
///                 })
///                 .finalize(|result| {
///                     match result {
///                         Ok(value) => println!("{}", value),
///                         Err(err_msg) => println!("{}", err_msg),
///                     }
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
pub struct Flower<S, R>
where
    S: Send,
    R: Send,
{
    state: Arc<InnerState<S, R>>,
    async_suspender: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<S, R> Flower<S, R>
where
    S: Send,
    R: Send,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(InnerState {
                activated: AtomicBool::new(false),
                result_ready: AtomicBool::new(false),
                channel_present: AtomicBool::new(false),
                mtx: Mutex::new(TypeOpt::None),
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
    pub fn handle(&self) -> Handle<S, R> {
        self.state.canceled.store(false, Ordering::Relaxed);
        Handle {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }

    /// Get the state
    ///
    /// Since `Flower` itself is uncloneable to avoid data races, this is an alternative `fn` for `self.clone()`
    pub fn state(&self) -> FlowerState<S, R> {
        self.state.canceled.store(false, Ordering::Relaxed);
        FlowerState {
            state: Clone::clone(&self.state),
            async_suspender: Clone::clone(&self.async_suspender),
            id: self.id,
        }
    }

    /// Cancel flower.
    ///
    /// will do nothing if not explicitly configured on the `Handle`.
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::Relaxed);
    }

    /// Check if the flower is canceled
    pub fn is_canceled(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Check if the current flower is active
    pub fn is_active(&self) -> bool {
        self.state.activated.load(Ordering::Relaxed)
    }

    /// Check if result value of the flower is ready
    pub fn result_is_ready(&self) -> bool {
        self.state.result_ready.load(Ordering::Relaxed)
    }

    /// Check if channel value of the flower is present
    pub fn channel_is_present(&self) -> bool {
        self.state.channel_present.load(Ordering::Relaxed)
    }

    #[deprecated = "result is highly deprecated, use `try_result` fn is instead"]
    pub fn result(&self, f: impl FnOnce(Result<R, String>)) {
        self.try_result(f);
    }

    /// Try get the result of the flower and ignore channel value (if any).
    ///
    /// **Warning!** don't use this fn if channel value is important, use `extract fn` and then use `finalize fn` instead.
    pub fn try_result(&self, f: impl FnOnce(Result<R, String>)) {
        if self.state.channel_present.load(Ordering::Relaxed) {
            self.state.cvar.notify_all();
        }
        if self.state.result_ready.load(Ordering::Relaxed) {
            let _self = self;
            let result = move || {
                let result = _self.state.mtx.lock().unwrap().take();
                _self.state.result_ready.store(false, Ordering::Relaxed);
                _self.state.activated.store(false, Ordering::Relaxed);
                result
            };
            let result = result();
            if let TypeOpt::Success(value) = result {
                f(Ok(value))
            } else if let TypeOpt::Error(value) = result {
                f(Err(value))
            }
        }
    }

    /// Try extract channel value of the flower if available, and then `finalize` (must_use)
    pub fn extract(&self, f: impl FnOnce(Option<S>)) -> Extracted<'_, S, R> {
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
                if let TypeOpt::Channel(value) = channel {
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

        Extracted(self)
    }
}

impl<S, R> Debug for Flower<S, R>
where
    S: Debug + Send,
    R: Debug + Send,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flower")
            .field("state", &self.state)
            .field("async_suspender", &self.async_suspender)
            .field("id", &self.id)
            .finish()
    }
}

impl<S, R> Drop for Flower<S, R>
where
    S: Send,
    R: Send,
{
    fn drop(&mut self) {
        if thread::panicking() {
            self.state.activated.store(false, Ordering::Relaxed)
        }
    }
}

pub type IOError = Box<dyn Error>;
/// Alternative alias to avoid conflict with other crate type
pub type OIError = IOError;

/// A converter to convert `Option<T>` into `Result<T, E>` using `catch` fn.
pub trait IntoResult<T> {
    /// Convert `Option<T>` into `Result<T, E>`
    fn catch(self, error_msg: impl ToString) -> Result<T, Box<dyn Error>>;
}

impl<T> IntoResult<T> for Option<T> {
    fn catch(self, error_msg: impl ToString) -> Result<T, Box<dyn Error>> {
        let message: String = error_msg.to_string();
        match self {
            Some(val) => Ok(val),
            None => Err(message.into()),
        }
    }
}
