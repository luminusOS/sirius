//! Pure navigation state over the resolved page list. Knows nothing about GTK.

/// Tracks the current position in the resolved page sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Navigator {
    pages: Vec<String>,
    index: usize,
}

impl Navigator {
    /// Build from the resolved page order (from `PagesConfig::resolve()`).
    /// Panics only if `pages` is empty — the resolver always yields mandatory pages.
    pub fn new(pages: Vec<String>) -> Self {
        assert!(!pages.is_empty(), "navigator requires at least one page");
        Self { pages, index: 0 }
    }

    pub fn current(&self) -> &str {
        &self.pages[self.index]
    }

    pub fn is_first(&self) -> bool {
        self.index == 0
    }

    pub fn is_last(&self) -> bool {
        self.index + 1 == self.pages.len()
    }

    /// Advance one page if not already at the end. Returns the new current page.
    pub fn next(&mut self) -> &str {
        if !self.is_last() {
            self.index += 1;
        }
        self.current()
    }

    /// Go back one page if not at the start. Returns the new current page.
    pub fn prev(&mut self) -> &str {
        if !self.is_first() {
            self.index -= 1;
        }
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nav() -> Navigator {
        Navigator::new(vec!["welcome".into(), "disk".into(), "finished".into()])
    }

    #[test]
    fn starts_at_first() {
        let n = nav();
        assert_eq!(n.current(), "welcome");
        assert!(n.is_first());
        assert!(!n.is_last());
    }

    #[test]
    fn next_advances_and_clamps() {
        let mut n = nav();
        assert_eq!(n.next(), "disk");
        assert_eq!(n.next(), "finished");
        assert!(n.is_last());
        assert_eq!(n.next(), "finished"); // clamped
    }

    #[test]
    fn prev_goes_back_and_clamps() {
        let mut n = nav();
        n.next();
        assert_eq!(n.prev(), "welcome");
        assert_eq!(n.prev(), "welcome"); // clamped
    }
}
