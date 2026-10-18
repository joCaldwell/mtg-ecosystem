use super::*;
use std::io::Cursor;

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    text: String,
    nested: Nested,
}
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nested {
    quantity: Quantity,
}
const GOOD: &str = r#"{"text":"☃🧪\n\"\\","nested":{"quantity":"-7"}}"#;

struct Fragmented {
    cursor: Cursor<Vec<u8>>,
    width: usize,
    interrupted: bool,
    fail_after: Option<u64>,
    fail_position: bool,
    fail_rewind: bool,
    change_to: Option<Vec<u8>>,
}
impl Fragmented {
    fn new(bytes: &[u8], width: usize) -> Self {
        Self {
            cursor: Cursor::new(bytes.to_vec()),
            width,
            interrupted: false,
            fail_after: None,
            fail_position: false,
            fail_rewind: false,
            change_to: None,
        }
    }
}
impl Read for Fragmented {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        assert!(buffer.len() <= BUFFER, "no unbounded input buffering");
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.fail_after.is_some_and(|n| self.cursor.position() >= n) {
            return Err(io::Error::other("injected read failure"));
        }
        let count = buffer.len().min(self.width);
        self.cursor.read(&mut buffer[..count])
    }
}
impl Seek for Fragmented {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.fail_position && matches!(position, SeekFrom::Current(_)) {
            return Err(io::Error::other("injected position failure"));
        }
        if matches!(position, SeekFrom::Start(_)) {
            if self.fail_rewind {
                return Err(io::Error::other("injected rewind failure"));
            }
            if let Some(replacement) = self.change_to.take() {
                self.cursor = Cursor::new(replacement);
            }
        }
        self.cursor.seek(position)
    }
}

#[test]
fn strict_seekable_matches_original_typed_reader_at_every_fragment_and_nonzero_offset() {
    let expected: Row = strict_read(GOOD).unwrap();
    for width in [1, 2, 3, 7, 63, 64, BUFFER] {
        let mut source = Fragmented::new(format!("prefix{GOOD} \n\t").as_bytes(), width);
        source.cursor.set_position(6);
        source.interrupted = true;
        assert_eq!(
            strict_read_seekable::<Row, _>(&mut source).unwrap(),
            expected
        );
        assert_eq!(
            source.cursor.position(),
            source.cursor.get_ref().len() as u64
        );
    }
    let bytes = format!("{GOOD} \n\t").into_bytes();
    let (_, proof) = pass::<Row>(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(proof.bytes, bytes.len() as u64);
    assert_eq!(proof.sha256, sha256(&bytes));
}

#[test]
fn strict_seekable_rejects_original_unknown_duplicate_nested_trailing_and_truncated_inputs() {
    let cases = [
        r#"{"text":"a","text":"b","nested":{"quantity":"1"}}"#,
        r#"{"text":"a","\u0074ext":"b","nested":{"quantity":"1"}}"#,
        r#"{"text":"a","nested":{"quantity":"1","quantity":"2"}}"#,
        r#"{"text":"a","nested":{"quantity":"1"},"extra":0}"#,
        r#"{"text":"a","nested":{"quantity":"1","extra":0}}"#,
        r#"{"text":"a","nested":{"quantity":"01"}}"#,
        r#"{"text":"a","nested":{"quantity":"1"}} {}"#,
        r#"{"text":"a","nested":{"quantity":"1"}} trailing"#,
    ];
    for raw in cases {
        assert!(strict_read::<Row>(raw).is_err(), "old reader: {raw}");
        assert!(
            strict_read_seekable::<Row, _>(&mut Fragmented::new(raw.as_bytes(), 1)).is_err(),
            "new reader: {raw}"
        );
    }
    for end in 0..GOOD.len() {
        assert!(strict_read_seekable::<Row, _>(&mut Cursor::new(&GOOD.as_bytes()[..end])).is_err());
    }
    assert!(strict_read_seekable::<Row, _>(&mut Cursor::new([0xff, 0xfe])).is_err());
    // Discard-only scan also sees nested duplicates in tagged/otherwise ignored
    // shapes; it must reject before typed serde could discard an extra field.
    assert!(
        strict_read_seekable::<serde_json::Value, _>(&mut Cursor::new(
            br#"{"kind":"pass","ignored":{"x":1,"x":2}}"#
        ))
        .is_err()
    );
}

#[test]
fn strict_seekable_changed_second_pass_cannot_borrow_first_pass_duplicate_proof() {
    for replacement in [
        GOOD.replace("-7", "-8"),
        format!("{GOOD} "),
        GOOD.replace("☃", "λ"),
    ] {
        let mut source = Fragmented::new(GOOD.as_bytes(), 1);
        source.change_to = Some(replacement.into_bytes());
        let error = strict_read_seekable::<Row, _>(&mut source).unwrap_err();
        assert!(error.contains("changed between"));
    }
    // serde Value would ordinarily discard a repeated key; it must not borrow
    // a clean duplicate scan from a different same-length first-pass document.
    let mut replaced = Fragmented::new(br#"{"a":1,"b":2}"#, 1);
    replaced.change_to = Some(br#"{"a":1,"a":2}"#.to_vec());
    assert!(
        strict_read_seekable::<serde_json::Value, _>(&mut replaced)
            .unwrap_err()
            .contains("changed between")
    );
    let mut source = Fragmented::new(GOOD.as_bytes(), 3);
    source.change_to = Some(GOOD.replace("nested", "unknown").into_bytes());
    assert!(strict_read_seekable::<Row, _>(&mut source).is_err());
}

#[test]
fn strict_seekable_read_seek_count_and_eof_errors_never_return_a_typed_value() {
    for end in [0, 1, 13, GOOD.len() as u64] {
        let mut source = Fragmented::new(GOOD.as_bytes(), 1);
        source.fail_after = Some(end);
        assert!(strict_read_seekable::<Row, _>(&mut source).is_err());
    }
    for rewind in [false, true] {
        let mut source = Fragmented::new(GOOD.as_bytes(), 3);
        source.fail_position = !rewind;
        source.fail_rewind = rewind;
        assert!(strict_read_seekable::<Row, _>(&mut source).is_err());
    }
    let mut checked = CheckedRead {
        source: Cursor::new(b"x"),
        bytes: u64::MAX,
        sha256: Sha256Sink::new(),
    };
    assert!(
        checked
            .read(&mut [0])
            .unwrap_err()
            .to_string()
            .contains("overflow")
    );

    struct InvalidRead;
    impl Read for InvalidRead {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            Ok(buffer.len() + 1)
        }
    }
    let mut invalid = CheckedRead {
        source: InvalidRead,
        bytes: 0,
        sha256: Sha256Sink::new(),
    };
    assert!(
        invalid
            .read(&mut [0])
            .unwrap_err()
            .to_string()
            .contains("invalid byte count")
    );
    assert_eq!(invalid.bytes, 0);
    assert_eq!(invalid.sha256.finalize_hex(), sha256(b""));

    struct WrongRewind(Cursor<Vec<u8>>);
    impl Read for WrongRewind {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.0.read(buffer)
        }
    }
    impl Seek for WrongRewind {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            let actual = self.0.seek(position)?;
            Ok(if matches!(position, SeekFrom::Start(_)) {
                actual + 1
            } else {
                actual
            })
        }
    }
    assert!(
        strict_read_seekable::<Row, _>(&mut WrongRewind(Cursor::new(GOOD.as_bytes().to_vec())))
            .unwrap_err()
            .contains("original position")
    );
}
