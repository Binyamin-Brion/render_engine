/// Calculates the next index into an array such that every index is visited in an infinite loop
#[derive(Copy, Clone, PartialEq)]
pub struct ArrayIndexer<const N: usize>
{
    index: usize,
}

impl<const N: usize> ArrayIndexer<N>
{
    /// Creates a new indexer starting at the given index
    ///
    /// `starting_index` - the index the indexer should start at
    pub fn new(starting_index: usize) -> ArrayIndexer<N>
    {
        debug_assert!(starting_index < N);
        ArrayIndexer{ index: starting_index }
    }

    /// The current index to use when accessing an array in a round robin fashion
    pub fn index(&self) -> usize
    {
        self.index
    }

    /// Increment the index to get the next index to use when accessing an array with round robin
    pub fn increment(&self) -> ArrayIndexer<N>
    {
        ArrayIndexer{ index: (self.index + 1) % N }
    }
}
