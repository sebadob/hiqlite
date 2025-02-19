use std::{borrow::Cow, collections::{hash_map::Entry, HashMap}};

use rusqlite::{types::Value, Transaction};

use crate::Error;

/// A previously executed statement
pub struct ExecutedStatement {
    /// The executed SQL
    pub sql: Cow<'static, str>,
    /// The first returned row of the statement.
    /// This will be empty if the statement returned no rows.
    pub first_row: Vec<Value>,
}

impl ExecutedStatement {
    fn by_index(statements: &[ExecutedStatement], index: usize) -> Result<&ExecutedStatement, Cow<'static, str>> {
        Ok(statements.get(index).ok_or("statement index out of bounds")?)
    }

    fn get_first_row_value(&self, column_index: usize) -> Result<&Value, Cow<'static, str>> {
        Ok(self.first_row.get(column_index).ok_or("column index out of bounds")?)
    }
}

/// Data structure for supporting Param::StmtOutput
pub struct TransactionEnv {
    /// already executed statements
    executed_stmts: Vec<ExecutedStatement>,

    /// A cache of column indexes per statement
    column_index_cache: HashMap<usize, HashMap<Cow<'static, str>, usize>>
}

impl TransactionEnv {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            executed_stmts: Vec::with_capacity(cap),
            column_index_cache: Default::default(),
        }
    }

    pub fn push(&mut self, executed_stmt: ExecutedStatement) {
        self.executed_stmts.push(executed_stmt);
    }
}

/// A context for looking up Param variables
pub struct TransactionParamContext<'a, 't> {
    pub txn: &'a Transaction<'t>,
    pub env: &'a mut TransactionEnv,
}

impl TransactionParamContext<'_, '_> {
    pub fn lookup_statement_output_indexed(&mut self, statement_index: usize, column_index: usize) -> Result<Value, Cow<'static, str>> {
        let executed_stmt = ExecutedStatement::by_index(&self.env.executed_stmts, statement_index)?;
        Ok(executed_stmt.get_first_row_value(column_index)?.clone())
    }

    pub fn lookup_statement_output_named(&mut self, statement_index: usize, column_name: Cow<'static, str>) -> Result<Value, Cow<'static, str>> {
        let executed_stmt = ExecutedStatement::by_index(&self.env.executed_stmts, statement_index)?;

        let cache = self.env.column_index_cache.entry(statement_index).or_default();
        let column_index = match cache.entry(column_name) {
            Entry::Occupied(occpied) => *occpied.get(),
            Entry::Vacant(vacant) => {
                // Need to re-prepare the statement.
                // Hopefully the statement will be cached.
                // The statement has already been prepared (and error-checked) earlier, so should not fail.
                let stmt = self.txn.prepare_cached(&executed_stmt.sql)
                    .map_err(|_| "re-preparation")?;

                let column_index = stmt.column_index(vacant.key())
                    .map_err(|err| format!("{err:?}"))?;

                // Cache the column index for later, in case the same value
                // is used many times in the same transaction, which is not unlikely.
                // This should avoid some statement re-preparation in larger transactions.
                *vacant.insert(column_index)
            },
        };

        Ok(executed_stmt.get_first_row_value(column_index)?.clone())
    }
}
