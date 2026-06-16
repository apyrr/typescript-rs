// Arena allocator

pub struct Arena<T> {
    data: Vec<T>,
    chunks: Vec<Vec<T>>,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            chunks: Vec::new(),
        }
    }
}

impl<T> Arena<T>
where
    T: Default + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    // Allocate a single element in the arena and return a pointer to the element. If the arena is at capacity,
    // a new arena of the next size up is allocated.
    fn alloc(&mut self) -> &mut T {
        if self.data.len() == self.data.capacity() {
            let next_size = next_arena_size(self.data.len());
            self.grow_current(next_size);
        }
        self.data.push(T::default());
        self.data.last_mut().unwrap()
    }

    pub fn new_item(&mut self) -> &mut T {
        self.alloc()
    }

    pub fn new_value(&mut self) -> &mut T {
        self.new_item()
    }

    // Allocate a slice of the given size in the arena. If the requested size is beyond the capacity of the arena
    // and an arena of the next size up still wouldn't fit the slice, make a separate memory allocation for the slice.
    // Otherwise, grow the arena if necessary and allocate a slice out of it. The length and capacity of the resulting
    // slice are equal to the given size.
    pub fn new_slice(&mut self, size: usize) -> &mut [T] {
        if size == 0 {
            return &mut [];
        }
        if self.data.len() + size > self.data.capacity() {
            let next_size = next_arena_size(self.data.len());
            if size > next_size {
                self.chunks.push(vec![T::default(); size]);
                return self.chunks.last_mut().unwrap().as_mut_slice();
            } else {
                self.grow_current(next_size);
            }
        }
        let start = self.data.len();
        self.data.resize(start + size, T::default());
        &mut self.data[start..start + size]
    }

    pub fn new_slice1(&mut self, t: T) -> &mut [T] {
        let slice = self.new_slice(1);
        slice[0] = t;
        slice
    }

    pub fn clone(&mut self, t: &[T]) -> &mut [T] {
        if t.is_empty() {
            return &mut [];
        }
        let slice = self.new_slice(t.len());
        slice.clone_from_slice(t);
        slice
    }

    pub fn clone_slice(&mut self, t: &[T]) -> &mut [T] {
        self.clone(t)
    }

    fn grow_current(&mut self, next_size: usize) {
        let old = std::mem::replace(&mut self.data, Vec::with_capacity(next_size));
        if !old.is_empty() {
            self.chunks.push(old);
        }
    }
}

fn next_arena_size(size: usize) -> usize {
    // This compiles down branch-free.
    size.max(1).saturating_mul(2).min(256)
}
