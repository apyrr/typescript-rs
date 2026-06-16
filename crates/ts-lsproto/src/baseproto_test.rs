use std::{
    cell::RefCell,
    io::{self, Cursor, Write},
    rc::Rc,
};

use crate::{new_base_reader, new_base_writer};

#[test]
fn test_base_reader() {
    let tests = [
        (
            "empty",
            b"Content-Length: 0\r\n\r\n".as_slice(),
            None,
            Some("jsonrpc: no content length"),
        ),
        ("early end", b"oops".as_slice(), None, Some("EOF")),
        (
            "negative length",
            b"Content-Length: -1\r\n\r\n".as_slice(),
            None,
            Some("jsonrpc: invalid content length: negative value -1"),
        ),
        (
            "invalid content",
            b"Content-Length: 1\r\n\r\n{".as_slice(),
            Some(b"{".as_slice()),
            None,
        ),
        (
            "valid content",
            b"Content-Length: 2\r\n\r\n{}".as_slice(),
            Some(b"{}".as_slice()),
            None,
        ),
        (
            "extra header values",
            b"Content-Length: 2\r\nExtra: 1\r\n\r\n{}".as_slice(),
            Some(b"{}".as_slice()),
            None,
        ),
        (
            "too long content length",
            b"Content-Length: 100\r\n\r\n{}".as_slice(),
            None,
            Some("jsonrpc: read content: unexpected EOF"),
        ),
        (
            "missing content length",
            b"Content-Length: \r\n\r\n{}".as_slice(),
            None,
            Some("jsonrpc: invalid content length: parse error"),
        ),
        (
            "invalid header",
            b"Nope\r\n\r\n{}".as_slice(),
            None,
            Some("jsonrpc: invalid header"),
        ),
    ];

    for (name, input, value, err) in tests {
        let r = new_base_reader(Cursor::new(input));

        let out = r.read();
        if let Some(err) = err {
            assert!(
                out.as_ref()
                    .err()
                    .is_some_and(|actual| actual.to_string().contains(err)),
                "expected error containing {err:?} in {name}, got {out:?}"
            );
        } else {
            assert_eq!(out.unwrap(), value.unwrap(), "{name}");
        }
    }
}

#[test]
fn test_base_reader_multiple_reads() {
    let data = b"Content-Length: 4\r\n\r\n1234Content-Length: 2\r\n\r\n{}";
    let r = new_base_reader(Cursor::new(data));

    let v1 = r.read().unwrap();
    assert_eq!(v1, b"1234");

    let v2 = r.read().unwrap();
    assert_eq!(v2, b"{}");

    let err = r.read().unwrap_err();
    assert!(err.to_string().contains("EOF"));
}

#[test]
fn test_base_writer() {
    let tests = [
        (
            "empty",
            b"{}".as_slice(),
            b"Content-Length: 2\r\n\r\n{}".as_slice(),
        ),
        (
            "bigger object",
            br#"{"key":"value"}"#.as_slice(),
            b"Content-Length: 15\r\n\r\n{\"key\":\"value\"}".as_slice(),
        ),
    ];

    for (name, value, input) in tests {
        let b = SharedBuffer::default();
        let w = new_base_writer(b.clone());
        w.write(value).unwrap();
        assert_eq!(b.bytes(), input, "{name}");
    }
}

#[test]
fn test_base_writer_write_error() {
    let w = new_base_writer(ErrorWriter);
    let err = w.write(b"{}").unwrap_err();
    assert_eq!(err.to_string(), "test error");
}

#[derive(Clone, Default)]
struct SharedBuffer(Rc<RefCell<Vec<u8>>>);

impl SharedBuffer {
    fn bytes(&self) -> Vec<u8> {
        self.0.borrow().clone()
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ErrorWriter;

impl Write for ErrorWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "test error"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
