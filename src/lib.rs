#![deny(unsafe_code)]
use core::{
    clone::Clone,
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use std::sync::{Condvar, Mutex};
use std::{sync::Arc, thread};

struct State<S, R>
where
    S: Send,
    R: Send,
{
    activated: AtomicBool,
    result_ready: AtomicBool,
    channel_present: AtomicBool,
    mtx: Mutex<(Option<S>, Option<R>, Option<String>)>,
    cvar: Condvar,
    canceled: AtomicBool,
}

impl<S, R> Debug for State<S, R>
where
    S: Debug + Send,
    R: Debug + Send,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("result_ready", &self.result_ready)
            .field("channel_present", &self.channel_present)
            .field("mtx", &self.mtx)
            .field("cvar", &self.cvar)
            .field("canceled", &self.canceled)
            .field("activated", &self.activated)
            .finish()
    }
}

impl<S, R> Drop for State<S, R>
where
    S: Send,
    R: Send,
{
    fn drop(&mut self) {}
}

/// Flow loosely and gracefully.
///
/// Where:
///
/// S = type of sender (channel) value
///
/// R = type of Ok value of the Result (Result<'R', String>, and Err value always return String)
///
/// # Quick Example:
///
///```
///use flowync::Flower;
///type TestFlower = Flower<u32, String>;
///
///fn _main() {
///    let flower: TestFlower = Flower::new(1);
///    std::thread::spawn({
///        let handle = flower.handle();
///        // Activate
///        handle.activate();
///        move || {
///            for i in 0..10 {
///                // Send current value through channel, will block the spawned thread
///                // until the option value successfully being polled in the main thread.
///                handle.send(i);
///                // or handle.send_async(i).await; can be used from any multithreaded async runtime,
///                
///                // Return error if the job is failure, for example:
///                // if i >= 3 {
///                //    return handle.error("Err");
///                // }
///            }
///            // And return if the job successfully completed.
///            return handle.success("Ok".to_string());
///        }
///    });
///
///    let mut exit = false;
///
///    loop {
///        // Check if the flower is_active()
///        // and will deactivate itself if the result value successfully received.
///        if flower.is_active() {
///            // Another logic goes here...
///            // e.g:
///            // notify_loading_fn();
///
///            flower
///                .extract(|channel| {
///                     // Poll channel
///                     if let Some(value) = channel {
///                         println!("{}", value);
///                     }
///                 })
///                .finalize(|result| {
///                    match result {
///                        Ok(value) => println!("{}", value),
///                        Err(err_msg) => println!("{}", err_msg),
///                    }
///
///                    // Exit if completed
///                    exit = true;
///                });
///        }
///
///        if exit {
///            break;
///        }
///    }
///}
/// ```
pub struct Flower<S, R>
where
    S: Send,
    R: Send,
{
    state: Arc<State<S, R>>,
    awaiting: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

pub struct Extracted<'a, S: Send, R: Send>(&'a Flower<S, R>);

impl<S, R> Extracted<'_, S, R>
where
    S: Send,
    R: Send,
{
    /// Try finalize result of the flower.
    pub fn finalize(self, f: impl FnOnce(Result<R, String>)) {
        let _self = self.0;
        if _self.state.result_ready.load(Ordering::Relaxed) {
            let mut result_value = _self.state.mtx.lock().unwrap();
            let (_, ok, error) = &mut (*result_value);
            _self.state.result_ready.store(false, Ordering::Relaxed);
            _self.state.activated.store(false, Ordering::Relaxed);

            if let Some(value) = ok.take() {
                f(Ok(value));
            } else if let Some(value) = error.take() {
                f(Err(value));
            }
        }
    }
}

impl<S, R> Flower<S, R>
where
    S: Send,
    R: Send,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(State {
                activated: AtomicBool::new(false),
                result_ready: AtomicBool::new(false),
                channel_present: AtomicBool::new(false),
                mtx: Mutex::new((None, None, None)),
                cvar: Condvar::new(),
                canceled: AtomicBool::new(false),
            }),
            awaiting: Arc::new((Mutex::new(None), AtomicBool::new(false))),
            id,
        }
    }

    /// Get ID of the flower.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get handle of the flower.
    pub fn handle(&self) -> Handle<S, R> {
        self.state.canceled.store(false, Ordering::Relaxed);
        Handle {
            state: Clone::clone(&self.state),
            awaiting: Clone::clone(&self.awaiting),
            id: self.id,
        }
    }

    /// Cancel current flower handle.
    ///
    /// will do nothing if not explicitly configured.
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

    /// Get the result of the flower and ignore channel value (if any).
    ///
    /// **Warning!** don't use this fn if channel value is important, use `extract fn` and then use `finalize fn` instead.
    pub fn result(&self, f: impl FnOnce(Result<R, String>)) {
        if self.state.channel_present.load(Ordering::Relaxed) {
            let _ = self.state.mtx.lock().unwrap().0.take();
            self.state.cvar.notify_all();
        }

        if self.state.result_ready.load(Ordering::Relaxed) {
            let mut result_value = self.state.mtx.lock().unwrap();
            let (_, ok, error) = &mut *result_value;
            self.state.result_ready.store(false, Ordering::Relaxed);
            self.state.activated.store(false, Ordering::Relaxed);
            if let Some(value) = ok.take() {
                f(Ok(value));
            } else if let Some(value) = error.take() {
                f(Err(value));
            }
        }
    }

    /// Try extract channel value of the flower if available, and then `finalize` (must_use)
    pub fn extract(&self, f: impl FnOnce(Option<S>)) -> Extracted<'_, S, R> {
        if self.state.channel_present.load(Ordering::Relaxed) {
            let value = self.state.mtx.lock().unwrap().0.take();
            self.state.channel_present.store(false, Ordering::Relaxed);
            if self.awaiting.1.load(Ordering::Relaxed) {
                let mut mg_opt_waker = self.awaiting.0.lock().unwrap();
                self.awaiting.1.store(false, Ordering::Relaxed);
                if let Some(waker) = mg_opt_waker.take() {
                    waker.wake();
                }
            } else {
                self.state.cvar.notify_all();
            }
            f(value)
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
            .field("awaiting", &self.awaiting)
            .field("id", &self.id)
            .finish()
    }
}

impl<S, R> Clone for Flower<S, R>
where
    S: Send,
    R: Send,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            awaiting: Clone::clone(&self.awaiting),
            id: self.id,
        }
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

/// A handle for the Flower
pub struct Handle<S, R>
where
    S: Send,
    R: Send,
{
    state: Arc<State<S, R>>,
    awaiting: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
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
        mtx.0 = Some(s);
        self.state.channel_present.store(true, Ordering::Relaxed);
        self.awaiting.1.store(false, Ordering::Relaxed);
        let _e = self.state.cvar.wait(mtx);
    }

    /// Send current progress value asynchronously.
    pub async fn send_async(&self, s: S) {
        self.state.mtx.lock().unwrap().0 = Some(s);
        self.awaiting.1.store(true, Ordering::Relaxed);
        self.state.channel_present.store(true, Ordering::Relaxed);
        AsyncSuspender {
            awaiting: self.awaiting.clone(),
        }
        .await
    }

    /// Set the Ok value of the result.
    pub fn success(&self, r: R) {
        let mut result = self.state.mtx.lock().unwrap();
        let (_, ok, error) = &mut *result;
        *ok = Some(r);
        *error = None;
        self.state.result_ready.store(true, Ordering::Relaxed);
    }

    /// Set the Err value of the result.
    pub fn error(&self, e: impl ToString) {
        let mut result = self.state.mtx.lock().unwrap();
        let (_, ok, error) = &mut *result;
        *error = Some(e.to_string());
        *ok = None;
        self.state.result_ready.store(true, Ordering::Relaxed);
    }
}

struct AsyncSuspender {
    awaiting: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
}

impl Future for AsyncSuspender {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut mtx = self.awaiting.0.lock().unwrap();
        if !self.awaiting.1.load(Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            *mtx = Some(cx.waker().clone());
            Poll::Pending
        }
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
            awaiting: Clone::clone(&self.awaiting),
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
            .field("awaiting", &self.awaiting)
            .field("id", &self.id)
            .finish()
    }
}
