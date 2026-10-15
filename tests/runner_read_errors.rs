use std::io::{self, Read};

use agent_meter::failure::CollectorError;
use agent_meter::runner::read_capped;

struct Failing {
    sent: bool,
}

impl Read for Failing {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.sent {
            return Err(io::Error::other("pipe broke"));
        }
        self.sent = true;
        buf[0] = b'x';
        Ok(1)
    }
}

#[test]
fn a_read_failure_is_an_error_not_truncated_output() {
    let result = read_capped(Failing { sent: false }, 100);
    assert_eq!(result, Err(CollectorError::Unavailable));
}

#[test]
fn a_clean_read_returns_the_bytes_and_flags_overflow_by_length() {
    assert_eq!(read_capped(&b"abc"[..], 100), Ok(b"abc".to_vec()));
    assert_eq!(read_capped(&b"abcdef"[..], 3).map(|b| b.len()), Ok(4));
}
