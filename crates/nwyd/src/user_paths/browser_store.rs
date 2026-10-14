use std::{collections::HashMap, io};

/// Session values take precedence over local storage, including after a quota
/// error leaves an older persisted value behind. Cache successful reads too, so
/// losing storage access later does not discard already loaded preferences.
#[derive(Default)]

pub(super) struct SessionPreferences {
    values: HashMap<String, Vec<u8>>,
}

impl SessionPreferences {
    pub(super) fn read(
        &mut self,
        key: &str,
        persisted: impl FnOnce() -> io::Result<Option<String>>,
    ) -> io::Result<Vec<u8>> {
        if let Some(bytes) = self.values.get(key) {
            return Ok(bytes.clone());
        }

        let bytes: Vec<u8> = persisted()
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_str(&value).ok())
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "no saved browser preference")
            })?;

        self.values.insert(key.to_owned(), bytes.clone());

        Ok(bytes)
    }

    pub(super) fn write(
        &mut self,
        key: &str,
        bytes: &[u8],
        persist: impl FnOnce(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.values.insert(key.to_owned(), bytes.to_vec());

        let value = serde_json::to_string(bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

        // Browser policies and storage quota must not prevent entering the game
        // or changing settings. The latest value remains available this session.
        let _ = persist(&value);

        Ok(())
    }
}

#[cfg(test)]

mod tests {

    use super::*;

    fn denied<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "storage blocked",
        ))
    }

    #[test]

    fn blocked_storage_allows_default_seeding_and_subsequent_changes() {
        let mut session = SessionPreferences::default();

        assert_eq!(
            session.read("guild", denied).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );

        session.write("guild", b"default", |_| denied()).unwrap();

        assert_eq!(session.read("guild", denied).unwrap(), b"default");

        session.write("guild", b"changed", |_| denied()).unwrap();

        assert_eq!(session.read("guild", denied).unwrap(), b"changed");

        assert_eq!(
            session.read("slots", denied).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]

    fn quota_failure_never_restores_the_old_persisted_value() {
        let mut session = SessionPreferences::default();

        let saved = serde_json::to_string(b"old".as_slice()).unwrap();

        assert_eq!(
            session
                .read("settings", || Ok(Some(saved.clone())))
                .unwrap(),
            b"old"
        );

        session
            .write("settings", b"new", |_| {
                Err(io::Error::other("quota exceeded"))
            })
            .unwrap();

        assert_eq!(
            session.read("settings", || Ok(Some(saved))).unwrap(),
            b"new"
        );
    }

    #[test]

    fn existing_preferences_survive_storage_becoming_unavailable() {
        let mut session = SessionPreferences::default();

        assert_eq!(
            session
                .read("slots", || Ok(Some("[0,255,42]".into())))
                .unwrap(),
            [0, 255, 42]
        );

        assert_eq!(session.read("slots", denied).unwrap(), [0, 255, 42]);
    }

    #[test]

    fn missing_and_corrupt_values_allow_game_defaults() {
        for value in [None, Some("invalid".into()), Some("[256]".into())] {
            let mut session = SessionPreferences::default();

            assert_eq!(
                session.read("guild", || Ok(value)).unwrap_err().kind(),
                io::ErrorKind::NotFound
            );

            session.write("guild", b"default", |_| Ok(())).unwrap();

            assert_eq!(session.read("guild", denied).unwrap(), b"default");
        }
    }

    #[test]

    fn working_storage_round_trips_binary_values_across_sessions() {
        let mut session = SessionPreferences::default();

        let mut persisted = None;

        session
            .write("slots", &[0, 255, 42], |value| {
                persisted = Some(value.to_owned());

                Ok(())
            })
            .unwrap();

        let mut next_session = SessionPreferences::default();

        assert_eq!(
            next_session.read("slots", || Ok(persisted)).unwrap(),
            [0, 255, 42]
        );
    }
}
