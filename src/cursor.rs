pub trait Cursor<T> {
    fn source(&self) -> &[T];
    fn current(&self) -> usize;
    fn current_mut(&mut self) -> &mut usize;
    fn is_at_end(&self) -> bool;

    fn peek(&self) -> &T {
        &self.source()[self.current()]
    }

    fn peek_next(&self) -> Option<&T> {
        let next = self.current() + 1;
        if next >= self.source().len() {
            None
        } else {
            Some(&self.source()[next])
        }
    }

    fn previous(&self) -> &T {
        &self.source()[self.current() - 1]
    }

    fn advance(&mut self) -> &T {
        if !self.is_at_end() {
            *self.current_mut() += 1;
        }
        self.previous()
    }
}
