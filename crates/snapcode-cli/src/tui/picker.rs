// SPDX-License-Identifier: MIT
//! A searchable list overlay, for the fields whose lists are too long to cycle.

use super::fields::Field;

pub const AUTOMATIC: &str = "(automatic)";
pub const PAIRED: &str = "(paired)";

#[derive(Debug, Clone)]
pub struct Picker {
    pub field: Field,
    pub title: String,
    items: Vec<String>,
    query: String,
    matches: Vec<usize>,
    selected: usize,
}

impl Picker {
    pub fn new(
        field: Field,
        title: impl Into<String>,
        items: Vec<String>,
        current: Option<&str>,
    ) -> Self {
        let mut picker = Self {
            field,
            title: title.into(),
            items,
            query: String::new(),
            matches: Vec::new(),
            selected: 0,
        };
        picker.refilter();
        if let Some(current) = current {
            if let Some(at) = picker
                .matches
                .iter()
                .position(|i| picker.items[*i] == current)
            {
                picker.selected = at;
            }
        }
        picker
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    pub fn matches(&self) -> impl Iterator<Item = &str> {
        self.matches.iter().map(|i| self.items[*i].as_str())
    }

    pub fn current(&self) -> Option<&str> {
        self.matches
            .get(self.selected)
            .map(|i| self.items[*i].as_str())
    }

    pub fn push(&mut self, ch: char) {
        self.query.push(ch);
        self.refilter();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.matches.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.matches.len() as i32;
        self.selected = (self.selected as i32 + delta).rem_euclid(len) as usize;
    }

    fn refilter(&mut self) {
        let query = self.query.trim().to_ascii_lowercase();
        self.matches = if query.is_empty() {
            (0..self.items.len()).collect()
        } else {
            let mut prefix = Vec::new();
            let mut contains = Vec::new();
            for (index, item) in self.items.iter().enumerate() {
                let lower = item.to_ascii_lowercase();
                if lower.starts_with(&query) {
                    prefix.push(index);
                } else if lower.contains(&query) {
                    contains.push(index);
                }
            }
            prefix.extend(contains);
            prefix
        };
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
    }

    pub fn window(&self, height: usize) -> (usize, usize) {
        if height == 0 || self.matches.is_empty() {
            return (0, 0);
        }
        let start = self.selected.saturating_sub(height / 2);
        let start = start.min(self.matches.len().saturating_sub(height));
        (start, (start + height).min(self.matches.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages() -> Vec<String> {
        [
            "Swift",
            "Rust",
            "Python",
            "Ruby",
            "JavaScript",
            "Regular Expressions (Ruby)",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn picker() -> Picker {
        Picker::new(Field::Language, "Language", languages(), None)
    }

    #[test]
    fn starts_showing_everything() {
        let picker = picker();
        assert_eq!(picker.match_count(), 6);
        assert_eq!(picker.total_count(), 6);
        assert_eq!(picker.current(), Some("Swift"));
    }

    #[test]
    fn opens_on_the_current_value() {
        let picker = Picker::new(Field::Language, "Language", languages(), Some("Python"));
        assert_eq!(picker.current(), Some("Python"));
    }

    #[test]
    fn a_current_value_that_is_gone_does_not_break_anything() {
        let picker = Picker::new(Field::Language, "Language", languages(), Some("Deleted"));
        assert_eq!(
            picker.current(),
            Some("Swift"),
            "should fall back to the first"
        );
    }

    #[test]
    fn typing_narrows_the_list() {
        let mut picker = picker();
        picker.push('r');
        picker.push('u');
        let found: Vec<&str> = picker.matches().collect();
        assert_eq!(found, vec!["Rust", "Ruby", "Regular Expressions (Ruby)"]);
        assert_eq!(picker.current(), Some("Rust"));
    }

    #[test]
    fn filtering_ignores_case_and_surrounding_space() {
        let mut picker = picker();
        for ch in "SWI".chars() {
            picker.push(ch);
        }
        assert_eq!(picker.current(), Some("Swift"));
        assert_eq!(picker.match_count(), 1);
    }

    #[test]
    fn backspace_widens_the_list_again() {
        let mut picker = picker();
        picker.push('s');
        picker.push('w');
        assert_eq!(picker.match_count(), 1);
        picker.pop();
        assert!(picker.match_count() > 1);
        picker.pop();
        assert_eq!(picker.match_count(), 6);
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn a_query_that_matches_nothing_is_survivable() {
        let mut picker = picker();
        for ch in "zzzz".chars() {
            picker.push(ch);
        }
        assert_eq!(picker.match_count(), 0);
        assert_eq!(picker.current(), None);
        picker.move_selection(1);
        picker.move_selection(-1);
        assert_eq!(picker.selected_index(), 0);
        assert_eq!(picker.window(10), (0, 0));

        while !picker.query().is_empty() {
            picker.pop();
        }
        assert_eq!(picker.match_count(), 6, "backing out should recover");
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut picker = picker();
        picker.move_selection(-1);
        assert_eq!(picker.current(), Some("Regular Expressions (Ruby)"));
        picker.move_selection(1);
        assert_eq!(picker.current(), Some("Swift"));
    }

    #[test]
    fn narrowing_pulls_an_out_of_range_cursor_back() {
        let mut picker = picker();
        picker.move_selection(5);
        assert_eq!(picker.selected_index(), 5);
        picker.push('s');
        picker.push('w');
        assert!(picker.selected_index() < picker.match_count().max(1));
        assert_eq!(picker.current(), Some("Swift"));
    }

    #[test]
    fn the_window_follows_the_cursor() {
        let mut picker = picker();
        assert_eq!(picker.window(3).0, 0);

        picker.move_selection(-1);
        let (start, end) = picker.window(3);
        assert!(end <= picker.match_count());
        assert!(
            (start..end).contains(&picker.selected_index()),
            "cursor {} outside window {start}..{end}",
            picker.selected_index()
        );

        assert_eq!(picker.window(100), (0, 6));
    }

    #[test]
    fn an_automatic_entry_can_be_offered_and_chosen() {
        let mut items = vec![AUTOMATIC.to_string()];
        items.extend(languages());
        let picker = Picker::new(Field::Language, "Language", items, None);
        assert_eq!(picker.current(), Some(AUTOMATIC));
    }
}
