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
/// SOME: type of Some value (Option<'SOME'>)
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
///    let handle = flower.handle();
///    std::thread::spawn(move || {
///        handle.start_flowing();
///        for i in 0..10 {
///            handle.send_current(i);
///            // // Return error.
///            // if i >= 3 {
///            //    return handle.err("Err".into());
///            // }
///        }
///        handle.ok("Ok".into());
///    });
///
///    let mut exit = false;
///
///    loop {
///        flower.try_recv(
///            |option| {
///                if let Some(value) = option {
///                    println!("{}", value);
///                }
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
    flowing: Arc<(AtomicBool, Mutex<Option<SOME>>, Condvar)>,
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
            flowing: Arc::new((AtomicBool::new(false), Mutex::new(None), Condvar::new())),
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

    /// Check if the flower is flowing
    pub fn is_flowing(&self) -> bool {
        self.flowing.0.load(Ordering::Relaxed)
    }

    /// Try receive the contained values of the flower
    pub fn try_recv<Current: FnOnce(Option<SOME>) -> (), Done: FnOnce(Result<OK, String>) -> ()>(
        &self,
        c: Current,
        d: Done,
    ) {
        if self.flowing.0.load(Ordering::Relaxed) {
            if !self.state.0.load(Ordering::Relaxed) {
                match self.flowing.1.lock() {
                    Ok(mut result) => {
                        let opt = result.clone();
                        if opt.is_some() {
                            *result = None;
                            self.flowing.2.notify_one();
                            c(opt);
                            return;
                        }
                    }
                    _ => (),
                }
                c(None);
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
                self.state.0.store(false, Ordering::Relaxed);
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
    flowing: Arc<(AtomicBool, Mutex<Option<SOME>>, Condvar)>,
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

    /// Call this fn every time at the very begining (runtime) to initialize,, otherwise the flower won't flow.
    pub fn start_flowing(&self) {
        self.flowing.0.store(true, Ordering::Relaxed);
    }

    /// Fn start_flowing() with async topping for multi-threaded async runtime use case
    pub async fn start_flowing_async(&self) {
        self.flowing.0.store(true, Ordering::Relaxed);
    }

    /// Pause the flower from flowing.
    pub fn pause(&self) {
        self.flowing.0.store(false, Ordering::Relaxed);
    }

    /// Send current progress value
    pub fn send_current(&self, _value: SOME) {
        let (_, mtx, cvar) = &*self.flowing;
        if let Ok(mut mtx) = mtx.lock() {
            *mtx = Some(_value);
            let _ = cvar.wait(mtx);
        }
    }

    /// Contains the success value
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

    /// Contains the error value
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
                    "Flower with id: {} error, the thread panicked maybe?",
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
///    let handle = leaper.handle();
///    std::thread::spawn(move || {
///        handle.start_leaping();
///        // // Return error.
///        // return handle.err("Err".into());
///        //
///        handle.ok("Ok".into());
///    });
///
///    let mut exit = false;
///
///    loop {
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>)>,
    id: usize,
}

impl<OK> Leaper<OK>
where
    OK: Clone + Send + Sync + 'static,
{
    pub fn new(id: usize) -> Self {
        Self {
            state: Arc::new((AtomicBool::new(false), Mutex::new((None, None)))),
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

    /// Check if the leaper is leaping
    pub fn is_leaping(&self) -> bool {
        self.state.0.load(Ordering::Relaxed)
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
    state: Arc<(AtomicBool, Mutex<(Option<OK>, Option<String>)>)>,
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

    /// Call this fn every time at the very begining (runtime) to initialize, otherwise the leaper won't leap.
    pub fn start_leaping(&self) {
        self.state.0.store(true, Ordering::Relaxed);
    }

    /// Fn start_leaping() with async topping for multi-threaded async runtime use case
    pub async fn start_leaping_async(&self) {
        self.state.0.store(true, Ordering::Relaxed);
    }

    /// Pause the leaper from leaping
    pub fn pause(&self) {
        self.state.0.store(false, Ordering::Relaxed);
    }

    /// Contains the success value
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

    /// Contains the error value
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
                    "Leaper with id: {} error, the thread panicked maybe?",
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
