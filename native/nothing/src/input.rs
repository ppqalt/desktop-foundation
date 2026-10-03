// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded, cancellation-safe JSON-lines input for the event select loop.
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const MAX_LINE: usize = 8192;
pub enum CommandLine {
    Text(String),
    TooLong,
    InvalidUtf8,
}
pub struct Lines<R> {
    reader: R,
    pending: Vec<u8>,
    discarding: bool,
}
impl<R: AsyncBufRead + Unpin> Lines<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            pending: Vec::new(),
            discarding: false,
        }
    }
    fn complete(&mut self) -> CommandLine {
        if self.discarding {
            self.discarding = false;
            return CommandLine::TooLong;
        }
        if self.pending.last() == Some(&b'\r') {
            self.pending.pop();
        }
        let pending = std::mem::take(&mut self.pending);
        if pending.len() > MAX_LINE {
            return CommandLine::TooLong;
        }
        match String::from_utf8(pending) {
            Ok(s) => CommandLine::Text(s),
            Err(_) => CommandLine::InvalidUtf8,
        }
    }
    pub async fn next_line(&mut self) -> io::Result<Option<CommandLine>> {
        loop {
            // The only await precedes state changes. Pending fragments survive
            // cancellation when another socket/event branch wins select!.
            let bytes = self.reader.fill_buf().await?;
            if bytes.is_empty() {
                if self.pending.is_empty() && !self.discarding {
                    return Ok(None);
                }
                return Ok(Some(self.complete()));
            }
            let newline = bytes.iter().position(|b| *b == b'\n');
            let payload = newline.unwrap_or(bytes.len());
            if !self.discarding {
                // One extra CR is allowed before CRLF normalization.
                if self.pending.len() + payload <= MAX_LINE + 1 {
                    self.pending.extend_from_slice(&bytes[..payload]);
                } else {
                    self.pending.clear();
                    self.discarding = true;
                }
            }
            self.reader
                .consume(payload + usize::from(newline.is_some()));
            if newline.is_some() {
                return Ok(Some(self.complete()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncWriteExt, BufReader};
    #[tokio::test]
    async fn oversized_and_invalid_lines_recover_without_unbounded_memory() {
        let data = [
            vec![b'x'; 1024 * 1024],
            b"\n\xff\n{\"action\":\"close\"}\r\nlast".to_vec(),
        ]
        .concat();
        let mut lines = Lines::new(BufReader::new(data.as_slice()));
        assert!(matches!(
            lines.next_line().await.unwrap(),
            Some(CommandLine::TooLong)
        ));
        assert!(lines.pending.capacity() <= 2 * (MAX_LINE + 1));
        assert!(matches!(
            lines.next_line().await.unwrap(),
            Some(CommandLine::InvalidUtf8)
        ));
        assert!(
            matches!(lines.next_line().await.unwrap(), Some(CommandLine::Text(s)) if s == "{\"action\":\"close\"}")
        );
        assert!(
            matches!(lines.next_line().await.unwrap(), Some(CommandLine::Text(s)) if s == "last")
        );
        assert!(lines.next_line().await.unwrap().is_none());
    }
    #[tokio::test]
    async fn cancellation_preserves_partial_command() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let mut lines = Lines::new(BufReader::new(reader));
        writer.write_all(b"{\"action\":").await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), lines.next_line())
                .await
                .is_err()
        );
        writer.write_all(b"\"close\"}\n").await.unwrap();
        assert!(
            matches!(lines.next_line().await.unwrap(), Some(CommandLine::Text(s)) if s == "{\"action\":\"close\"}")
        );
    }
    #[tokio::test]
    async fn exact_limit_with_crlf_is_accepted_and_eof_overflow_is_rejected() {
        let mut data = vec![b'x'; MAX_LINE];
        data.extend_from_slice(b"\r\n");
        data.extend(vec![b'x'; MAX_LINE + 2]);
        let mut lines = Lines::new(BufReader::new(data.as_slice()));
        assert!(
            matches!(lines.next_line().await.unwrap(), Some(CommandLine::Text(s)) if s.len() == MAX_LINE)
        );
        assert!(matches!(
            lines.next_line().await.unwrap(),
            Some(CommandLine::TooLong)
        ));
        assert!(lines.next_line().await.unwrap().is_none());
    }
}
