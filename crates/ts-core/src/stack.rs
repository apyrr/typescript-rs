pub struct Stack<T> {
    data: Vec<T>,
}

impl<T> Default for Stack<T> {
    fn default() -> Self {
        Self { data: Vec::new() }
    }
}

impl<T> Stack<T> {
    pub fn push(&mut self, item: T) {
        self.data.push(item);
    }

    pub fn pop(&mut self) -> T {
        self.data.pop().unwrap_or_else(|| panic!("stack is empty"))
    }

    pub fn peek(&self) -> &T {
        self.data.last().unwrap_or_else(|| panic!("stack is empty"))
    }

    pub fn peek_mut(&mut self) -> &mut T {
        self.data
            .last_mut()
            .unwrap_or_else(|| panic!("stack is empty"))
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
