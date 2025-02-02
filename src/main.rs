use std::fs::{File, OpenOptions};
use std::io::{stdout, BufReader, BufWriter, Write};
use std::path::PathBuf;

use clap::Parser;
use crossterm::{cursor, event, execute, style, terminal, ExecutableCommand};
use ropey::Rope;

const COMMAND_VIEW_ROWS: u16 = 2;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    filename: Option<PathBuf>,
}

struct Editor {
    text: Rope,
    filename: Option<PathBuf>,
    cursor_col: u16,
    cursor_row: u16,
    shift_row: usize,
    mode: &'static str,
    cmd_message: Rope,
    dirty: bool,
}

impl Editor {
    fn currline(&self) -> String {
        let mut currline = self
            .text
            .line(self.shift_row + self.cursor_row as usize)
            .to_string();
        if currline.ends_with('\n') {
            currline.pop();
        }
        currline
    }
    fn line_max(&self) -> u16 {
        self.currline().chars().into_iter().count() as u16
    }
    fn save(&mut self) -> bool {
        if let Some(&ref pathbuf) = self.filename.as_ref() {
            self.text
                .write_to(BufWriter::new(File::create(pathbuf).unwrap()))
                .unwrap();
            self.cmd_message.remove(0..self.cmd_message.len_chars());
            self.cmd_message
                .insert(0, &format!("{:?} written", self.filename.as_ref().unwrap()));
            self.dirty = false;
            true
        } else {
            self.cmd_message.remove(0..self.cmd_message.len_chars());
            self.cmd_message
                .insert(0, "Cannot save file without a name");
            false
        }
    }
    fn render(&self) -> std::io::Result<()> {
        let (cols, rows) = terminal::size()?;
        stdout()
            .execute(terminal::Clear(terminal::ClearType::All))?
            .execute(cursor::MoveTo(0, 0))?
            .execute(style::SetForegroundColor(style::Color::Blue))?
            .execute(style::ResetColor)?;

        for (line, i) in self
            .text
            .lines_at(self.shift_row)
            .zip(0..(rows - COMMAND_VIEW_ROWS).min(self.text.len_lines() as u16))
        {
            let mut string_line = line.to_string();
            if string_line.ends_with('\n') {
                string_line.pop();
            }
            let colls_string = &string_line[..(string_line.len().min(cols as usize))];
            stdout().execute(style::Print(colls_string))?;

            if i != rows - 2 {
                stdout().execute(style::Print("\r\n"))?;
            } else {
                stdout().execute(style::Print("\r"))?;
            }
        }

        let filename_label;
        if self.filename.is_some() {
            filename_label = format!(" | {}", self.filename.as_ref().unwrap().to_str().unwrap())
        } else {
            filename_label = "".to_string();
        }

        let dirty_label;
        if self.dirty {
            dirty_label = " | +"
        } else {
            dirty_label = ""
        }

        stdout().execute(cursor::MoveTo(0, rows - 2))?;
        stdout().execute(style::Print(format!(
            "{}{}{}\r\n{}",
            self.mode, filename_label, dirty_label, self.cmd_message
        )))?;
        stdout().execute(cursor::MoveTo(self.cursor_col, self.cursor_row))?;

        if self.mode == "Normal" {
            stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
        } else if self.mode == "Insert" {
            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
        } else if self.mode == "Command" {
            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
        } else {
            stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
        }
        Ok(())
    }
}

fn run(mut logs: Option<File>, filename: Option<PathBuf>) -> std::io::Result<()> {
    let text;
    if let Some(&ref pathbuf) = filename.as_ref() {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(pathbuf)?;

        text = Rope::from_reader(BufReader::new(file))?;
    } else {
        text = Rope::new();
    }

    let mut editor = Editor {
        text,
        filename,
        cursor_col: 0,
        cursor_row: 0,
        shift_row: 0,
        mode: "Normal",
        cmd_message: Rope::new(),
        dirty: false,
    };

    let mut prefered_col: Option<u16> = None;

    let mut prev_cursor_row = 0;
    let mut prev_cursor_col = 0;

    loop {
        let (cols, rows) = terminal::size()?;

        if let Some(logs) = logs.as_mut() {
            writeln!(logs, "Size ({} x {})", cols, rows)?;
        }

        editor.render()?;
        // Events
        let ev = event::read()?;

        if let Some(logs) = logs.as_mut() {
            writeln!(logs, "Got event {:?}", ev)?;
            writeln!(logs, "shift_row {}", editor.shift_row)?;
            writeln!(logs, "text len lines {}", editor.text.len_lines())?;
        }

        if let event::Event::Key(keyev) = ev {
            if let event::KeyCode::Char(c) = keyev.code {
                if c != 'j' && c != 'k' {
                    prefered_col = None;
                }
            }
        }

        match ev {
            event::Event::Key(keyev) => match (keyev.code, editor.mode) {
                (event::KeyCode::Char('q'), "Normal") => {}
                (event::KeyCode::Char('h'), "Normal") => {
                    if editor.cursor_col != 0 {
                        editor.cursor_col -= 1;
                    }
                }
                (event::KeyCode::Char('j'), "Normal") => {
                    if let None = prefered_col {
                        prefered_col = Some(editor.cursor_col);
                    }

                    if (editor.cursor_row != rows - 1 - COMMAND_VIEW_ROWS)
                        && (((editor.cursor_row + 1) as usize) < editor.text.len_lines())
                    {
                        editor.cursor_row += 1;
                    } else {
                        if (editor.shift_row + rows as usize - COMMAND_VIEW_ROWS as usize)
                            < editor.text.len_lines() - 1
                        {
                            editor.shift_row += 1;
                        }
                    }

                    editor.cursor_col = prefered_col.unwrap().min(editor.line_max());
                }
                (event::KeyCode::Char('k'), "Normal") => {
                    if let None = prefered_col {
                        prefered_col = Some(editor.cursor_col);
                    }

                    if editor.cursor_row != 0 {
                        editor.cursor_row -= 1;
                    } else {
                        if editor.shift_row != 0 {
                            editor.shift_row -= 1;
                        }
                    }

                    editor.cursor_col = prefered_col.unwrap().min(editor.line_max());
                }
                (event::KeyCode::Char('l'), "Normal") => {
                    if (editor.cursor_col != cols - 1) && (editor.cursor_col < editor.line_max()) {
                        editor.cursor_col += 1;
                    }
                }
                (event::KeyCode::Char(':'), "Normal") => {
                    editor.mode = "Command";
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                    editor.cmd_message.insert(0, ":");

                    prev_cursor_col = editor.cursor_col;
                    prev_cursor_row = editor.cursor_row;

                    editor.cursor_row = rows - 1;
                    editor.cursor_col = 1;
                }
                (event::KeyCode::Char(c), "Command") => {
                    editor.cmd_message.insert_char(editor.cursor_col.into(), c);
                    editor.cursor_col += 1;
                }
                (event::KeyCode::Backspace, "Command") => {
                    if editor.cursor_col == 1 {
                        editor.mode = "Normal";
                        editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                        editor.cursor_col = prev_cursor_col;
                        editor.cursor_row = prev_cursor_row;
                        continue;
                    }
                    editor
                        .cmd_message
                        .remove((editor.cursor_col as usize - 1)..(editor.cursor_col as usize));
                    editor.cursor_col -= 1;
                }
                (event::KeyCode::Esc, "Command") => {
                    editor.mode = "Normal";
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());

                    editor.cursor_col = prev_cursor_col;
                    editor.cursor_row = prev_cursor_row;
                }
                (event::KeyCode::Enter, "Command") => {
                    let message = editor.cmd_message.to_string();
                    let words: Vec<&str> = message.split_whitespace().collect();

                    editor.cursor_col = prev_cursor_col;
                    editor.cursor_row = prev_cursor_row;

                    if words[0] == ":q" || words[0] == ":quit" {
                        if editor.dirty {
                            editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                            editor
                                .cmd_message
                                .insert(0, "Unsaved changes! Save file with :w or force quit :q!");
                            editor.mode = "Normal";
                        } else {
                            stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                            break;
                        }
                    } else if words[0] == ":q!" {
                        stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                        break;
                    } else if words[0] == ":w" || words[0] == ":write" {
                        if words.len() > 2 {
                            editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                            editor.cmd_message.insert(0, "Too many args for :write");
                        } else if words.len() == 2 {
                            editor.filename = Some(PathBuf::from(words[1]))
                        }

                        editor.mode = "Normal";
                        editor.save();
                    } else if words[0] == ":wq" {
                        if words.len() > 2 {
                            editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                            editor.cmd_message.insert(0, "Too many args for :wq");
                        } else if words.len() == 2 {
                            editor.filename = Some(PathBuf::from(words[1]))
                        }

                        if editor.save() {
                            stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
                            break;
                        } else {
                            editor.mode = "Normal";
                        }
                    } else {
                        editor.mode = "Normal";
                        let cmd = editor.cmd_message.to_string();

                        editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                        editor
                            .cmd_message
                            .insert(0, &format!("Unrecognized command {}", cmd));
                    }
                }
                (event::KeyCode::Esc, "Insert") => {
                    editor.mode = "Normal";
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('i'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('a'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cursor_col = (editor.cursor_col + 1).min(editor.line_max());
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('I'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cursor_col = 0;
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('A'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cursor_col = editor.line_max();
                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('o'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cursor_col = editor.line_max();
                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;
                    editor.dirty = true;
                    editor.text.insert_char(cursor_pos, '\n');
                    editor.cursor_row += 1;
                    editor.cursor_col = 0;

                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char('O'), "Normal") => {
                    editor.mode = "Insert";
                    editor.cursor_col = 0;
                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;
                    editor.dirty = true;
                    editor.text.insert_char(cursor_pos, '\n');
                    editor.cursor_col = 0;

                    editor.cmd_message.remove(0..editor.cmd_message.len_chars());
                }
                (event::KeyCode::Char(c), "Insert") => {
                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;
                    editor.dirty = true;
                    editor.text.insert_char(cursor_pos, c);
                    editor.cursor_col += 1;
                }
                (event::KeyCode::Backspace, "Insert") => {
                    if editor.cursor_col == 0 && editor.cursor_row == 0 {
                        continue;
                    }

                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;

                    if editor.cursor_col != 0 {
                        editor.cursor_col -= 1;
                    } else {
                        editor.cursor_row -= 1;
                        editor.cursor_col = editor.line_max();
                    }

                    editor.text.remove((cursor_pos - 1)..(cursor_pos));
                }
                (event::KeyCode::Enter, "Insert") => {
                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;
                    editor.dirty = true;
                    editor.text.insert_char(cursor_pos, '\n');
                    editor.cursor_row += 1;
                    editor.cursor_col = 0;
                }
                _ => {
                    if let Some(logs) = logs.as_mut() {
                        writeln!(logs, "Unknown key")?;
                    }
                }
            },
            event::Event::Resize(_, _) => (),
            _ => {
                break;
            }
        }
    }

    Ok(())
}

fn wrap_screen(logs: Option<File>, filename: Option<PathBuf>) -> std::io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(stdout(), terminal::EnterAlternateScreen)?;

    std::panic::set_hook(Box::new(|info| {
        let _ = execute!(stdout(), terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        eprintln!("Application panicked: {}", info);
    }));

    run(logs, filename)?;

    execute!(stdout(), terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    Ok(())
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    // If there "logs.txt" in cwd, write logs to it
    let logs = OpenOptions::new()
        .write(true)
        .append(true)
        .open("logs.txt")
        .ok();

    wrap_screen(logs, cli.filename)
}
