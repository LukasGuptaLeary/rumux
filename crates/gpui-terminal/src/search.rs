//! Search the terminal grid, including wrapped lines and scrollback.
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Point};
use alacritty_terminal::term::{
    Term,
    search::{Match, RegexIter, RegexSearch},
};

const MAX_MATCHES: usize = 10_000;

pub fn find_matches<T>(term: &Term<T>, query: &str) -> Vec<Match> {
    if query.is_empty() {
        return Vec::new();
    }
    // Search is literal. Alacritty provides smart case and correct wide/wrapped
    // cell coordinates; regex punctuation in shell output must stay literal.
    let escaped: String = query
        .chars()
        .flat_map(|ch| {
            let special = matches!(
                ch,
                '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\'
            );
            special
                .then_some('\\')
                .into_iter()
                .chain(std::iter::once(ch))
        })
        .collect();
    let Ok(mut regex) = RegexSearch::new(&escaped) else {
        return Vec::new();
    };
    let start = Point::new(term.topmost_line(), Column(0));
    let end = Point::new(term.bottommost_line(), term.last_column());
    RegexIter::new(start, end, Direction::Right, term, &mut regex)
        .take(MAX_MATCHES)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GpuiEventProxy, TerminalState};

    fn terminal(cols: usize, rows: usize, history: usize) -> TerminalState {
        let (tx, _) = std::sync::mpsc::channel();
        TerminalState::with_scrollback(cols, rows, history, GpuiEventProxy::new(tx))
    }

    #[test]
    fn searches_scrollback_and_regex_punctuation_literally() {
        let mut state = terminal(40, 2, 20);
        state.process_bytes(b"first [a].*\r\nsecond [a].*\r\nthird\r\nfourth");
        state.with_term(|term| {
            let matches = find_matches(term, "[a].*");
            assert_eq!(matches.len(), 2);
            assert!(matches[0].start().line.0 < 0);
            assert_eq!(matches[0].start().column.0, 6);
            assert!(find_matches(term, "absent").is_empty());
            assert!(find_matches(term, "").is_empty());
        });
    }

    #[test]
    fn search_understands_soft_wraps_and_wide_unicode() {
        let mut state = terminal(8, 4, 20);
        state.process_bytes("123456跨界abc".as_bytes());
        state.with_term(|term| {
            let matches = find_matches(term, "跨界abc");
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].start().column.0, 6);
            assert!(matches[0].end().line > matches[0].start().line);
        });
    }

    #[test]
    fn configured_history_bounds_scrollback_and_find() {
        let mut state = terminal(20, 2, 3);
        for i in 0..30 {
            state.process_bytes(format!("line {i}\r\n").as_bytes());
        }
        state.with_term_mut(|term| {
            assert_eq!(term.grid().history_size(), 3);
            term.scroll_display(alacritty_terminal::grid::Scroll::Top);
            assert_eq!(term.grid().display_offset(), 3);
            assert!(find_matches(term, "line 0").is_empty());
            term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
            assert_eq!(term.grid().display_offset(), 0);
        });
    }
}
