use std::{
    any::Any,
    collections::HashMap,
    fmt::Display,
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use thiserror::Error;

type ErasedSession = Arc<dyn Any + Send + Sync>;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("session manager is shut down")]
    ShutDown,

    #[error("session registry lock is poisoned")]
    RegistryLockPoisoned,

    #[error("session `{key}` lock is poisoned")]
    SessionLockPoisoned { key: String },

    #[error("session `{key}` exists with a different type")]
    TypeMismatch { key: String },

    #[error("failed to initialize session `{key}`: {message}")]
    Initialization { key: String, message: String },
}

pub struct SessionHandle<T> {
    key: String,
    session: Arc<Mutex<T>>,
}

impl<T> Clone for SessionHandle<T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            session: Arc::clone(&self.session),
        }
    }
}

impl<T> SessionHandle<T> {
    fn new(key: String, session: Arc<Mutex<T>>) -> Self {
        Self { key, session }
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, T>, SessionError> {
        self.session
            .lock()
            .map_err(|_| SessionError::SessionLockPoisoned {
                key: self.key.clone(),
            })
    }
}

#[derive(Default)]
struct SessionRegistry {
    sessions: HashMap<String, ErasedSession>,
    shut_down: bool,
}

#[derive(Default)]
pub struct SessionManager {
    registry: RwLock<SessionRegistry>,
}

impl SessionManager {
    pub fn get<T>(&self, key: &str) -> Result<Option<SessionHandle<T>>, SessionError>
    where
        T: Send + 'static,
    {
        let registry = self
            .registry
            .read()
            .map_err(|_| SessionError::RegistryLockPoisoned)?;

        if registry.shut_down {
            return Err(SessionError::ShutDown);
        }

        let session = registry.sessions.get(key).cloned();
        drop(registry);

        session
            .map(|session| downcast_session(key, session))
            .transpose()
    }

    pub fn get_or_try_init<T, F, E>(
        &self,
        key: impl Into<String>,
        initialize: F,
    ) -> Result<SessionHandle<T>, SessionError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, E>,
        E: Display,
    {
        let key = key.into();

        if let Some(session) = self.get(&key)? {
            return Ok(session);
        }

        let initialized = initialize().map_err(|error| SessionError::Initialization {
            key: key.clone(),
            message: error.to_string(),
        })?;
        let initialized = Arc::new(Mutex::new(initialized));

        let mut registry = self
            .registry
            .write()
            .map_err(|_| SessionError::RegistryLockPoisoned)?;

        if registry.shut_down {
            return Err(SessionError::ShutDown);
        }

        if let Some(existing) = registry.sessions.get(&key).cloned() {
            drop(registry);
            return downcast_session(&key, existing);
        }

        let erased: ErasedSession = initialized.clone();
        registry.sessions.insert(key.clone(), erased);

        Ok(SessionHandle::new(key, initialized))
    }

    pub fn remove(&self, key: &str) -> Result<bool, SessionError> {
        let mut registry = self
            .registry
            .write()
            .map_err(|_| SessionError::RegistryLockPoisoned)?;

        if registry.shut_down {
            return Err(SessionError::ShutDown);
        }

        Ok(registry.sessions.remove(key).is_some())
    }

    pub fn len(&self) -> Result<usize, SessionError> {
        let registry = self
            .registry
            .read()
            .map_err(|_| SessionError::RegistryLockPoisoned)?;

        if registry.shut_down {
            return Err(SessionError::ShutDown);
        }

        Ok(registry.sessions.len())
    }

    pub fn is_empty(&self) -> Result<bool, SessionError> {
        Ok(self.len()? == 0)
    }

    pub fn shutdown(&self) -> Result<(), SessionError> {
        let mut registry = self
            .registry
            .write()
            .map_err(|_| SessionError::RegistryLockPoisoned)?;

        if registry.shut_down {
            return Ok(());
        }

        registry.shut_down = true;
        registry.sessions.clear();
        Ok(())
    }
}

fn downcast_session<T>(key: &str, session: ErasedSession) -> Result<SessionHandle<T>, SessionError>
where
    T: Send + 'static,
{
    Arc::downcast::<Mutex<T>>(session)
        .map(|session| SessionHandle::new(key.to_string(), session))
        .map_err(|_| SessionError::TypeMismatch {
            key: key.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn reuses_the_same_session_across_calls() {
        let manager = SessionManager::default();
        let first = manager
            .get_or_try_init("wd", || Ok::<_, Infallible>(0_u32))
            .unwrap();
        *first.lock().unwrap() = 42;

        let second = manager
            .get_or_try_init("wd", || Ok::<_, Infallible>(99_u32))
            .unwrap();

        assert_eq!(*second.lock().unwrap(), 42);
        assert_eq!(manager.len().unwrap(), 1);
    }

    #[test]
    fn serializes_access_to_one_session() {
        let manager = Arc::new(SessionManager::default());
        manager
            .get_or_try_init("wd", || Ok::<_, Infallible>(()))
            .unwrap();

        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut workers = Vec::new();

        for _ in 0..4 {
            let manager = Arc::clone(&manager);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);

            workers.push(thread::spawn(move || {
                let session = manager.get::<()>("wd").unwrap().unwrap();
                let _session = session.lock().unwrap();
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                maximum.fetch_max(current, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(10));
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for worker in workers {
            worker.join().unwrap();
        }

        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn shutdown_clears_and_closes_the_manager() {
        let manager = SessionManager::default();
        manager
            .get_or_try_init("wd", || Ok::<_, Infallible>(()))
            .unwrap();

        manager.shutdown().unwrap();

        assert!(matches!(
            manager.get::<()>("wd"),
            Err(SessionError::ShutDown)
        ));
    }
}
