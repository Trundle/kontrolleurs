use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::fd::AsFd,
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use regex::Regex;
use termion::{
    event::Key,
    input::TermRead,
    raw::{IntoRawMode, RawTerminal},
};
use termwiz::cell::unicode_column_width;

use terminal_size::terminal_size;

mod terminal_size;

struct HistoryIter<R: BufRead> {
    reader: R,
}

impl<R: BufRead> HistoryIter<R> {
    pub fn from_reader(reader: R) -> Self {
        Self { reader }
    }
}

impl<R: BufRead> Iterator for HistoryIter<R> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut buf = Vec::with_capacity(1024);
            let mut bytes = self.reader.read_until(b'\0', &mut buf).ok()?;
            if bytes == 0 {
                return None;
            }
            // Omit trailing null byte if present
            if buf[bytes - 1] == b'\0' {
                bytes -= 1;
            }
            let Some(entry) = std::str::from_utf8(&buf[..bytes])
                .ok()
                .map(ToOwned::to_owned)
            else {
                // Skip undecodable entries, rather than returning a likely wrong entry
                continue;
            };
            return Some(entry);
        }
    }
}

/// An iterator that can be started from the beginning again, by memorizing all items.
struct ReusableIter<I: Iterator, T> {
    consumed_iter: <Vec<T> as IntoIterator>::IntoIter,
    inner: I,
    elements: Vec<T>,
}

impl<I: Iterator<Item = T>, T> ReusableIter<I, T> {
    pub fn new(inner: I) -> Self {
        Self {
            consumed_iter: Vec::new().into_iter(),
            inner,
            elements: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.elements
            .extend(std::mem::take(&mut self.consumed_iter));
        let elements = std::mem::take(&mut self.elements);
        self.consumed_iter = elements.into_iter();
    }
}

impl<I: Iterator<Item = T>, T: Clone> Iterator for ReusableIter<I, T> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.consumed_iter.next().or_else(|| self.inner.next()) {
            self.elements.push(next.clone());
            Some(next)
        } else {
            None
        }
    }
}

fn adjust_cursor(pos: usize, key: Key) -> usize {
    match key {
        Key::Left if pos > 0 => pos - 1,
        Key::Right => pos + 1,
        Key::Home => 0,
        Key::End =>
        // Really just a large number and fish then places at the end
        {
            65536
        }
        _ => pos,
    }
}

#[derive(Debug, PartialEq)]
enum PromptResult {
    Incomplete,
    Selected(String, bool, usize),
    Quit,
}

struct Prompt<I: Iterator<Item = String>, W: Write + AsFd> {
    input: String,
    history: ReusableIter<I, String>,
    stdout: RawTerminal<W>,
    /// (columns, rows)
    terminal_size: (u16, u16),
    current_input_height: usize,
    current_entry: Option<String>,
}

impl<I: Iterator<Item = String>, W: Write + AsFd> Prompt<I, W> {
    pub fn new(stdout: RawTerminal<W>, history: I) -> std::io::Result<Self> {
        let terminal_size = terminal_size(&stdout.as_fd())?;
        Ok(Self {
            input: String::new(),
            history: ReusableIter::new(history),
            stdout,
            terminal_size,
            current_input_height: 0,
            current_entry: None,
        })
    }

    pub fn handle_key_press(&mut self, key: Key) -> PromptResult {
        match key {
            Key::Esc | Key::Ctrl('c' | 'g') => PromptResult::Quit,
            Key::Char('\n') | Key::Left | Key::Right | Key::Home | Key::End => {
                let execute = key == Key::Char('\n');
                if let Some(ref entry) = self.current_entry {
                    let cursor = self
                        .input_to_regex()
                        .find(entry)
                        .expect("Current entry should match input")
                        .end();
                    PromptResult::Selected(entry.clone(), execute, adjust_cursor(cursor, key))
                } else {
                    PromptResult::Quit
                }
            }
            Key::Ctrl('r') => {
                self.update();
                PromptResult::Incomplete
            }
            Key::Backspace => {
                self.input.pop();
                self.history.reset();
                self.update();
                PromptResult::Incomplete
            }
            Key::Char(ch) => {
                self.input.push(ch);
                self.history.reset();
                self.update();
                PromptResult::Incomplete
            }
            _ => PromptResult::Incomplete,
        }
    }

    pub fn handle_terminal_size_change(&mut self) {
        let new_size = terminal_size(&self.stdout.as_fd()).unwrap();
        if new_size.0 != self.terminal_size.0 {
            let prompt = self.prompt();
            self.current_input_height =
                unicode_column_width(&prompt, None).div_ceil(new_size.0.into());
        }
        self.terminal_size = new_size;
    }

    fn update(&mut self) {
        self.current_entry = self
            .history
            .find(|x| x.to_lowercase().contains(&self.input.to_lowercase()));
        self.redraw();
    }

    pub fn redraw(&mut self) {
        let prompt = self.prompt();
        let _ = write!(
            self.stdout,
            "{}\r{}{}",
            if self.current_input_height > 1 {
                termion::cursor::Up((self.current_input_height - 1).try_into().unwrap()).to_string()
            } else {
                String::new()
            },
            prompt,
            termion::clear::AfterCursor
        );
        self.current_input_height =
            unicode_column_width(&prompt, None).div_ceil(self.terminal_size.0.into());
        if let Some(ref entry) = self.current_entry {
            let highlight = self.input_to_regex();
            let mut entry_height = 0;
            for line in entry.lines() {
                Self::print_line(line, &highlight, &mut self.stdout);
                entry_height +=
                    unicode_column_width(line, None).div_ceil(self.terminal_size.0.into());
            }
            let cursor_col: usize =
                unicode_column_width(&prompt, None) % self.terminal_size.0 as usize;
            let _ = write!(
                self.stdout,
                "{}\r{}",
                termion::cursor::Up(entry_height.try_into().unwrap()),
                termion::cursor::Right(cursor_col.try_into().unwrap()),
            );
        }
        let _ = self.stdout.flush();
    }

    fn print_line(line: &str, highlight: &Regex, stdout: &mut RawTerminal<W>) {
        let _ = write!(stdout, "\r\n");
        let mut last_end = 0;
        for m in highlight.find_iter(line) {
            let _ = write!(
                stdout,
                "{}{}{}{}{}{}",
                &line[last_end..m.start()],
                termion::color::Fg(termion::color::Red),
                termion::style::Invert,
                termion::style::Bold,
                m.as_str(),
                termion::style::Reset
            );
            last_end = m.end();
        }
        let _ = write!(stdout, "{}", &line[last_end..]);
    }

    fn input_to_regex(&self) -> Regex {
        Regex::new(&format!("(?i){}", regex::escape(&self.input)))
            .expect("Should be valid regex pattern")
    }

    fn prompt(&self) -> String {
        format!("bck-i-search: {}", self.input)
    }
}

fn main() -> ExitCode {
    let Ok(stdin) = File::open("/dev/tty") else {
        eprintln!("[FATAL] Could not open TTY");
        return ExitCode::FAILURE;
    };
    let Ok(stdout) = File::create("/dev/tty") else {
        eprintln!("[FATAL] Could not open TTY");
        return ExitCode::FAILURE;
    };
    let stdout = stdout.into_raw_mode().unwrap();
    let history = HistoryIter::from_reader(BufReader::new(std::io::stdin()));
    let mut prompt = Prompt::new(stdout, history).unwrap();
    prompt.redraw();

    let winch = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGWINCH, Arc::clone(&winch))
        .expect("Registering signal handler should work");

    let mut selection = None;
    for key in stdin.keys() {
        let Ok(key) = key else {
            continue;
        };
        if winch.load(Ordering::Acquire) {
            winch.store(false, Ordering::SeqCst);
            prompt.handle_terminal_size_change();
        }
        match prompt.handle_key_press(key) {
            PromptResult::Incomplete => (),
            PromptResult::Selected(entry, execute, cursor_pos) => {
                selection = Some((entry, execute, cursor_pos));
                break;
            }
            PromptResult::Quit => break,
        }
    }
    drop(prompt);

    if let Some((entry, execute, cursor_pos)) = selection {
        println!("{execute}");
        println!("{cursor_pos}");
        print!("{entry}\0");
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{HistoryIter, ReusableIter};

    fn collect_history(input: &[u8]) -> Vec<String> {
        let reader = std::io::Cursor::new(input);
        HistoryIter::from_reader(reader).collect()
    }

    #[test]
    fn test_history_iter() {
        let lines = collect_history(b"entry1\0entry2\0entry 3\nstill entry 3\0");
        assert_eq!(
            lines,
            vec![
                "entry1".to_string(),
                "entry2".to_string(),
                "entry 3\nstill entry 3".to_string()
            ]
        );
    }

    #[test]
    fn test_history_iter_missing_traling_null() {
        let lines = collect_history(b"first entry");
        assert_eq!(lines, vec!["first entry".to_string()]);
    }

    #[test]
    fn test_history_iter_invalid_utf_8() {
        let lines = collect_history(b"first en\xc3try\0second entry\0");
        assert_eq!(lines, vec!["second entry".to_string()]);
    }

    #[test]
    fn test_reusable_iter() {
        let mut iter = ReusableIter::new(["spam", "eggs"].iter());
        assert_eq!(iter.next(), Some("spam").as_ref());
        assert_eq!(iter.next(), Some("eggs").as_ref());
        assert_eq!(iter.next(), None);

        iter.reset();
        assert_eq!(iter.next(), Some("spam").as_ref());

        iter.reset();
        assert_eq!(vec![&"spam", &"eggs"], iter.collect::<Vec<_>>());
    }
}
