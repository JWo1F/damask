/// Maps byte offsets to zero-based `(line, column)` positions.
///
/// Columns are counted in UTF-16 code units, matching the Language Server
/// Protocol's default position encoding, so the LSP can translate parser spans
/// into LSP ranges directly.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line.
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        LineIndex {
            line_starts,
            len: src.len(),
        }
    }

    /// Convert a byte offset into a zero-based `(line, utf16_column)`.
    ///
    /// `src` must be the same string this index was built from. Offsets past the
    /// end are clamped to the end of the text.
    pub fn line_col(&self, src: &str, offset: usize) -> (u32, u32) {
        let offset = offset.min(self.len);
        // Binary search for the last line start <= offset.
        let line = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        // UTF-16 column: sum code-unit widths of chars in [line_start, offset).
        let col = src[line_start..offset]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();
        (line as u32, col)
    }
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    #[test]
    fn resolves_lines_and_columns() {
        let src = "ab\ncde\nf";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(src, 0), (0, 0));
        assert_eq!(idx.line_col(src, 2), (0, 2)); // end of line 0
        assert_eq!(idx.line_col(src, 3), (1, 0)); // start of line 1
        assert_eq!(idx.line_col(src, 6), (1, 3)); // end of line 1
        assert_eq!(idx.line_col(src, 7), (2, 0)); // start of line 2
    }

    #[test]
    fn utf16_columns_count_surrogate_pairs() {
        // "😀" is one char, two UTF-16 code units, four UTF-8 bytes.
        let src = "😀x";
        let idx = LineIndex::new(src);
        let x_offset = "😀".len();
        assert_eq!(idx.line_col(src, x_offset), (0, 2));
    }

    #[test]
    fn clamps_past_end() {
        let src = "abc";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(src, 999), (0, 3));
    }
}
