// use crate::keymap::{At, CharSearch, Movement, usize, Word};
use std::fmt;
use std::io::Write;
use std::iter;
use std::ops::{Deref, Index, Range};
use std::string::Drain;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Maximum buffer size for the line read
pub(crate) static MAX_LINE: usize = 4096;

/// Delete (kill) direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Forward,
    Backward,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::Forward
    }
}

/// Represents the current input (text and cursor position).
///
/// The methods do text manipulations or/and cursor movements.
pub struct LineBuffer {
    buf: String, // Edited line buffer (rl_line_buffer)
    pos: usize,  // Current cursor position (byte position) (rl_point)
}

impl fmt::Debug for LineBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineBuffer")
            .field("buf", &self.buf)
            .field("pos", &self.pos)
            .finish()
    }
}

impl LineBuffer {
    /// Create a new line buffer with the given maximum `capacity`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: String::with_capacity(capacity),
            pos: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn init(line: &str, pos: usize) -> Self {
        let mut lb = Self::with_capacity(MAX_LINE);
        assert!(lb.insert_str(0, line));
        lb.set_pos(pos);
        lb
    }

    /// Extracts a string slice containing the entire buffer.
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// Converts a buffer into a `String` without copying or allocating.
    pub fn into_string(self) -> String {
        self.buf
    }

    /// Current cursor position (byte position)
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Set cursor position (byte position)
    pub fn set_pos(&mut self, pos: usize) {
        assert!(pos <= self.buf.len());
        self.pos = pos;
    }

    /// Returns the length of this buffer, in bytes.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if this buffer has a length of zero.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Set line content (`buf`) and cursor position (`pos`).
    pub fn update(&mut self, buf: &str, pos: usize) {
        assert!(pos <= buf.len());
        let end = self.len();
        self.drain(0..end, Direction::default());
        let max = self.buf.capacity();
        if buf.len() > max {
            self.insert_str(0, &buf[..max]);
            if pos > max {
                self.pos = max;
            } else {
                self.pos = pos;
            }
        } else {
            self.insert_str(0, buf);
            self.pos = pos;
        }
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        let end = self.len();
        self.drain(0..end, Direction::default());
        self.pos = 0;
    }

    /// Returns the position of the character just after the current cursor
    /// position.
    pub fn next_pos(&self, n: usize) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .take(n)
            .last()
            .map(|(i, s)| i + self.pos + s.len())
    }

    /// Returns the position of the character just before the current cursor
    /// position.
    fn prev_pos(&self, n: usize) -> Option<usize> {
        if self.pos == 0 {
            return None;
        }
        self.buf[..self.pos]
            .grapheme_indices(true)
            .rev()
            .take(n)
            .last()
            .map(|(i, _)| i)
    }

    /// Insert the character `ch` at current cursor position
    /// and advance cursor position accordingly.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn insert(&mut self, ch: char, n: usize) -> Option<bool> {
        let shift = ch.len_utf8() * n;
        // if self.buf.len() + shift > self.buf.capacity() {
        //     return None;
        // }
        let push = self.pos == self.buf.len();
        if n == 1 {
            self.buf.insert(self.pos, ch);
        } else {
            let text = iter::repeat(ch).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Move cursor on the left.
    pub fn move_backward(&mut self, n: usize) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor on the right.
    pub fn move_forward(&mut self, n: usize) -> bool {
        match self.next_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor to the start of the line.
    pub fn move_home(&mut self) -> bool {
        if self.pos > 0 {
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) -> bool {
        if self.pos == self.buf.len() {
            false
        } else {
            self.pos = self.buf.len();
            true
        }
    }

    /// Delete the character at the right of the cursor without altering the
    /// cursor position. Basically this is what happens with the "Delete"
    /// keyboard key.
    /// Return the number of characters deleted.
    pub fn delete(&mut self, n: usize) -> Option<String> {
        match self.next_pos(n) {
            Some(pos) => {
                let start = self.pos;
                let chars = self
                    .drain(start..pos, Direction::Forward)
                    .collect::<String>();
                Some(chars)
            }
            None => None,
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace(&mut self, n: usize) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                let end = self.pos;
                self.drain(pos..end, Direction::Backward);
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Kill the text from point to the end of the line.
    pub fn kill_line(&mut self) -> bool {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let start = self.pos;
            let end = self.buf.len();
            self.drain(start..end, Direction::Forward);
            true
        } else {
            false
        }
    }

    /// Kill backward from point to the beginning of the line.
    pub fn discard_line(&mut self) -> bool {
        if self.pos > 0 && !self.buf.is_empty() {
            let end = self.pos;
            self.drain(0..end, Direction::Backward);
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Replaces the content between [`start`..`end`] with `text`
    /// and positions the cursor to the end of text.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = range.start;
        self.buf.drain(range);
        if start == self.buf.len() {
            self.buf.push_str(text);
        } else {
            self.buf.insert_str(start, text);
        }
        self.pos = start + text.len();
    }

    /// Insert the `s`tring at the specified position.
    /// Return `true` if the text has been inserted at the end of the line.
    pub fn insert_str(&mut self, idx: usize, s: &str) -> bool {
        if idx == self.buf.len() {
            self.buf.push_str(s);
            true
        } else {
            self.buf.insert_str(idx, s);
            false
        }
    }

    /// Remove the specified `range` in the line.
    pub fn delete_range(&mut self, range: Range<usize>) {
        self.set_pos(range.start);
        self.drain(range, Direction::default());
    }

    fn drain(&mut self, range: Range<usize>, dir: Direction) -> Drain<'_> {
        self.buf.drain(range)
    }
}

impl Deref for LineBuffer {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub col: usize,
    pub row: usize,
}

pub(super) struct Renderer {
    line: LineBuffer,
    out: std::io::Stdout,
}

impl Renderer {
    pub fn new(out: std::io::Stdout) -> Self {
        Self {
            line: LineBuffer::with_capacity(512),
            out,
        }
    }

    pub fn beep(&mut self) {
        self.out.write_all(b"\x07").unwrap();
        self.out.flush().unwrap();
    }

    pub fn move_rel(&mut self, bytes: &[u8]) {
        use std::convert::TryInto;
        let pos = i16::from_be_bytes(bytes[0..2].try_into().unwrap());
        // info!("move: pos={}", pos);
        if pos < 0 {
            self.line.move_backward(-pos as usize);
            write!(self.out, "{}", termion::cursor::Left(-pos as u16));
        } else {
            self.line.move_forward(pos as usize);
            write!(self.out, "{}", termion::cursor::Right(pos as u16));
        }
        self.out.flush().unwrap();
    }

    pub fn put_chars(&mut self, chars: &[u8]) {
        self.line.move_end();

        for c in chars {
            if *c == b'\n' {
                self.out.write_all(b"\r\n").unwrap();
                self.out.flush().unwrap();
                self.line.clear();
            } else {
                assert!(self.line.insert(*c as char, 1).is_some());
                self.out.write_all(&[*c]).unwrap();
            }
            // move cursor
        }
        self.out.flush().unwrap();
    }

    pub fn insert_chars(&mut self, chars: &[u8]) {
        for c in chars {
            if *c == b'\n' {
                self.out.write_all(b"\r\n").unwrap();
                self.out.flush().unwrap();
                self.line.clear();
            } else {
                assert!(self.line.insert(*c as char, 1).is_some())
            }
            // move cursor
        }
        // TODO: need to redraw more efficiently and with multiline
        write!(self.out, "\r{}", termion::clear::CurrentLine);
        self.out.write_all(self.line.as_str().as_bytes()).unwrap();
        write!(
            self.out,
            "\r{}",
            termion::cursor::Right(self.line.pos as u16)
        );
        self.out.flush().unwrap();
    }

    pub fn delete_chars(&mut self, bytes: &[u8]) {
        use std::convert::TryInto;
        let n = i16::from_be_bytes(bytes[0..2].try_into().unwrap());
        // info!("delete_chars: n={}", n);
        if n > 0 {
            // delete forwards
            self.line.delete(n as usize);
        } else {
            // delete backwards
            self.line.backspace(n.abs() as usize);
        }
        // TODO: need to redraw more efficiently and with multiline
        write!(self.out, "\r{}", termion::clear::CurrentLine);
        self.out.write_all(self.line.as_str().as_bytes()).unwrap();
        write!(
            self.out,
            "\r{}",
            termion::cursor::Right(self.line.pos as u16)
        );
        self.out.flush().unwrap();
    }
}

fn width(s: &str, esc_seq: &mut u8) -> usize {
    if *esc_seq == 1 {
        if s == "[" {
            // CSI
            *esc_seq = 2;
        } else {
            // two-character sequence
            *esc_seq = 0;
        }
        0
    } else if *esc_seq == 2 {
        if s == ";" || (s.as_bytes()[0] >= b'0' && s.as_bytes()[0] <= b'9') {
            /*} else if s == "m" {
            // last
             *esc_seq = 0;*/
        } else {
            // not supported
            *esc_seq = 0;
        }
        0
    } else if s == "\x1b" {
        *esc_seq = 1;
        0
    } else if s == "\n" {
        0
    } else {
        s.width()
    }
}

const TAB_STOP: usize = 4;
const COLS: usize = 200;

// calculate_position(line, Position::default());

/// Control characters are treated as having zero width.
/// Characters with 2 column width are correctly handled (not split).
fn calculate_position(s: &str, orig: Position) -> Position {
    let mut pos = orig;
    let mut esc_seq = 0;
    for c in s.graphemes(true) {
        if c == "\n" {
            pos.row += 1;
            pos.col = 0;
            continue;
        }
        let cw = if c == "\t" {
            TAB_STOP - (pos.col % TAB_STOP)
        } else {
            width(c, &mut esc_seq)
        };
        pos.col += cw;
        if pos.col > COLS {
            pos.row += 1;
            pos.col = cw;
        }
    }
    if pos.col == COLS {
        pos.col = 0;
        pos.row += 1;
    }
    pos
}

#[cfg(test)]
mod test {
    use super::{Direction, LineBuffer, MAX_LINE};
    // use crate::keymap::{At, CharSearch, Word};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn next_pos() {
        let s = LineBuffer::init("ö̲g̈", 0);
        assert_eq!(7, s.len());
        let pos = s.next_pos(1);
        assert_eq!(Some(4), pos);

        let s = LineBuffer::init("ö̲g̈", 4);
        let pos = s.next_pos(1);
        assert_eq!(Some(7), pos);
    }

    #[test]
    fn prev_pos() {
        let s = LineBuffer::init("ö̲g̈", 4);
        assert_eq!(7, s.len());
        let pos = s.prev_pos(1);
        assert_eq!(Some(0), pos);

        let s = LineBuffer::init("ö̲g̈", 7);
        let pos = s.prev_pos(1);
        assert_eq!(Some(4), pos);
    }

    #[test]
    fn insert() {
        let mut s = LineBuffer::with_capacity(MAX_LINE);
        let push = s.insert('α', 1).unwrap();
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, push);

        let push = s.insert('ß', 1).unwrap();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, push);

        s.pos = 0;
        let push = s.insert('γ', 1).unwrap();
        assert_eq!("γαß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(false, push);
    }

    #[test]
    fn moves() {
        let mut s = LineBuffer::init("αß", 4);
        let ok = s.move_backward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_forward(1);
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_home();
        assert_eq!("αß", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);

        let ok = s.move_end();
        assert_eq!("αß", s.buf);
        assert_eq!(4, s.pos);
        assert_eq!(true, ok);
    }

    #[test]
    fn move_grapheme() {
        let mut s = LineBuffer::init("ag̈", 4);
        assert_eq!(4, s.len());
        let ok = s.move_backward(1);
        assert_eq!(true, ok);
        assert_eq!(1, s.pos);

        let ok = s.move_forward(1);
        assert_eq!(true, ok);
        assert_eq!(4, s.pos);
    }

    #[test]
    fn delete() {
        let mut s = LineBuffer::init("αß", 2);
        let chars = s.delete(1);
        assert_eq!("α", s.buf);
        assert_eq!(2, s.pos);
        assert_eq!(Some("ß".to_owned()), chars);

        let ok = s.backspace(1);
        assert_eq!("", s.buf);
        assert_eq!(0, s.pos);
        assert_eq!(true, ok);
    }
}
