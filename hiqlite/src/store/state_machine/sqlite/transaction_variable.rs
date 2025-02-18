/// A 0-based reference to a transaction statement
#[derive(Clone, Copy)]
pub struct StmtIndex(pub usize);

impl StmtIndex {
    /// Specify a column on the StmtIndex to produce a [StmtColumn].
    pub fn column<C>(self, column: C) -> StmtColumn<C> {
        StmtColumn { stmt_index: self, column }
    }
}

/// A reference to a column produced by a statement in a transaction.
#[derive(Clone, Copy)]
pub struct StmtColumn<C> {
    pub(crate) stmt_index: StmtIndex,
    pub(crate) column: C,
}
