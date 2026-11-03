use std::time::Duration;

/// Run a file operation, retrying briefly while something (antivirus, a launcher) has the
/// file open: Windows' sharing (32) and lock (33) violations. Blocking.
pub fn retry_in_use<T>(mut op: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    for _ in 0..5 {
        match op() {
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                std::thread::sleep(Duration::from_millis(200));
            }
            done => return done,
        }
    }
    op()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_while_the_file_is_in_use() {
        let mut calls = 0;
        let r = retry_in_use(|| {
            calls += 1;
            if calls < 3 {
                Err(std::io::Error::from_raw_os_error(32))
            } else {
                Ok(calls)
            }
        });
        assert_eq!(r.unwrap(), 3);

        let mut calls = 0;
        let r: std::io::Result<()> = retry_in_use(|| {
            calls += 1;
            Err(std::io::ErrorKind::NotFound.into())
        });
        assert!(r.is_err());
        assert_eq!(calls, 1, "other errors aren't retried");
    }
}
