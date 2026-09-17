use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Default)]
struct FileCursor {
    offset: u64,
    len: u64,
    mtime: Option<SystemTime>,
    /// The most recent model id this session file has reported (e.g. from a
    /// Codex `session_meta`/`turn_context` event), carried across
    /// incremental parses so a later `token_count` line that doesn't repeat
    /// the model id can still be attributed to it. See agents/codex.rs.
    current_model: Option<String>,
}

/// Per-agent tail-reading state: for each known file, remembers how far it
/// has already been read so each poll tick only reads newly appended bytes
/// instead of re-parsing the whole file (or the whole log history) every
/// minute.
#[derive(Default)]
pub struct ScanCursors {
    cursors: HashMap<PathBuf, FileCursor>,
}

impl ScanCursors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns newly appended, complete lines for `path` since the last call.
    /// If the file shrank (rotated/truncated) the cursor resets to the start.
    pub fn read_new_lines(&mut self, path: &Path) -> std::io::Result<Vec<String>> {
        let metadata = std::fs::metadata(path)?;
        let len = metadata.len();
        let mtime = metadata.modified().ok();

        let cursor = self.cursors.entry(path.to_path_buf()).or_default();

        if cursor.len == len && cursor.mtime == mtime {
            return Ok(Vec::new());
        }

        let start_offset = if len < cursor.offset {
            0
        } else {
            cursor.offset
        };

        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(start_offset))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        // Only count complete (newline-terminated) lines; a partial trailing
        // line (writer mid-append) is left for the next tick rather than
        // parsed early and skipped.
        let mut lines = Vec::new();
        let mut line_start = 0usize;
        let mut consumed: usize = 0;
        for (i, b) in buf.iter().enumerate() {
            if *b == b'\n' {
                let raw = &buf[line_start..i];
                let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
                lines.push(String::from_utf8_lossy(raw).into_owned());
                line_start = i + 1;
                consumed = line_start;
            }
        }

        cursor.offset = start_offset + consumed as u64;
        cursor.len = len;
        cursor.mtime = mtime;

        Ok(lines)
    }

    /// The most recently recorded model id for `path` (see `set_model`).
    pub fn current_model(&self, path: &Path) -> Option<String> {
        self.cursors.get(path).and_then(|c| c.current_model.clone())
    }

    /// Records the model id currently in effect for `path`, so later lines
    /// that don't carry their own model id can still be attributed to it.
    pub fn set_model(&mut self, path: &Path, model: String) {
        self.cursors
            .entry(path.to_path_buf())
            .or_default()
            .current_model = Some(model);
    }
}
