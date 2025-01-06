use ropey::Rope;
use std::fs::{File, OpenOptions};
use std::io::{stdout, BufReader, BufWriter, Write};
use std::path::PathBuf;

use crossterm::{cursor, event, execute, style, terminal, ExecutableCommand};

use clap::Parser;

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
    fn save(&self) {
        if let Some(&ref pathbuf) = self.filename.as_ref() {
            self.text
                .write_to(BufWriter::new(File::create(pathbuf).unwrap()))
                .unwrap();
        }
    }
}

fn run(mut logs: Option<File>, filename: Option<PathBuf>) -> std::io::Result<()> {
    let text;
    if let Some(&ref pathbuf) = filename.as_ref() {
        text = Rope::from_reader(BufReader::new(File::open(pathbuf)?))?;
    } else {
        text = Rope::new();
    }

    let mut editor = Editor {
        text,
        filename,
        cursor_col: 0,
        cursor_row: 0,
        shift_row: 0,
    };

    let command_view_rows = 2;
    let mut mode = "Normal";

    let mut prefered_col: Option<u16> = None;
    let mut cmd_message = String::new();

    loop {
        let (cols, rows) = terminal::size()?;

        if let Some(logs) = logs.as_mut() {
            writeln!(logs, "Size ({} x {})", cols, rows)?;
        }

        // Render
        stdout()
            .execute(terminal::Clear(terminal::ClearType::All))?
            .execute(cursor::MoveTo(0, 0))?
            .execute(style::SetForegroundColor(style::Color::Blue))?
            .execute(style::ResetColor)?;

        for (line, i) in editor
            .text
            .lines_at(editor.shift_row)
            .zip(0..(rows - command_view_rows).min(editor.text.len_lines() as u16))
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

        stdout().execute(cursor::MoveTo(0, rows - 2))?;
        stdout().execute(style::Print(format!("{}\r\n{}", mode, cmd_message)))?;
        stdout().execute(cursor::MoveTo(editor.cursor_col, editor.cursor_row))?;

        if mode == "Insert" {
            stdout().execute(cursor::SetCursorStyle::SteadyBar)?;
        } else {
            stdout().execute(cursor::SetCursorStyle::SteadyBlock)?;
        }

        // Events
        let ev = event::read()?;

        if let Some(logs) = logs.as_mut() {
            writeln!(logs, "Got event {:?}", ev)?;
            writeln!(logs, "shift_row {}", editor.shift_row)?;
            writeln!(logs, "text len lines {}", editor.text.len_lines())?;
        }

        match ev {
            event::Event::Key(keyev) => match (keyev.code, mode) {
                (event::KeyCode::Char('q'), "Normal") => {
                    break;
                }
                (event::KeyCode::Char('w'), "Normal") => {
                    if editor.filename.is_some() {
                        editor.save();
                        cmd_message = format!("{:?} written", editor.filename.as_ref().unwrap());
                    }
                }
                (event::KeyCode::Char('h'), "Normal") => {
                    prefered_col = None;

                    if editor.cursor_col != 0 {
                        editor.cursor_col -= 1;
                    }
                }
                (event::KeyCode::Char('j'), "Normal") => {
                    if let None = prefered_col {
                        prefered_col = Some(editor.cursor_col);
                    }

                    if (editor.cursor_row != rows - 1 - command_view_rows)
                        && (((editor.cursor_row + 1) as usize) < editor.text.len_lines())
                    {
                        editor.cursor_row += 1;
                    } else {
                        if (editor.shift_row + rows as usize - command_view_rows as usize)
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
                    prefered_col = None;

                    if (editor.cursor_col != cols - 1) && (editor.cursor_col < editor.line_max()) {
                        editor.cursor_col += 1;
                    }
                }
                (event::KeyCode::Esc, "Insert") => {
                    mode = "Normal";
                    cmd_message = "".to_string();
                }
                (event::KeyCode::Char('i'), "Normal") => {
                    mode = "Insert";
                    cmd_message = "".to_string();
                }
                (event::KeyCode::Char(c), "Insert") => {
                    let cursor_pos = editor
                        .text
                        .line_to_char(editor.cursor_row as usize + editor.shift_row)
                        + editor.cursor_col as usize;
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
