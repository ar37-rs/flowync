#![deny(unsafe_code)]
use core::{
    clone::Clone,
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll, Waker},
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

struct FlowerState<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    result_ready: AtomicBool,
    channel_present: AtomicBool,
    mtx: Mutex<(Option<SOME>, Option<OK>, Option<String>)>,
    cvar: Condvar,
    canceled: AtomicBool,
}

impl<SOME, OK> Debug for FlowerState<SOME, OK>
where
    SOME: Debug + Clone + Send + Sync + 'static,
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowerState")
            .field("result_ready", &self.result_ready)
            .field("channel_present", &self.channel_present)
            .field("mtx", &self.mtx)
            .field("cvar", &self.cvar)
            .field("canceled", &self.canceled)
            .finish()
    }
}

impl<SOME, OK> Drop for FlowerState<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {}
}

/// Flow loosely and gracefully.
///
/// Where:
///
/// SOME: type of sender (channel) value
///
/// OK: type of Ok value of the Result (Result<'OK', String>, and Err value always return String)
///
/// # Quick Example:
///
///```
///use flowync::Flower;
///
///fn main() {
///    let flower = Flower::<i32, String>::new(1);
///    std::thread::spawn({
///        let handle = flower.handle();
///        move || {
///            for i in 0..10 {
///                // Send current value through channel, will block the spawned thread
///                // until the option value successfully being polled in the main thread.
///                handle.send(i);
///                // or handle.send_async(i).await; can be used from any multithreaded async runtime,
///                // it won't block the other async operations.  
///                
///                // // Return error if the job is failure, for example:
///                // if i >= 3 {
///                //    return handle.err("Err".to_string());
///                // }
///            }
///            // And return ok if the job successfully completed.
///            return handle.ok("Ok".to_string());
///        }
///    });
///
///    let mut exit = false;
///
///    loop {
///        // Starting from version 1.8.x instead of polling the mutex over and over,
///        // the fn will be activated automatically if the handle sending or return a value
///        // and will deactivate itself if the result value successfully received.
///        // Note: this fn is non-blocking (won't block the current thread).
///        flower.try_recv(
///            |value| println!("{}\n", value),
///            |result| {
///                match result {
///                    Ok(value) => {
///                        println!("{}", value);
///                    }
///                    Err(e) => {
///                        println!("{}", e);
///                    }
///                }
///                exit = true;
///            }
///        );
///
///        if exit {
///            break;
///        }
///    }
///}
/// ```
pub struct Flower<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    state: Arc<FlowerState<SOME, OK>>,
    awaiting: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<SOME, OK> Flower<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(FlowerState {
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
    pub fn handle(&self) -> FlowerHandle<SOME, OK> {
        self.state.canceled.store(false, Ordering::Relaxed);
        FlowerHandle {
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

    /// Check if result value of the flower is ready
    pub fn result_is_ready(&self) -> bool {
        self.state.result_ready.load(Ordering::Relaxed)
    }

    /// Check if channel value of the flower is present
    pub fn channel_is_present(&self) -> bool {
        self.state.channel_present.load(Ordering::Relaxed)
    }

    /// Try receive the flower values
    pub fn try_recv<
        ChannelValue: FnOnce(SOME) -> (),
        ResultValue: FnOnce(Result<OK, String>) -> (),
    >(
        &self,
        cv: ChannelValue,
        rv: ResultValue,
    ) {
        if self.state.channel_present.load(Ordering::Relaxed) {
            let result = self.state.mtx.lock().unwrap().0.take();
            self.state.channel_present.store(false, Ordering::Relaxed);
            if self.awaiting.1.load(Ordering::Relaxed) {
                let mut mg_opt_waker = self.awaiting.0.lock().unwrap();
                self.awaiting.1.store(false, Ordering::Relaxed);
                match mg_opt_waker.take() {
                    Some(waker) => waker.wake(),
                    _ => (),
                }
            } else {
                self.state.cvar.notify_one();
            }

            if let Some(value) = result {
                cv(value);
            }
        } else if self.state.result_ready.load(Ordering::Relaxed) {
            let mut result = self.state.mtx.lock().unwrap();
            let (_, ok, error) = &mut *result;
            self.state.result_ready.store(false, Ordering::Relaxed);
            if let Some(value) = ok.take() {
                rv(Ok(value));
            } else if let Some(value) = error.take() {
                rv(Err(value));
            }
        }
    }
}

impl<SOME, OK> Debug for Flower<SOME, OK>
where
    SOME: Debug + Clone + Send + Sync + 'static,
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flower")
            .field("state", &self.state)
            .field("awaiting", &self.awaiting)
            .field("id", &self.id)
            .finish()
    }
}

impl<SOME, OK> Clone for Flower<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            awaiting: Clone::clone(&self.awaiting),
            id: self.id,
        }
    }
}

impl<SOME, OK> Drop for Flower<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {}
}

/// A handle for the Flower
pub struct FlowerHandle<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    state: Arc<FlowerState<SOME, OK>>,
    awaiting: Arc<(Mutex<Option<Waker>>, AtomicBool)>,
    id: usize,
}

impl<SOME, OK> FlowerHandle<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    /// Get ID of the flower.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Check if the current flower should be canceled
    pub fn should_cancel(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Send current progress value
    pub fn send(&self, _value: SOME) {
        let mut mtx = self.state.mtx.lock().unwrap();
        mtx.0 = Some(_value);
        self.state.channel_present.store(true, Ordering::Relaxed);
        self.awaiting.1.store(false, Ordering::Relaxed);
        let _ = self.state.cvar.wait(mtx).unwrap();
    }

    /// Send current progress value asynchronously.
    pub async fn send_async(&self, _value: SOME) {
        self.state.mtx.lock().unwrap().0 = Some(_value);
        self.awaiting.1.store(true, Ordering::Relaxed);
        self.state.channel_present.store(true, Ordering::Relaxed);
        AsyncSuspender {
            awaiting: self.awaiting.clone(),
        }
        .await
    }

    /// Contains the success value for the result.
    pub fn ok(&self, _value: OK) {
        let mut result = self.state.mtx.lock().unwrap();
        let (_, ok, error) = &mut *result;
        *ok = Some(_value);
        *error = None;
        self.state.result_ready.store(true, Ordering::Relaxed);
    }

    /// Contains the error value for the result.
    pub fn err(&self, _value: String) {
        let mut result = self.state.mtx.lock().unwrap();
        let (_, ok, error) = &mut *result;
        *error = Some(_value);
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

impl<SOME, OK> Clone for FlowerHandle<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            awaiting: Clone::clone(&self.awaiting),
            id: self.id,
        }
    }
}

impl<SOME, OK> Drop for FlowerHandle<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {
        if thread::panicking() {
            if !self.state.result_ready.load(Ordering::Relaxed) {
                self.err(format!(
                    "the flower handle with id: {} error, the thread panicked maybe?",
                    self.id
                ));
            }
        }
    }
}

impl<SOME, OK> Debug for FlowerHandle<SOME, OK>
where
    SOME: Debug + Clone + Send + Sync + 'static,
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlowerHandle")
            .field("state", &self.state)
            .field("awaiting", &self.awaiting)
            .field("id", &self.id)
            .finish()
    }
}

struct LeaperState<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    result_ready: AtomicBool,
    mtx: Mutex<(Option<OK>, Option<String>)>,
    canceled: AtomicBool,
}

impl<OK> Debug for LeaperState<OK>
where
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeaperState")
            .field("result_ready", &self.result_ready)
            .field("mtx", &self.mtx)
            .field("canceled", &self.canceled)
            .finish()
    }
}

impl<OK> Drop for LeaperState<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {}
}

/// (Flower without channel)
///
/// spring in a long way, jump to a great height, or with great force.
///
/// Where:
///
/// OK: type of Ok value of the Result (Result<'OK', String>, and Err value always return String)
///
/// # Quick Example:
///
///```
///use flowync::Leaper;
///
///fn main() {
///    let leaper = Leaper::<String>::new(1);
///    std::thread::spawn({
///        let handle = leaper.handle();
///        move || {
///            // // Return error if the job is failure, for example:
///            // return handle.err("Err".to_string());
///
///            // And return ok if the job successfully completed.
///            return handle.ok("Ok".to_string());
///        }
///    });
///
///    let mut exit = false;
///
///    loop {
///        // Starting from version 0.2.x instead of polling the mutex over and over,
///        // the fn will be activated automatically if the handle return a value
///        // and will deactivate itself if the result value successfully catched.
///        // Note: this fn is non-blocking (won't block the current thread).
///        leaper.try_catch(|result| {
///            match result {
///                Ok(value) => {
///                    println!("{}", value);
///                }
///                Err(e) => {
///                    println!("{}", e);
///                }
///            }
///            exit = true;
///        });
///
///        if exit {
///            break;
///        }
///    }
///}
/// ```
pub struct Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    state: Arc<LeaperState<OK>>,
    id: usize,
}

impl<OK> Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new(LeaperState {
                result_ready: AtomicBool::new(false),
                mtx: Mutex::new((None, None)),
                canceled: AtomicBool::new(false),
            }),
            id,
        }
    }

    /// Get ID of the leaper
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get handle of the leaper.
    pub fn handle(&self) -> LeaperHandle<OK> {
        self.state.canceled.store(false, Ordering::Relaxed);
        LeaperHandle {
            state: Clone::clone(&self.state),
            id: self.id,
        }
    }

    /// Cancel current leaper handle.
    ///
    /// will do noting if not explicitly configured.
    pub fn cancel(&self) {
        self.state.canceled.store(true, Ordering::Relaxed);
    }

    /// Check if the leaper is canceled
    pub fn is_canceled(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Check if the result is ready to catch.
    pub fn result_is_ready(&self) -> bool {
        self.state.result_ready.load(Ordering::Relaxed)
    }

    /// Try catch the value (result) of the leaper
    pub fn try_catch<ResultValue: FnOnce(Result<OK, String>) -> ()>(&self, rv: ResultValue) {
        if self.state.result_ready.load(Ordering::Relaxed) {
            let mut result = self.state.mtx.lock().unwrap();
            let (ok, error) = &mut *result;
            self.state.result_ready.store(false, Ordering::Relaxed);
            if let Some(value) = ok.take() {
                rv(Ok(value));
            } else if let Some(value) = error.take() {
                rv(Err(value));
            }
        }
    }
}

impl<OK> Debug for Leaper<OK>
where
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Leaper")
            .field("state", &self.state)
            .field("id", &self.id)
            .finish()
    }
}

impl<OK> Clone for Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            id: self.id,
        }
    }
}

impl<OK> Drop for Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {}
}

/// A handle for the Leaper
pub struct LeaperHandle<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    state: Arc<LeaperState<OK>>,
    id: usize,
}

impl<OK> LeaperHandle<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    /// Get ID of the leaper
    pub fn id(&self) -> usize {
        self.id
    }

    /// Check if the current leaper should be canceled
    pub fn should_cancel(&self) -> bool {
        self.state.canceled.load(Ordering::Relaxed)
    }

    /// Contains the success value for the result.
    pub fn ok(&self, _value: OK) {
        let mut result = self.state.mtx.lock().unwrap();
        let (ok, error) = &mut *result;
        *ok = Some(_value);
        *error = None;
        self.state.result_ready.store(true, Ordering::Relaxed);
    }

    /// Contains the error value for the result.
    pub fn err(&self, _value: String) {
        let mut result = self.state.mtx.lock().unwrap();
        let (ok, error) = &mut *result;
        *error = Some(_value);
        *ok = None;
        self.state.result_ready.store(true, Ordering::Relaxed);
    }
}

impl<OK> Clone for LeaperHandle<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            state: Clone::clone(&self.state),
            id: self.id,
        }
    }
}

impl<OK> Drop for LeaperHandle<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    fn drop(&mut self) {
        if thread::panicking() {
            if !self.state.result_ready.load(Ordering::Relaxed) {
                self.err(format!(
                    "the leaper handle with id: {} error, the thread panicked maybe?",
                    self.id
                ));
            }
        }
    }
}

impl<OK> Debug for LeaperHandle<OK>
where
    OK: Debug + Clone + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeaperHandle")
            .field("state", &self.state)
            .field("id", &self.id)
            .finish()
    }
}
