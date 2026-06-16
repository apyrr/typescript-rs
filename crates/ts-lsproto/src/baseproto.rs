use std::io::{Read, Write};

use ts_jsonrpc as jsonrpc;

// https://microsoft.github.io/language-server-protocol/specifications/base/0.9/specification/

// BaseReader wraps jsonrpc.Reader for backwards compatibility.
pub struct BaseReader<R> {
    pub reader: jsonrpc::Reader<R>,
}

// NewBaseReader creates a new BaseReader.
pub fn new_base_reader<R: Read>(reader: R) -> BaseReader<R> {
    BaseReader {
        reader: jsonrpc::Reader::new(reader),
    }
}

impl<R: Read> BaseReader<R> {
    pub fn read(&self) -> std::io::Result<Vec<u8>> {
        self.reader.read()
    }
}

// BaseWriter wraps jsonrpc.Writer for backwards compatibility.
pub struct BaseWriter<W: Write> {
    pub writer: jsonrpc::Writer<W>,
}

// NewBaseWriter creates a new BaseWriter.
pub fn new_base_writer<W: Write>(writer: W) -> BaseWriter<W> {
    BaseWriter {
        writer: jsonrpc::Writer::new(writer),
    }
}

impl<W: Write> BaseWriter<W> {
    pub fn write(&self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write(data)
    }
}
