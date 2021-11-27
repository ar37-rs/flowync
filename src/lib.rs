#![deny(unsafe_code)]
use core::{
    clone::Clone,
    fmt::{self, Debug, Formatter},
    sync::atomic::{AtomicBool, Ordering},
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

/// Flow loosely and gracefully.
///
/// Where:
///
/// SOME: type of sender (channel) value
///
/// OK: type of Ok value (Result<'OK', String>, and Err value always return String)
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
///        // Starting from version 1.x.x instead of polling the mutex over and over,
///        // the fn will be activated automatically if the handle sending or return a value
///        // and will deactivate itself if the result value successfully received.
///        // Note: this fn is non-blocking (won't block the current thread).
///        flower.try_recv(
///            |value| {
///                println!("{}", value);
///            },
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
///            },
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>)>,
    flowing: Arc<(AtomicBool, Mutex<Option<SOME>>, Condvar, AtomicBool)>,
    id: usize,
}

impl<SOME, OK> Flower<SOME, OK>
where
    SOME: Clone + Send + Sync + 'static,
    OK: Clone + Send + Sync + 'static,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new((AtomicBool::new(false), Mutex::new((None, None)))),
            flowing: Arc::new((
                AtomicBool::new(false),
                Mutex::new(None),
                Condvar::new(),
                AtomicBool::new(false),
            )),
            id,
        }
    }

    /// Get ID of the flower.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get handle of the flower.
    pub fn handle(&self) -> FlowerHandle<SOME, OK> {
        FlowerHandle {
            state: Clone::clone(&self.state),
            flowing: Clone::clone(&self.flowing),
            id: self.id,
        }
    }

    /// Cancel current flower handle.
    ///
    /// will do noting if not explicitly configured.
    pub fn cancel(&self) {
        self.flowing.3.store(true, Ordering::Relaxed);
    }

    /// Try receive the contained values of the flower
    pub fn try_recv<Current: FnOnce(SOME) -> (), Done: FnOnce(Result<OK, String>) -> ()>(
        &self,
        c: Current,
        d: Done,
    ) {
        if self.flowing.0.load(Ordering::Relaxed) {
            if !self.state.0.load(Ordering::Relaxed) {
                let value = match self.flowing.1.lock() {
                    Ok(mut _result) => {
                        let result = _result.clone();
                        *_result = None;
                        self.flowing.2.notify_one();
                        if let Some(value) = result {
                            value
                        } else {
                            return;
                        }
                    }
                    _ => return,
                };
                c(value);
            } else {
                match self.state.1.lock() {
                    Ok(mut result) => {
                        let (ok, error) = &mut *result;
                        if let Some(val) = ok {
                            let val = val.clone();
                            *ok = None;
                            d(Ok(val));
                        } else if let Some(val) = error {
                            let val = val.clone();
                            *error = None;
                            d(Err(val));
                        }
                    }
                    _ => (),
                }
                self.flowing.3.store(false, Ordering::Relaxed);
                self.flowing.0.store(false, Ordering::Relaxed);
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
            .field("flowing", &self.flowing)
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
            flowing: Clone::clone(&self.flowing),
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>)>,
    flowing: Arc<(AtomicBool, Mutex<Option<SOME>>, Condvar, AtomicBool)>,
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
        self.flowing.3.load(Ordering::Relaxed)
    }

    /// Send current progress value
    pub fn send(&self, _value: SOME) {
        let (_, mtx, cvar, _) = &*self.flowing;
        if let Ok(mut mtx) = mtx.lock() {
            *mtx = Some(_value);
            self.state.0.store(false, Ordering::Relaxed);
            self.flowing.0.store(true, Ordering::Relaxed);
            let _ = cvar.wait(mtx);
        }
    }

    /// Contains the success value for the result.
    pub fn ok(&self, _value: OK) {
        match self.state.1.lock() {
            Ok(mut result) => {
                let (ok, error) = &mut *result;
                *ok = Some(_value);
                *error = None;
                self.state.0.store(true, Ordering::Relaxed);
                self.flowing.0.store(true, Ordering::Relaxed);
            }
            _ => (),
        }
    }

    /// Contains the error value for the result.
    pub fn err(&self, _value: String) {
        match self.state.1.lock() {
            Ok(mut result) => {
                let (ok, error) = &mut *result;
                *error = Some(_value);
                *ok = None;
                self.state.0.store(true, Ordering::Relaxed);
                self.flowing.0.store(true, Ordering::Relaxed);
            }
            _ => (),
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
            flowing: Clone::clone(&self.flowing),
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
            if !self.state.0.load(Ordering::Relaxed) {
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
            .field("flowing", &self.flowing)
            .field("id", &self.id)
            .finish()
    }
}

/// (Flower without channel)
///
/// spring in a long way, jump to a great height, or with great force.
///
/// Where:
///
/// OK: type of Ok value (Result<'OK', String>, and Err value always return String)
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>, AtomicBool)>,
    id: usize,
}

impl<OK> Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new((
                AtomicBool::new(false),
                Mutex::new((None, None)),
                AtomicBool::new(false),
            )),
            id,
        }
    }

    /// Get ID of the leaper
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get handle of the leaper.
    pub fn handle(&self) -> LeaperHandle<OK> {
        LeaperHandle {
            state: Clone::clone(&self.state),
            id: self.id,
        }
    }

    /// Cancel current leaper handle.
    ///
    /// will do noting if not explicitly configured.
    pub fn cancel(&self) {
        self.state.2.store(true, Ordering::Relaxed);
    }

    /// Try catch the value (result) of the leaper
    pub fn try_catch<Done: FnOnce(Result<OK, String>) -> ()>(&self, d: Done) {
        if self.state.0.load(Ordering::Relaxed) {
            match self.state.1.lock() {
                Ok(mut result) => {
                    let (ok, error) = &mut *result;
                    if let Some(val) = ok {
                        let val = val.clone();
                        *ok = None;
                        d(Ok(val));
                    } else if let Some(val) = error {
                        let val = val.clone();
                        *error = None;
                        d(Err(val));
                    }
                }
                _ => (),
            }
            self.state.0.store(false, Ordering::Relaxed);
            self.state.2.store(false, Ordering::Relaxed);
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>, AtomicBool)>,
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
        self.state.2.load(Ordering::Relaxed)
    }

    /// Contains the success value for the result.
    pub fn ok(&self, _value: OK) {
        match self.state.1.lock() {
            Ok(mut result) => {
                let (ok, error) = &mut *result;
                *ok = Some(_value);
                *error = None;
                self.state.0.store(true, Ordering::Relaxed);
            }
            _ => (),
        }
    }

    /// Contains the error value for the result.
    pub fn err(&self, _value: String) {
        match self.state.1.lock() {
            Ok(mut result) => {
                let (ok, error) = &mut *result;
                *error = Some(_value);
                *ok = None;
                self.state.0.store(true, Ordering::Relaxed);
            }
            _ => (),
        }
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
            if !self.state.0.load(Ordering::Relaxed) {
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
