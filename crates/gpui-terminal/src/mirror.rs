//! Validated parsed terminal state for a read-only view of a service-owned PTY.
use alacritty_terminal::grid::{Dimensions, Grid};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Cell;
use alacritty_terminal::term::color::{COUNT, Colors};
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{CursorShape, CursorStyle, Rgb};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::event::GpuiEventProxy;
use crate::terminal::TerminalState;

pub const MAX_COLUMNS: usize = 512;
pub const MAX_ROWS: usize = 256;
pub const MAX_HISTORY: usize = 10_000;
pub const MAX_CELLS: usize = 1_000_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorCursor {
    pub revision: u64,
    pub columns: usize,
    pub rows: usize,
    pub history_serial: u64,
    pub history_size: usize,
    pub mode: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CellRun {
    pub count: usize,
    pub cell: Cell,
}
pub type EncodedRow = Vec<CellRun>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorCursorStyle {
    pub shape: u8,
    pub blinking: bool,
}
impl MirrorCursorStyle {
    fn capture(style: CursorStyle) -> Self {
        Self {
            blinking: style.blinking,
            shape: match style.shape {
                CursorShape::Block => 0,
                CursorShape::Underline => 1,
                CursorShape::Beam => 2,
                CursorShape::HollowBlock => 3,
                CursorShape::Hidden => 4,
            },
        }
    }
    fn decode(&self) -> Result<CursorStyle> {
        let shape = match self.shape {
            0 => CursorShape::Block,
            1 => CursorShape::Underline,
            2 => CursorShape::Beam,
            3 => CursorShape::HollowBlock,
            4 => CursorShape::Hidden,
            _ => anyhow::bail!("Invalid cursor shape"),
        };
        Ok(CursorStyle {
            shape,
            blinking: self.blinking,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorPacket {
    pub cursor: MirrorCursor,
    pub full: bool,
    pub history_limit: usize,
    /// Full retained history, or the newest rows since the previous cursor.
    pub history: Vec<EncodedRow>,
    pub screen: Vec<EncodedRow>,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub cursor_template: Cell,
    pub cursor_style: MirrorCursorStyle,
    pub colors: Vec<Option<Rgb>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MirrorUpdate {
    pub packet: MirrorPacket,
    pub metadata: serde_json::Value,
    pub exited: bool,
    #[serde(default)]
    pub bell: bool,
    #[serde(default)]
    pub clipboard: Vec<String>,
}

pub fn bounded_history(columns: usize, rows: usize, requested: usize) -> usize {
    requested
        .min(MAX_HISTORY)
        .min((MAX_CELLS / columns.max(1)).saturating_sub(rows))
}

impl MirrorPacket {
    pub fn capture(
        term: &Term<GpuiEventProxy>,
        after: Option<&MirrorCursor>,
        revision: u64,
        history_limit: usize,
    ) -> Result<Self> {
        let grid = term.grid();
        Self::capture_grid(
            term,
            grid,
            after,
            revision,
            history_limit,
            term.mode().bits(),
        )
    }

    pub fn capture_primary(
        term: &Term<GpuiEventProxy>,
        revision: u64,
        history_limit: usize,
    ) -> Result<Self> {
        Self::capture_grid(
            term,
            term.primary_grid(),
            None,
            revision,
            history_limit,
            term.mode().bits() & !TermMode::ALT_SCREEN.bits(),
        )
    }

    fn capture_grid(
        term: &Term<GpuiEventProxy>,
        grid: &Grid<Cell>,
        after: Option<&MirrorCursor>,
        revision: u64,
        history_limit: usize,
        mode: u32,
    ) -> Result<Self> {
        let columns = grid.columns();
        let rows = grid.screen_lines();
        let history_size = grid.history_size();
        let history_serial = grid.history_serial();
        let appended = after.map_or(0, |after| {
            history_serial.saturating_sub(after.history_serial)
        });
        let full = after.is_none_or(|after| {
            after.columns != columns
                || after.rows != rows
                || (after.mode ^ mode) & TermMode::ALT_SCREEN.bits() != 0
                || after.history_serial > history_serial
                || after.revision > revision
                || appended > history_size as u64
                || history_size
                    != after
                        .history_size
                        .saturating_add(appended as usize)
                        .min(history_limit)
        });
        let history_count = if full {
            history_size
        } else {
            appended as usize
        };
        let history = (-(history_count as i32)..0)
            .map(|line| encode_row(grid, Line(line)))
            .collect();
        let screen = (0..rows as i32)
            .map(|line| encode_row(grid, Line(line)))
            .collect();
        let packet = Self {
            cursor: MirrorCursor {
                revision,
                columns,
                rows,
                history_serial,
                history_size,
                mode,
            },
            full,
            history_limit,
            history,
            screen,
            cursor_line: grid.cursor.point.line.0.max(0) as usize,
            cursor_column: grid.cursor.point.column.0,
            cursor_template: grid.cursor.template.clone(),
            cursor_style: MirrorCursorStyle::capture(term.cursor_style()),
            colors: (0..COUNT).map(|index| term.colors()[index]).collect(),
        };
        packet.validate()?;
        Ok(packet)
    }

    /// Prior output becomes scrollback for a fresh shell, without restoring
    /// the old application's modes, selection, clipboard or parser state.
    pub fn cold_history(&self, requested: usize) -> Result<Self> {
        self.validate()?;
        ensure!(self.full, "Cold history requires a full snapshot");
        let mut packet = self.clone();
        packet.history_limit =
            bounded_history(packet.cursor.columns, packet.cursor.rows, requested);
        packet.history.extend(packet.screen.clone());
        if packet.history.len() > packet.history_limit {
            packet
                .history
                .drain(..packet.history.len() - packet.history_limit);
        }
        packet.cursor.history_size = packet.history.len();
        packet.cursor.history_serial = packet.history.len() as u64;
        packet.cursor.mode = TermMode::default().bits();
        packet.cursor_line = 0;
        packet.cursor_column = 0;
        packet.cursor_template = Cell::default();
        packet.cursor_style = MirrorCursorStyle::capture(CursorStyle {
            shape: CursorShape::Block,
            blinking: true,
        });
        packet.screen = vec![
            vec![CellRun {
                count: packet.cursor.columns,
                cell: Cell::default()
            }];
            packet.cursor.rows
        ];
        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<()> {
        self.cursor_style.decode()?;
        let columns = self.cursor.columns;
        let rows = self.cursor.rows;
        ensure!(
            TermMode::from_bits(self.cursor.mode).is_some(),
            "Invalid terminal mode"
        );
        ensure!(
            (2..=MAX_COLUMNS).contains(&columns) && (1..=MAX_ROWS).contains(&rows),
            "Invalid terminal dimensions"
        );
        ensure!(
            self.history_limit <= MAX_HISTORY && (self.history_limit + rows) * columns <= MAX_CELLS,
            "Terminal history exceeds cell budget"
        );
        ensure!(
            self.cursor.history_size <= self.history_limit
                && self.history.len() <= self.cursor.history_size,
            "Invalid history size"
        );
        ensure!(
            !self.full || self.history.len() == self.cursor.history_size,
            "Incomplete history snapshot"
        );
        ensure!(
            self.screen.len() == rows && self.colors.len() == COUNT,
            "Incomplete render state"
        );
        ensure!(
            self.cursor_line < rows && self.cursor_column < columns,
            "Invalid terminal cursor"
        );
        for row in self.history.iter().chain(&self.screen) {
            ensure!(row.len() <= columns, "Too many cell runs");
            let mut length = 0usize;
            for run in row {
                ensure!(
                    run.count > 0 && run.count <= columns,
                    "Invalid cell run length"
                );
                length = length
                    .checked_add(run.count)
                    .ok_or_else(|| anyhow::anyhow!("Cell run overflow"))?;
            }
            ensure!(length == columns, "Cell runs do not cover the row");
        }
        Ok(())
    }

    /// Apply parsed state without replaying any terminal side effects or bytes.
    pub fn apply(&self, state: &mut TerminalState) -> Result<()> {
        self.validate()?;
        if self.full {
            state.resize(self.cursor.columns, self.cursor.rows);
        }
        ensure!(
            state.with_term(|term| term.columns() == self.cursor.columns
                && term.screen_lines() == self.cursor.rows),
            "A resize requires a full snapshot"
        );
        let columns = self.cursor.columns;
        let rows = self.cursor.rows;
        let style = self.cursor_style.decode()?;
        state.with_term_mut(|term| {
            if self.full {
                *term.grid_mut() = Grid::new(rows, columns, self.history_limit);
                term.selection = None;
            } else {
                let region = Line(0)..Line(rows as i32);
                let count = self.history.len();
                if count > 0 {
                    term.selection = term
                        .selection
                        .take()
                        .and_then(|selection| selection.rotate(term, &region, count as i32));
                }
            }
            grow_history(term.grid_mut(), self.history.len(), rows);
            let count = self.history.len() as i32;
            for (index, row) in self.history.iter().enumerate() {
                apply_row(term, Line(index as i32 - count), row);
            }
            for (index, row) in self.screen.iter().enumerate() {
                apply_row(term, Line(index as i32), row);
            }
            let cursor = &mut term.grid_mut().cursor;
            cursor.point = Point::new(Line(self.cursor_line as i32), Column(self.cursor_column));
            cursor.template = self.cursor_template.clone();
            let mut colors = Colors::default();
            for (index, color) in self.colors.iter().enumerate() {
                colors[index] = *color;
            }
            term.apply_render_state(TermMode::from_bits_retain(self.cursor.mode), colors, style);
        });
        Ok(())
    }
}

fn grow_history(grid: &mut Grid<Cell>, mut count: usize, rows: usize) {
    while count > 0 {
        let batch = count.min(rows);
        grid.scroll_up(&(Line(0)..Line(rows as i32)), batch);
        count -= batch;
    }
}

fn encode_row(grid: &Grid<Cell>, line: Line) -> EncodedRow {
    let mut runs: EncodedRow = Vec::new();
    for column in 0..grid.columns() {
        let cell = &grid[Point::new(line, Column(column))];
        if let Some(last) = runs.last_mut().filter(|last| last.cell == *cell) {
            last.count += 1;
        } else {
            runs.push(CellRun {
                count: 1,
                cell: cell.clone(),
            });
        }
    }
    runs
}

fn apply_row(term: &mut Term<GpuiEventProxy>, line: Line, runs: &EncodedRow) {
    let mut column = 0;
    for run in runs {
        for _ in 0..run.count {
            term.grid_mut()[Point::new(line, Column(column))] = run.cell.clone();
            column += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn cold_history_keeps_primary_output_and_resets_application_modes() {
        let (sender, _receiver) = mpsc::channel();
        let mut source = TerminalState::with_scrollback(20, 4, 12, GpuiEventProxy::new(sender));
        source.process_bytes(
            b"\x1b[38;2;17;34;51mPRIMARY_HISTORY\r\n\x1b[?1049hALTERNATE_ONLY\x1b[?2004h",
        );
        let packet = source
            .with_term(|term| MirrorPacket::capture_primary(term, 1, 12))
            .unwrap();
        let cold = packet.cold_history(12).unwrap();
        let (sender, _receiver) = mpsc::channel();
        let mut restored = TerminalState::with_scrollback(20, 4, 12, GpuiEventProxy::new(sender));
        cold.apply(&mut restored).unwrap();
        restored.with_term(|term| {
            let text = term
                .grid()
                .display_iter()
                .map(|cell| cell.cell.c)
                .collect::<String>();
            assert!(!text.contains("ALTERNATE_ONLY"));
            assert!(
                !term
                    .mode()
                    .intersects(TermMode::ALT_SCREEN | TermMode::BRACKETED_PASTE)
            );
            assert!(term.grid().history_size() > 0);
            let mut history = String::new();
            for line in -(term.grid().history_size() as i32)..0 {
                for column in 0..20 {
                    history.push(term.grid()[Point::new(Line(line), Column(column))].c);
                }
            }
            assert!(history.contains("PRIMARY_HISTORY"));
            assert!(!history.contains("ALTERNATE_ONLY"));
        });
        restored.process_bytes(b"FRESH_SHELL");
        assert_eq!(
            restored.with_term(|term| term.grid()[Point::new(Line(0), Column(0))].c),
            'F'
        );
    }
    fn terminal() -> TerminalState {
        let (tx, _) = mpsc::channel();
        TerminalState::with_scrollback(12, 3, 4, GpuiEventProxy::new(tx))
    }
    fn packet(state: &TerminalState, after: Option<&MirrorCursor>, revision: u64) -> MirrorPacket {
        state.with_term(|term| MirrorPacket::capture(term, after, revision, 4).unwrap())
    }
    fn assert_same(source: &TerminalState, mirror: &TerminalState) {
        let left = serde_json::to_value(packet(source, None, 0)).unwrap();
        let right = serde_json::to_value(packet(mirror, None, 0)).unwrap();
        for key in [
            "history",
            "screen",
            "cursor_line",
            "cursor_column",
            "cursor_template",
            "cursor_style",
            "colors",
        ] {
            assert_eq!(left[key], right[key], "{key}");
        }
        assert_eq!(source.mode(), mirror.mode());
    }
    #[test]
    fn mirrors_unicode_styles_and_fragmented_sequences_without_replay() {
        let mut source = terminal();
        source.process_bytes(b"\x1b[38;2;12;34;56mred\x1b[0m ");
        source.process_bytes("界e\u{301}".as_bytes());
        source.process_bytes(b"\x1b[?2004h\x1b[6 q\x1b]4;1;rgb:ab/cd/ef\x07\x1b[");
        let initial = packet(&source, None, 1);
        let mut mirror = terminal();
        let encoded = serde_json::to_vec(&initial).unwrap();
        let initial: MirrorPacket = serde_json::from_slice(&encoded).unwrap();
        initial.apply(&mut mirror).unwrap();
        source.process_bytes(b"32mgreen");
        packet(&source, Some(&initial.cursor), 2)
            .apply(&mut mirror)
            .unwrap();
        assert_same(&source, &mirror);
    }
    #[test]
    fn appends_history_at_capacity_and_rebases_after_alternate_screen_and_resize() {
        let mut source = terminal();
        let mut mirror = terminal();
        source.process_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\n");
        let initial = packet(&source, None, 1);
        initial.apply(&mut mirror).unwrap();
        assert_eq!(initial.history.len(), 4);
        source.process_bytes(b"eight\r\n");
        let delta = packet(&source, Some(&initial.cursor), 2);
        assert!(!delta.full);
        assert_eq!(delta.history.len(), 1);
        delta.apply(&mut mirror).unwrap();
        assert_same(&source, &mirror);
        source.process_bytes(b"\x1b[?1049h\x1b[2Jalternate");
        let alternate = packet(&source, Some(&delta.cursor), 3);
        assert!(alternate.full);
        alternate.apply(&mut mirror).unwrap();
        assert_same(&source, &mirror);
        source.process_bytes(b"\x1b[?1049l");
        source.resize(10, 4);
        let resized = packet(&source, Some(&alternate.cursor), 4);
        assert!(resized.full);
        resized.apply(&mut mirror).unwrap();
        assert_same(&source, &mirror);
    }
    #[test]
    fn rejects_malformed_packets_before_touching_a_grid() {
        let source = terminal();
        let valid = packet(&source, None, 1);
        let mut mirror = terminal();
        for mutate in [
            |p: &mut MirrorPacket| p.cursor.columns = 0,
            |p: &mut MirrorPacket| p.cursor_column = p.cursor.columns,
            |p: &mut MirrorPacket| p.history_limit = usize::MAX,
            |p: &mut MirrorPacket| p.screen[0][0].count = usize::MAX,
            |p: &mut MirrorPacket| {
                p.screen.pop();
            },
        ] {
            let mut invalid = valid.clone();
            mutate(&mut invalid);
            assert!(invalid.apply(&mut mirror).is_err());
        }
    }
}
