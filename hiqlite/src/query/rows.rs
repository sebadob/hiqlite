use crate::Error;
use bincode_next::{Decode, Encode};
use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use rusqlite::types::FromSql;
use rusqlite::{Error as SqliteError, Row as SqliteRow, Statement};
use rustc_hash::FxHashMap;
use serde::ser::{SerializeMap as _, Serializer};
use serde::{Deserialize, Serialize};
use std::any::{Any, TypeId};
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::error;

/// A column name -> index map of a prepared statement, built once per query.
///
/// rusqlite resolves a column name with a linear scan that asks SQLite for the name of every
/// candidate column, so a `Row::get("name")` costs one C call per column of the result set, for
/// every single row. A struct with N fields over a result set with N columns therefore does
/// N * (N + 1) / 2 of those calls per row.  Building the map once per query turns every later
/// lookup into a hash probe over names we already fetched.
#[derive(Debug)]
pub struct ColIdx {
    map: FxHashMap<Arc<str>, u16>,
    /// The column names in result-set order, the header that keys an owned row when it is
    /// serialized, and the only way back from the map to the order of the result set.  Empty on a
    /// `lookup_only` index, whose rows are never handed over.
    names: Vec<Arc<str>>,
}

impl ColIdx {
    /// The index of a statement whose rows are handed over: the map, and the header of the result
    /// set next to it.
    pub(crate) fn from_stmt(stmt: &Statement) -> Self {
        let count = stmt.column_count();
        let mut map = FxHashMap::default();
        // The result set tells us its column count upfront, so the map is reserved once and never
        // has to grow while we fill it.
        map.reserve(count);
        let mut names = Vec::with_capacity(count);
        // `entry().or_insert()` keeps the lowest index, so a query with duplicate column names,
        // like a join over two tables sharing one, maps to the first column exactly like
        // rusqlite's own scan does.  A name is one `Arc<str>` shared by its map key and its slot
        // in the order, so a query pays for its header exactly once per column.
        for (idx, name) in stmt.column_names().enumerate() {
            let name: Arc<str> = Arc::from(name);
            map.entry(name.clone()).or_insert(idx as u16);
            names.push(name);
        }
        Self { map, names }
    }

    /// The index of a statement whose rows are only ever read by name: the map, and no header.
    ///
    /// A `Setup::Named` mapping builds a row, maps it, and drops it - no caller of that path ever
    /// holds a row, so nothing reads its result-set order, and the query pays for its column names
    /// exactly once: in the map.  A row built on this index has an empty [`RowOwned::columns`],
    /// which is why only the mapping path may use it.
    pub(crate) fn lookup_only(stmt: &Statement) -> Self {
        let count = stmt.column_count();
        let mut map = FxHashMap::default();
        map.reserve(count);
        for (idx, name) in stmt.column_names().enumerate() {
            map.entry(Arc::from(name)).or_insert(idx as u16);
        }
        Self {
            map,
            names: Vec::new(),
        }
    }

    /// The same index over the column names of a result set that was read into owned rows.
    pub(crate) fn from_names(columns: &[String]) -> Self {
        let mut map = FxHashMap::default();
        map.reserve(columns.len());
        let names = columns
            .iter()
            .map(|name| Arc::from(name.as_str()))
            .collect::<Vec<Arc<str>>>();
        for (idx, name) in names.iter().enumerate() {
            map.entry(name.clone()).or_insert(idx as u16);
        }
        Self { map, names }
    }

    /// An index with no columns, for rows whose field names were resolved to positions upfront.
    ///
    /// Only the name lookups of a `Row` touch this map, and the hoisted path does all its reads by
    /// index, so it stays empty and a map of it never allocates.
    pub(crate) fn empty() -> Self {
        Self {
            map: FxHashMap::default(),
            names: Vec::new(),
        }
    }

    /// The column names in result-set order.
    pub(crate) fn names(&self) -> &[Arc<str>] {
        &self.names
    }

    /// The index a column name resolves to, an exact match like the owned rows always did.
    fn index(&self, idx: &str) -> Option<usize> {
        self.map.get(idx).map(|i| usize::from(*i))
    }

    /// Reads a column of a borrowed row, resolving its name through the map.
    ///
    /// A name that is not in the map goes through rusqlite's own resolution, which matches
    /// column names case-insensitively, so that behaviour is preserved.
    fn borrowed_get<T: FromSql>(row: &SqliteRow, cols: &Self, idx: &str) -> Result<T, SqliteError> {
        match cols.map.get(idx) {
            Some(i) => row.get(*i as usize),
            None => row.get(idx),
        }
    }
}

/// One column of a result set, resolved by name before the first row is read.
///
/// The indexed mapping path resolves every field name of a mapped type once per query and hands
/// the result to every row, so a row never looks a name up again.  A `ColIndex` can only be
/// created by that resolution, which keeps a hand-written, order-dependent index out of reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColIndex(u16);

impl ColIndex {
    /// The state of a field that no column matched yet.
    const UNRESOLVED: Self = Self(u16::MAX);

    #[inline]
    fn pos(self) -> usize {
        usize::from(self.0)
    }

    /// A column position of a result set as an index; results never have more than `u16::MAX`
    /// columns in practice, and a wider one clamps to the unresolved marker.
    #[inline]
    fn to_pos(pos: usize) -> u16 {
        u16::try_from(pos).unwrap_or(Self::UNRESOLVED.0)
    }

    #[inline]
    fn from_pos(pos: usize) -> Self {
        Self(Self::to_pos(pos))
    }
}

/// The field names of one mapped type, sorted by name, each with the slot it reads.
///
/// The `FromRow` derive emits this as a `static` slice of sorted pairs, and the query resolves it
/// into column positions once per statement; the resolution only ever binary searches it.
pub struct FieldNames {
    fields: &'static [(&'static str, u16)],
}

impl FieldNames {
    /// Builds a segment from its sorted field list; `fields` must be sorted by name.
    #[must_use]
    pub const fn new(fields: &'static [(&'static str, u16)]) -> Self {
        Self { fields }
    }

    /// The slot of a field name, exact match.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<u16> {
        self.fields
            .binary_search_by_key(&name, |(field, _)| *field)
            .ok()
            .map(|pos| self.fields[pos].1)
    }

    /// Every field of this segment, in name order.
    #[must_use]
    pub const fn entries(&self) -> &'static [(&'static str, u16)] {
        self.fields
    }

    /// The number of fields of this segment.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether this segment maps no field at all; a mapped type always has one.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// The field names of a mapped type as compile-time field segments, each with the slot offset its
/// fields start at.  A type without flattened fields has exactly one segment.
pub type FieldMaps<'a> = Vec<(&'a FieldNames, u16)>;

/// The compile-time fingerprint of a set of column names.
///
/// Order and case do not change the value, and the parts of a flattened type merge by wrapping
/// addition - a plain `+` of two of these values overflows in most real structs, and a debug build
/// would stop the compile over it - which is what lets a nested type contribute its own names.  The
/// `FromRow` derive computes it for the field names of a type and `sql_map!()` for the columns a
/// query returns; the check is a `const` comparison of the two.  It is a fingerprint for the compiler,
/// not data for storage: a collision is a missed check, never a wrong row.  The derive publishes a
/// shape by calling this very function, so there is a single algorithm, and its value is pinned to a
/// literal in the tests of this module.
#[must_use]
pub const fn sql_shape(names: &[&str]) -> u64 {
    let mut shape = (names.len() as u64) << 48;
    let mut i = 0;
    while i < names.len() {
        shape = shape.wrapping_add(shape_of_name(names[i]));
        i += 1;
    }
    shape
}

/// The FNV-1a hash of one column name, lowercased to match the case-insensitive name lookup a row
/// falls back to.
const fn shape_of_name(name: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i].to_ascii_lowercase() as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
        i += 1;
    }
    hash
}

/// The column names a mapped type reads, published as a fingerprint for the `sql_map!()` check.
///
/// The `FromRow` derive implements it for every type it maps, and a `flatten` field contributes the
/// shape of its own type; the `TryFromRow` derive does not, because `query_try_map()` reports a
/// column mismatch as an error at runtime.  A hand-written [`FromRowIndexed`] impl, any type mapped
/// only through `TryFromRow`, and any type whose row mapping you write yourself, add it with the
/// `SqlShape` derive of `hiqlite-derive`, or by listing the column names the mapping reads.  The
/// derives emit it in debug builds, and in a release build only when `sql-check-release` is on: a
/// plain release build compares nothing.
///
/// ```
/// # use hiqlite::{sql_shape, SqlShape};
/// struct Point {
///     x: i64,
///     y: i64,
/// }
///
/// impl SqlShape for Point {
///     const SHAPE: u64 = sql_shape(&["x", "y"]);
/// }
/// ```
pub trait SqlShape {
    /// The fingerprint of the column names this type reads; build it with [`sql_shape`].
    const SHAPE: u64;
}

/// A mapped type that builds its rows from indices that were resolved once per query.
///
/// `query_map()`, `query_map_one()` and `query_map_optional()` of the [`Client`](crate::Client)
/// map a type through this path once the type is registered, which the `FromRow` derive does from
/// the `From<&mut Row>` impl it emits; every other type is mapped by the column names its
/// `From<&mut Row>` impl reads.  The result set is resolved before the first row, so no row ever
/// looks a name up again.
///
/// A hand-written impl registers its type with [`register_hoisted`], which needs `Self: 'static`.
pub trait FromRowIndexed: Sized {
    /// The number of column slots this type reads, every flattened part included: the total of the
    /// segments [`field_maps`](Self::field_maps) describes, because the mapping below reads one
    /// column per slot of it.
    const FIELD_COUNT: usize;

    /// The field names of this type, and the slot offset each segment starts at.  Their total is
    /// [`FIELD_COUNT`](Self::FIELD_COUNT).
    fn field_maps() -> FieldMaps<'static>;

    /// Maps the current row from the resolved indices.
    ///
    /// The row is handed to you with an index that has no column names, because its names were
    /// resolved upfront; read every column through the indices you are given.
    ///
    /// # Panics
    ///
    /// If the value of a column cannot be converted into the declared type.
    fn from_row_indices(row: &mut Row<'_>, idx: &[ColIndex]) -> Self;
}

/// The non-panicking counterpart of [`FromRowIndexed`], which `query_try_map()`,
/// `query_try_map_one()` and `query_try_map_optional()` pick the same way; the `TryFromRow` derive
/// implements and registers it, and a hand-written impl registers with [`register_hoisted_try`].
pub trait TryFromRowIndexed: Sized {
    /// The number of column slots this type reads, every flattened part included: the total of the
    /// segments [`field_maps`](Self::field_maps) describes, because the mapping below reads one
    /// column per slot of it.
    const FIELD_COUNT: usize;

    /// The field names of this type, and the slot offset each segment starts at.  Their total is
    /// [`FIELD_COUNT`](Self::FIELD_COUNT).
    fn field_maps() -> FieldMaps<'static>;

    /// Maps the current row from the resolved indices.
    ///
    /// The row is handed to you with an index that has no column names, because its names were
    /// resolved upfront; read every column through the indices you are given.
    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that cannot be converted into the declared type; on a borrowed row
    /// that is the [`Error::Sqlite`] of `rusqlite`.
    fn try_from_row_indices(row: &mut Row<'_>, idx: &[ColIndex]) -> Result<Self, Error>;
}

impl ColIdx {
    /// The field index of a mapped type over a prepared statement, resolved once per query.
    pub(crate) fn resolve_indexed(
        maps: &FieldMaps<'_>,
        stmt: &Statement,
    ) -> Result<Vec<ColIndex>, String> {
        let mut idx = Self::new_index(maps);
        for pos in 0..stmt.column_count() {
            let name = stmt.column_name(pos).map_err(|e| e.to_string())?;
            Self::fill_index(maps, name, ColIndex::to_pos(pos), &mut idx);
        }
        Self::resolve_fallback(maps, &mut idx, |name| stmt.column_index(name).ok())?;
        Ok(idx)
    }

    /// The same index over a column index of a result set that travelled through the network layer.
    /// The exact matches come out of the map, and a name that only matches case-insensitively
    /// resolves to the lowest column that matches it, the column the scan above answers for that
    /// name.  Iterating the map is hash order, so the case-insensitive half picks by position:
    /// two columns whose names differ only in case are two keys of that map.
    pub(crate) fn resolve_indexed_owned(
        &self,
        maps: &FieldMaps<'_>,
    ) -> Result<Vec<ColIndex>, String> {
        let mut idx = Self::new_index(maps);
        for (name, pos) in &self.map {
            Self::fill_index(maps, name, *pos, &mut idx);
        }
        Self::resolve_fallback(maps, &mut idx, |name| {
            self.map
                .iter()
                .filter(|(col, _)| col.eq_ignore_ascii_case(name))
                .map(|(_, pos)| usize::from(*pos))
                .min()
        })?;
        Ok(idx)
    }

    /// The index vector that gives every field of a mapped type its slot.
    fn new_index(maps: &FieldMaps<'_>) -> Vec<ColIndex> {
        vec![ColIndex::UNRESOLVED; Self::slots(maps)]
    }

    /// The number of slots the segments of a mapped type describe: the highest slot a field of any
    /// of them can reach.
    pub(crate) fn slots(maps: &FieldMaps<'_>) -> usize {
        maps.iter()
            .map(|(map, off)| usize::from(*off) + map.len())
            .max()
            .unwrap_or(0)
    }

    /// Records `pos` for every segment whose field is exactly this column name, keeping the first
    /// column that matched it.  This is the exact half of the resolution; a field that no column
    /// matches exactly is resolved by [`ColIdx::resolve_fallback`], at the lowest matching position
    /// like rusqlite's own scan.  A field named in a case no column matches exactly, and a result
    /// set that holds two columns differing only in case, is where the two halves differ: the exact
    /// half takes the column spelled like the field, rusqlite's scan the first one it comes to.
    fn fill_index(maps: &FieldMaps<'_>, name: &str, pos: u16, idx: &mut [ColIndex]) {
        for (map, off) in maps {
            if let Some(local) = map.get(name) {
                let slot = usize::from(*off) + usize::from(local);
                if idx[slot] == ColIndex::UNRESOLVED {
                    idx[slot] = ColIndex(pos);
                }
            }
        }
    }

    /// Resolves every field that no column matched exactly through a case-insensitive lookup, which
    /// is the behaviour `Row::get("name")` always had.  A name nothing matches comes back as that
    /// name, and the mapping that asked for it decides what a missing column means.
    ///
    /// The fields are walked in slot order, the order a type declares them in, so the first field a
    /// result set is missing is the same one a name-based `From<&mut Row>` impl fails on first.
    fn resolve_fallback(
        maps: &FieldMaps<'_>,
        idx: &mut [ColIndex],
        mut lookup: impl FnMut(&str) -> Option<usize>,
    ) -> Result<(), String> {
        for (map, off) in maps {
            let off = usize::from(*off);
            if map
                .entries()
                .iter()
                .all(|(_, local)| idx[off + usize::from(*local)] != ColIndex::UNRESOLVED)
            {
                continue;
            }
            let mut fields = map.entries().to_vec();
            fields.sort_by_key(|(_, local)| *local);
            for (name, local) in fields {
                let slot = off + usize::from(local);
                if idx[slot] == ColIndex::UNRESOLVED {
                    match lookup(name) {
                        Some(pos) => idx[slot] = ColIndex::from_pos(pos),
                        None => return Err(name.to_string()),
                    }
                }
            }
        }
        Ok(())
    }

    /// A field of the panicking family whose column is missing from the result set: a mapping that
    /// can never read its fields requires a code change, so it is loud, exactly like `Row::get()`
    /// panics.
    fn missing_column(name: &str) {
        panic!("column '{name}' is not in the result set")
    }

    /// The same for the non-panicking family: an [`Error::QueryParams`] that names the column.
    fn missing_column_err(name: &str) -> Error {
        Error::QueryParams(format!("column '{name}' is not in the result set").into())
    }
}

/// The per-query state a mapper works from: the hoisted path has resolved the field indices of the
/// mapped type, the name-based path has the name map of the result set.
///
/// A query always picks a mapper and its setup together, so a mapper only ever sees its own
/// variant.
pub(crate) enum Setup {
    Named(Arc<ColIdx>),
    Indexed(Hoisted),
}

/// The field indices a hoisted setup resolved, and the first field no column of the result set
/// matched.
///
/// A missing column is a verdict about a mapping that reads it, not about the query: it is handed
/// to the first row that would have read it, so a query whose result set is empty answers the way
/// it always did, and a query that does have rows is as loud as `Row::get()` is.
pub(crate) struct Hoisted {
    idx: Box<[ColIndex]>,
    missing: Option<Box<str>>,
}

impl Hoisted {
    /// The setup of a resolution, keeping the name of a field no column matched.
    fn of(resolution: Result<Vec<ColIndex>, String>) -> Self {
        match resolution {
            Ok(idx) => Self {
                idx: idx.into_boxed_slice(),
                missing: None,
            },
            Err(name) => Self {
                idx: Vec::new().into_boxed_slice(),
                missing: Some(name.into()),
            },
        }
    }

    /// The panicking side of a missing column: the field name that no column matched.
    ///
    /// # Panics
    ///
    /// If a field of the mapped type has no column in the result set, naming that field.  The
    /// `FromRowIndexed` family panics on such a field exactly like `Row::get()` does.
    fn require(&self) {
        if let Some(name) = &self.missing {
            ColIdx::missing_column(name);
        }
    }
}

impl Setup {
    /// The column index of a result set that no mapping touches: a hoisted setup reads every column
    /// by index, so the names of its rows are never looked up and this map stays empty, which an
    /// empty hash map never allocates for.
    fn empty_cols() -> &'static ColIdx {
        static EMPTY: OnceLock<ColIdx> = OnceLock::new();
        EMPTY.get_or_init(ColIdx::empty)
    }

    /// The owned-row counterpart of [`Setup::empty_cols`]: one shared, empty index for every row of
    /// every hoisted query, so mapping a row allocates nothing for a map it never reads.
    fn empty_cols_owned() -> &'static Arc<ColIdx> {
        static EMPTY: OnceLock<Arc<ColIdx>> = OnceLock::new();
        EMPTY.get_or_init(|| Arc::new(ColIdx::empty()))
    }

    /// The hoisted setup of a type of the panicking family: its field names are resolved against
    /// the prepared statement once, and a field whose column is missing is carried to the rows.
    fn hoisted<T: FromRowIndexed>(stmt: &Statement) -> Self {
        let maps = T::field_maps();
        Self::assert_slots(T::FIELD_COUNT, &maps);
        Self::Indexed(Hoisted::of(ColIdx::resolve_indexed(&maps, stmt)))
    }

    /// The [`Setup::hoisted`] of a result set that travelled through the network layer.
    fn hoisted_owned<T: FromRowIndexed>(cols: &Arc<ColIdx>) -> Self {
        let maps = T::field_maps();
        Self::assert_slots(T::FIELD_COUNT, &maps);
        Self::Indexed(Hoisted::of(ColIdx::resolve_indexed_owned(cols, &maps)))
    }

    /// The [`Setup::hoisted`] of the non-panicking family, which reports a missing column as an
    /// [`Error::QueryParams`] naming it.
    fn hoisted_try<T: TryFromRowIndexed>(stmt: &Statement) -> Self {
        let maps = T::field_maps();
        Self::assert_slots(T::FIELD_COUNT, &maps);
        Self::Indexed(Hoisted::of(ColIdx::resolve_indexed(&maps, stmt)))
    }

    /// The [`Setup::hoisted_try`] of a result set that travelled through the network layer.
    fn hoisted_try_owned<T: TryFromRowIndexed>(cols: &Arc<ColIdx>) -> Self {
        let maps = T::field_maps();
        Self::assert_slots(T::FIELD_COUNT, &maps);
        Self::Indexed(Hoisted::of(ColIdx::resolve_indexed_owned(cols, &maps)))
    }

    /// The contract of a hand-written [`FromRowIndexed`] / [`TryFromRowIndexed`] impl:
    /// `FIELD_COUNT` is the total of the slots its `field_maps()` describes, because its mapping
    /// reads the resolved indices with every slot below that count.  A derive computes both from
    /// the same plan, so only a hand-written impl can split them, and it is asserted where a debug
    /// build can name the two items that disagree.
    fn assert_slots(count: usize, maps: &FieldMaps<'_>) {
        debug_assert_eq!(
            count,
            ColIdx::slots(maps),
            "FIELD_COUNT ({count}) must be the total slots of field_maps() \
             ({})",
            ColIdx::slots(maps)
        );
    }

    /// A row of the panicking family, read through the indices this setup resolved.
    ///
    /// # Panics
    ///
    /// If a field of the mapped type has no column in the result set; see [`Hoisted::require`].
    fn hoisted_map<T: FromRowIndexed>(row: &mut Row<'_>, setup: &Self) -> T {
        let Self::Indexed(hoisted) = setup else {
            unreachable!("a query always uses the setup of its own mapper");
        };
        hoisted.require();
        T::from_row_indices(row, &hoisted.idx)
    }

    /// A row of the non-panicking family, read through the indices this setup resolved.
    fn hoisted_try_map<T: TryFromRowIndexed>(row: &mut Row<'_>, setup: &Self) -> Result<T, Error> {
        let Self::Indexed(hoisted) = setup else {
            unreachable!("a query always uses the setup of its own mapper");
        };
        if let Some(name) = &hoisted.missing {
            return Err(ColIdx::missing_column_err(name));
        }
        T::try_from_row_indices(row, &hoisted.idx)
    }

    /// The column index a [`Row::Borrowed`] of this query carries.
    pub(crate) fn row_cols(&self) -> &ColIdx {
        match self {
            Self::Named(cols) => cols,
            Self::Indexed(_) => Self::empty_cols(),
        }
    }

    /// The column index a [`Row::Owned`] of this query carries.
    pub(crate) fn row_cols_owned(&self) -> Arc<ColIdx> {
        match self {
            Self::Named(cols) => cols.clone(),
            Self::Indexed(_) => Self::empty_cols_owned().clone(),
        }
    }
}

/// The hoisted mapping of one mapped type, as the row loop of a query receives it.
///
/// `setup` runs once per query and `map` once per row; a `map` of a type that only has a
/// hand-written `From<&mut Row>` reads the row by its names and ignores the setup.
pub(crate) trait RowMapper<T> {
    fn setup(&self, stmt: &Statement) -> Result<Setup, Error>;
    /// The same setup for the owned rows of a result set that came through the network layer.
    fn setup_owned(&self, cols: &Arc<ColIdx>) -> Result<Setup, Error>;
    fn map<'r>(&self, row: &'r mut Row<'r>, setup: &Setup) -> T;
}

/// The non-panicking counterpart of [`RowMapper`].
pub(crate) trait TryRowMapper<T> {
    fn setup(&self, stmt: &Statement) -> Result<Setup, Error>;
    fn setup_owned(&self, cols: &Arc<ColIdx>) -> Result<Setup, Error>;
    fn map<'r>(&self, row: &'r mut Row<'r>, setup: &Setup) -> Result<T, Error>;
}

/// The hoisted mapping of one mapped type, all of it the monomorphized functions of exactly that
/// type: the setup that resolves its field names and the mapping that reads a row through the
/// resolved indices.
///
/// A registry keeps one of these per type, behind a `dyn Any` box, which is how the type comes
/// back out: a lookup only succeeds when the `TypeId` of the asking type is the `TypeId` the entry
/// was inserted under, so the function pointer a query calls is always the one of its own type.
pub(crate) struct HoistedFn<T> {
    setup: fn(&Statement) -> Setup,
    setup_owned: fn(&Arc<ColIdx>) -> Setup,
    map: for<'a> fn(&'a mut Row<'a>, &Setup) -> T,
}

// Every field is a function pointer, so the type is `Copy` for any `T`, which the derived impl
// would only give it with a `T: Copy` bound.
impl<T> Clone for HoistedFn<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HoistedFn<T> {}

/// The [`HoistedFn`] of the non-panicking family: its `map` answers with a `Result`.
pub(crate) struct HoistedTryFn<T> {
    setup: fn(&Statement) -> Setup,
    setup_owned: fn(&Arc<ColIdx>) -> Setup,
    map: for<'a> fn(&'a mut Row<'a>, &Setup) -> Result<T, Error>,
}

impl<T> Clone for HoistedTryFn<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HoistedTryFn<T> {}

/// The types a process has mapped so far, keyed by their `TypeId`, one map per family.  A
/// `FromRow` / `TryFromRow` derive registers its type from the body of the `From` / `TryFrom` impl
/// it emits, which is the first row of the first query that maps that type.
///
/// An entry is written by one `insert` of a finished value and never touched again, so a panic on a
/// thread holding the lock cannot leave a half-written map: every lock of this type is taken with
/// `into_inner()` after a poison, which keeps a process mapping a type after an unrelated panic
/// instead of failing every query that comes after it.
struct Registry {
    hoisted: FxHashMap<TypeId, Box<dyn Any + Send>>,
    hoisted_try: FxHashMap<TypeId, Box<dyn Any + Send>>,
}

impl Registry {
    fn get() -> &'static Mutex<Self> {
        static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            Mutex::new(Registry {
                hoisted: FxHashMap::default(),
                hoisted_try: FxHashMap::default(),
            })
        })
    }

    /// The mapping of `T` enters the registry once; a type that is in there already is a lookup and
    /// nothing else.
    fn register<T: FromRowIndexed + 'static>() {
        let mut reg = Self::get().lock().unwrap_or_else(|e| e.into_inner());
        if reg.hoisted.contains_key(&TypeId::of::<T>()) {
            return;
        }
        reg.hoisted.insert(
            TypeId::of::<T>(),
            Box::new(HoistedFn::<T> {
                setup: Setup::hoisted::<T>,
                setup_owned: Setup::hoisted_owned::<T>,
                map: Setup::hoisted_map::<T>,
            }),
        );
    }

    fn register_try<T: TryFromRowIndexed + 'static>() {
        let mut reg = Self::get().lock().unwrap_or_else(|e| e.into_inner());
        if reg.hoisted_try.contains_key(&TypeId::of::<T>()) {
            return;
        }
        reg.hoisted_try.insert(
            TypeId::of::<T>(),
            Box::new(HoistedTryFn::<T> {
                setup: Setup::hoisted_try::<T>,
                setup_owned: Setup::hoisted_try_owned::<T>,
                map: Setup::hoisted_try_map::<T>,
            }),
        );
    }

    fn lookup<T: 'static>() -> Option<HoistedFn<T>> {
        let reg = Self::get().lock().unwrap_or_else(|e| e.into_inner());
        reg.hoisted
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.downcast_ref::<HoistedFn<T>>())
            .copied()
    }

    fn lookup_try<T: 'static>() -> Option<HoistedTryFn<T>> {
        let reg = Self::get().lock().unwrap_or_else(|e| e.into_inner());
        reg.hoisted_try
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.downcast_ref::<HoistedTryFn<T>>())
            .copied()
    }
}

/// Registers the hoisted mapping of a type that implements [`FromRowIndexed`].
///
/// The `FromRow` derive calls this from the `From<&mut Row>` impl it generates, once per type, so
/// calling it by hand is never needed, and it is a cheap lookup for a type that is registered
/// already.
#[doc(hidden)]
pub fn register_hoisted<T: FromRowIndexed + 'static>() {
    Registry::register::<T>();
}

/// The [`register_hoisted`] of the non-panicking family, for the [`TryFromRowIndexed`] side of a
/// type.
#[doc(hidden)]
pub fn register_hoisted_try<T: TryFromRowIndexed + 'static>() {
    Registry::register_try::<T>();
}

/// The mapping a query of the panicking family runs: the hoisted one of a registered type, or the
/// `From<&mut Row>` body of a type that has nothing registered.
pub(crate) enum Mapper<T> {
    Named,
    Hoisted(HoistedFn<T>),
}

impl<T: for<'r> From<&'r mut Row<'r>> + 'static> Mapper<T> {
    pub(crate) fn new() -> Self {
        match Registry::lookup::<T>() {
            Some(entry) => Self::Hoisted(entry),
            None => Self::Named,
        }
    }
}

impl<T: for<'r> From<&'r mut Row<'r>> + 'static> RowMapper<T> for Mapper<T> {
    fn setup(&self, stmt: &Statement) -> Result<Setup, Error> {
        match self {
            Self::Named => Ok(Setup::Named(Arc::new(ColIdx::lookup_only(stmt)))),
            Self::Hoisted(fn_) => Ok((fn_.setup)(stmt)),
        }
    }

    fn setup_owned(&self, cols: &Arc<ColIdx>) -> Result<Setup, Error> {
        match self {
            Self::Named => Ok(Setup::Named(cols.clone())),
            Self::Hoisted(fn_) => Ok((fn_.setup_owned)(cols)),
        }
    }

    #[inline]
    fn map<'r>(&self, row: &'r mut Row<'r>, setup: &Setup) -> T {
        match self {
            Self::Named => T::from(row),
            Self::Hoisted(fn_) => (fn_.map)(row, setup),
        }
    }
}

/// The [`Mapper`] of the non-panicking family.
pub(crate) enum MapperTry<T> {
    Named,
    Hoisted(HoistedTryFn<T>),
}

impl<T: for<'r> TryFrom<&'r mut Row<'r>, Error = crate::Error> + 'static> MapperTry<T> {
    pub(crate) fn new() -> Self {
        match Registry::lookup_try::<T>() {
            Some(entry) => Self::Hoisted(entry),
            None => Self::Named,
        }
    }
}

impl<T: for<'r> TryFrom<&'r mut Row<'r>, Error = crate::Error> + 'static> TryRowMapper<T>
    for MapperTry<T>
{
    fn setup(&self, stmt: &Statement) -> Result<Setup, Error> {
        match self {
            Self::Named => Ok(Setup::Named(Arc::new(ColIdx::lookup_only(stmt)))),
            Self::Hoisted(fn_) => Ok((fn_.setup)(stmt)),
        }
    }

    fn setup_owned(&self, cols: &Arc<ColIdx>) -> Result<Setup, Error> {
        match self {
            Self::Named => Ok(Setup::Named(cols.clone())),
            Self::Hoisted(fn_) => Ok((fn_.setup_owned)(cols)),
        }
    }

    #[inline]
    fn map<'r>(&self, row: &'r mut Row<'r>, setup: &Setup) -> Result<T, Error> {
        match self {
            Self::Named => T::try_from(row),
            Self::Hoisted(fn_) => (fn_.map)(row, setup),
        }
    }
}

#[derive(Debug)]
pub enum Row<'a> {
    /// A row borrowed from a local statement, with the column index of that statement.
    Borrowed(&'a SqliteRow<'a>, &'a ColIdx),
    /// A row whose values were read out into owned data, to travel through the network layer or to
    /// be handed over by a streaming query.
    Owned(RowOwned),
}

impl Row<'_> {
    ///
    /// # Panics
    ///
    /// If the column `idx` is not on the row, or its value cannot be converted into `T`.
    pub fn get<T>(&mut self, idx: &str) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Row::Borrowed(b, cols) => {
                let res = ColIdx::borrowed_get(b, cols, idx);
                if res.is_err() {
                    error!("Cannot convert column index '{idx}' to requested type");
                }
                res.unwrap()
            }
            Row::Owned(o) => o.get(idx),
        }
    }

    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that is not on the row, or whose value cannot be converted
    /// into `T`; on a borrowed row that is the [`Error::Sqlite`] of `rusqlite`.
    pub fn try_get<T>(&mut self, idx: &str) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        match self {
            Self::Borrowed(r, cols) => ColIdx::borrowed_get(r, cols, idx).map_err(Error::from),
            Self::Owned(o) => o.try_get(idx),
        }
    }

    /// Reads the column behind a resolved index; the indexed counterpart of [`Row::get`].
    ///
    /// # Panics
    ///
    /// If the column was already read, or its value cannot be converted into `T`.
    pub fn get_idx<T>(&mut self, idx: ColIndex) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + FromSql,
    {
        match self {
            Row::Borrowed(b, _) => {
                let res = b.get(idx.pos());
                if res.is_err() {
                    error!("Cannot convert column {} to requested type", idx.pos());
                }
                res.unwrap()
            }
            Row::Owned(o) => o.get_idx(idx),
        }
    }

    /// Reads the column behind a resolved index; the indexed counterpart of [`Row::try_get`].
    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that was already read, or whose value cannot be converted into
    /// `T`; on a borrowed row that is the [`Error::Sqlite`] of `rusqlite`.
    pub fn try_get_idx<T>(&mut self, idx: ColIndex) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + FromSql,
    {
        match self {
            Self::Borrowed(r, _) => r.get(idx.pos()).map_err(Error::from),
            Self::Owned(o) => o.try_get_idx(idx),
        }
    }
}

/// A row whose values were read out into owned data, together with the column index of the result
/// set they belong to, shared through an `Arc`.
///
/// This is the [`Row::Owned`] of a result set that travels through the network layer, and the row a
/// streaming query hands over: owned data that leaves the thread it was read on.  Reading a column
/// takes it out of its slot, so every column of a row can be read exactly once, like the name scan
/// of a borrowed row does.
#[derive(Debug)]
pub struct RowOwned {
    row: RowValues,
    cols: Arc<ColIdx>,
}

impl RowOwned {
    pub(crate) fn new(row: RowValues, cols: Arc<ColIdx>) -> Self {
        Self { row, cols }
    }

    /// The column names of the result set this row belongs to, in result-set order: the header its
    /// values are keyed by when it is serialized, and the names [`RowOwned::get`] resolves.
    ///
    /// The header is shared through the `Arc` of the row, so reading it allocates nothing, on any
    /// row of any result set.  A query that returns no rows has no row to read it from, which is
    /// the one thing an owned row cannot give you.
    #[must_use]
    pub fn columns(&self) -> &[Arc<str>] {
        self.cols.names()
    }

    /// The value of the column `idx`, with the panicking behaviour of [`Row::get`].
    ///
    /// # Panics
    ///
    /// If the column `idx` is not on the row, or its value cannot be converted into `T`.
    pub fn get<T>(&mut self, idx: &str) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        self.row
            .try_get(self.cols.as_ref(), idx)
            .unwrap_or_else(|err| {
                panic!("Cannot convert column index '{idx}' to requested type: {err:?}")
            })
    }

    /// The non-panicking counterpart of [`RowOwned::get`].
    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that is not on the row, or whose value cannot be converted into
    /// `T`.
    pub fn try_get<T>(&mut self, idx: &str) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error> + rusqlite::types::FromSql,
    {
        self.row.try_get(self.cols.as_ref(), idx)
    }

    /// Reads the column behind a resolved index; the indexed counterpart of [`RowOwned::get`].
    ///
    /// # Panics
    ///
    /// If the column was already read, or its value cannot be converted into `T`.
    fn get_idx<T>(&mut self, idx: ColIndex) -> T
    where
        T: TryFrom<ValueOwned, Error = crate::Error>,
    {
        self.try_get_idx(idx)
            .unwrap_or_else(|err| panic!("Cannot convert a column: {err}"))
    }

    /// The indexed counterpart of [`RowOwned::try_get`].
    ///
    /// # Errors
    ///
    /// The [`Error`] of a column that was already read, or whose value cannot be converted into `T`.
    fn try_get_idx<T>(&mut self, idx: ColIndex) -> Result<T, Error>
    where
        T: TryFrom<ValueOwned, Error = crate::Error>,
    {
        self.row.try_get_index(idx.pos())
    }
}

/// A [`ValueOwned`] as a plain JSON value: a number, a string, an array of bytes, or `null`.
///
/// The serialized form of `ValueOwned` itself tags its variant (`{"Integer": 1}`), which is what
/// the dashboard reads, so an owned row writes its values through this adapter to keep the JSON of
/// a query plain.
struct ValueJson<'a>(&'a ValueOwned);

impl Serialize for ValueJson<'_> {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            ValueOwned::Null => ser.serialize_none(),
            ValueOwned::Integer(v) => ser.serialize_i64(*v),
            ValueOwned::Real(v) => ser.serialize_f64(*v),
            ValueOwned::Text(v) => ser.serialize_str(v),
            ValueOwned::Blob(v) => ser.serialize_bytes(v),
        }
    }
}

/// The row as a JSON object, keyed by the column names of its result set, in result-set order.
///
/// Values are plain JSON: a number, a string, an array of bytes, and `null` for a SQL NULL.  A
/// column you have already read is `null` as well, because [`RowOwned::get`] takes a value out of
/// its slot and serialization only ever gets `&self`.  A query whose columns repeat a name, like a
/// join over two tables that share one, emits that key once per column, exactly like the row itself
/// repeats it.
impl Serialize for RowOwned {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let names = self.cols.names();
        let mut obj = ser.serialize_map(Some(names.len()))?;
        for (name, value) in names.iter().zip(self.row.values.iter()) {
            let value = match value {
                Some(value) => ValueJson(value),
                None => ValueJson(&ValueOwned::Null),
            };
            obj.serialize_entry(&**name, &value)?;
        }
        obj.end()
    }
}

/// A result set that travels through the network, or into the dashboard: the column names of the
/// query once, and the values of every row in that same order.
#[derive(Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct RowsOwned {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<RowValues>,
}

impl RowsOwned {
    pub(crate) fn new(columns: Vec<String>, rows: Vec<RowValues>) -> Self {
        Self { columns, rows }
    }

    /// Split into the shared column index and the rows, the shape [`Row::Owned`] is built from.
    pub(crate) fn into_parts(self) -> (Arc<ColIdx>, Vec<RowValues>) {
        let cols = self.col_idx();
        (cols, self.rows)
    }

    fn col_idx(&self) -> Arc<ColIdx> {
        Arc::new(ColIdx::from_names(&self.columns))
    }
}

/// [`RowsOwned`] for statements that report every row as its own `Result`, like
/// `execute_returning`, where one failing row sits next to the rows that succeeded.
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct RowsOwnedChecked {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Result<RowValues, Error>>,
}

impl RowsOwnedChecked {
    pub(crate) fn new(columns: Vec<String>, rows: Vec<Result<RowValues, Error>>) -> Self {
        Self { columns, rows }
    }

    /// Split into the shared column index and the rows, keeping each row's `Result`.
    pub(crate) fn into_parts(self) -> (Arc<ColIdx>, Vec<Result<RowValues, Error>>) {
        let cols = self.col_idx();
        (cols, self.rows)
    }

    fn col_idx(&self) -> Arc<ColIdx> {
        Arc::new(ColIdx::from_names(&self.columns))
    }
}

/// The values of one row, in the column order of the result set it was read from.  The names of
/// those columns are not repeated per row; they are the `columns` of the [`RowsOwned`] this row
/// belongs to.
///
/// Reading a value takes it out of its slot, so a column can be read exactly once, like the
/// per-row name scan did before.  Slots stay in place, which is what keeps the shared column
/// index of the result set valid for every lookup of the row.
#[derive(Debug, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct RowValues {
    pub(crate) values: Vec<Option<ValueOwned>>,
}

impl RowValues {
    #[inline]
    pub(crate) fn from_row_column(row: &rusqlite::Row, columns: &[ColumnInfo]) -> Self {
        let mut cols = Vec::with_capacity(columns.len());

        for (i, info) in columns.iter().enumerate() {
            let value = match info.typ {
                ColumnType::Expr => {
                    // returned expressions can be any type we don't know in advance
                    // The chain below covers every SQLite storage class, so the final Null is
                    // only reachable for a genuine SQL NULL.
                    if let Ok(text) = row.get::<_, String>(i) {
                        ValueOwned::Text(text)
                    } else if let Ok(i) = row.get::<_, i64>(i) {
                        ValueOwned::Integer(i)
                    } else if let Ok(r) = row.get::<_, f64>(i) {
                        ValueOwned::Real(r)
                    } else if let Ok(b) = row.get::<_, Vec<u8>>(i) {
                        ValueOwned::Blob(b)
                    } else {
                        ValueOwned::Null
                    }
                }
                // ColumnType::Expr => row.get(i).map(ValueOwned::Text).unwrap_or(ValueOwned::Null),
                // Typed columns: a genuine SQL NULL maps to Null; a stored value that cannot be
                // converted is a type mismatch and must fail loudly instead of silently
                // becoming Null.
                ColumnType::Integer => match row.get::<_, Option<i64>>(i) {
                    Ok(Some(v)) => ValueOwned::Integer(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Integer but the stored value \
                         cannot be converted to i64: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Real => match row.get::<_, Option<f64>>(i) {
                    Ok(Some(v)) => ValueOwned::Real(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Real but the stored value \
                         cannot be converted to f64: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Text => match row.get::<_, Option<String>>(i) {
                    Ok(Some(v)) => ValueOwned::Text(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Text but the stored value \
                         cannot be converted to String: {err:?}",
                        info.name
                    ),
                },
                ColumnType::Blob => match row.get::<_, Option<Vec<u8>>>(i) {
                    Ok(Some(v)) => ValueOwned::Blob(v),
                    Ok(None) => ValueOwned::Null,
                    Err(err) => panic!(
                        "Column '{}' is declared as Blob but the stored value \
                         cannot be converted to Vec<u8>: {err:?}",
                        info.name
                    ),
                },
            };

            cols.push(Some(value));
        }

        Self { values: cols }
    }

    pub(crate) fn try_get<T: TryFrom<ValueOwned, Error = crate::Error>>(
        &mut self,
        cols: &ColIdx,
        idx: &str,
    ) -> Result<T, Error> {
        let not_found = || Error::QueryParams(format!("column '{idx}' not found").into());
        match cols.index(idx).and_then(|i| self.values.get_mut(i)) {
            Some(slot) => match slot.take() {
                Some(value) => T::try_from(value),
                None => Err(not_found()),
            },
            None => Err(not_found()),
        }
    }

    /// Reads a column by its position; the indexed counterpart of [`RowValues::try_get`].
    pub(crate) fn try_get_index<T: TryFrom<ValueOwned, Error = crate::Error>>(
        &mut self,
        pos: usize,
    ) -> Result<T, Error> {
        let not_found = || Error::QueryParams(format!("column {pos} not found").into());
        match self.values.get_mut(pos).and_then(|slot| slot.take()) {
            Some(value) => T::try_from(value),
            None => Err(not_found()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub typ: ColumnType,
}

impl ColumnInfo {
    /// The mapping information of every column of a prepared statement, in result-set order.
    #[inline]
    pub(crate) fn mapping_cols_from_stmt<'a>(
        columns: impl ExactSizeIterator<Item = rusqlite::Column<'a>>,
    ) -> Vec<Self> {
        let mut cols = Vec::with_capacity(columns.len());
        for col in columns {
            cols.push(Self {
                name: col.name().to_string(),
                typ: ColumnType::from_decl_type(col.decl_type()),
            });
        }
        cols
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColumnType {
    Expr,
    Integer,
    Real,
    Text,
    Blob,
}

impl ColumnType {
    #[inline]
    fn from_decl_type(typ: Option<&str>) -> Self {
        // we will always match as few character as possible for improved speed
        if let Some(t) = typ {
            // We will check "correct" types first for better speed with correct definitions.
            // SQLITE will convert int, integer, text, blob, real to UPPERCASE on creation.
            if t.starts_with("INT") || t.starts_with("int") {
                return Self::Integer;
            }

            match t {
                "TEXT" | "text" => Self::Text,
                "BLOB" | "blob" => Self::Blob,
                "REAL" | "real" => Self::Real,
                _ => {
                    // When the proper types don't exist, try check type-affinity.
                    // Type affinity matches will NOT be converted to uppercase automatically!

                    if t.is_empty() {
                        return Self::Blob;
                    }

                    let ty = t.to_uppercase();

                    // 3.1. Determination Of Column Affinity from SQLite docs:
                    // https://www.sqlite.org/datatype3.html

                    // .starts_with("INT") already checked
                    // INT
                    // INTEGER
                    // INT2
                    // INT8
                    //
                    // TINYINT
                    // SMALLINT
                    // MEDIUMINT
                    // BIGINT
                    // UNSIGNED BIG INT
                    if ty.contains("INT") {
                        Self::Integer

                    // "TEXT already checked
                    // TEXT
                    //
                    // CHARACTER(20)
                    // VARCHAR(255)
                    // VARYING CHARACTER(255)
                    // NCHAR(55)
                    // NATIVE CHARACTER(70)
                    // NVARCHAR(100)
                    // CLOB
                    } else if ty.contains("CHAR") || ty.contains("CLOB") {
                        Self::Text

                    // .starts_with("RE") already checked
                    // REAL
                    //
                    // DOUBLE
                    // DOUBLE PRECISION
                    // FLOAT
                    } else if ty.contains("FLOA") || ty.contains("DOUB") {
                        Self::Real

                    // Anything SQLite cannot match properly should be an INTEGER / NUMERIC.
                    } else {
                        Self::Integer
                    }
                }
            }
        } else {
            Self::Expr
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ValueOwned {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl ValueOwned {
    #[inline]
    fn try_as_str(&self) -> Result<&str, Error> {
        match self {
            ValueOwned::Text(s) => Ok(s.as_str()),
            _ => Err(Error::Sqlite("Cannot convert ValueOwned to &str".into())),
        }
    }
}

impl TryFrom<ValueOwned> for i64 {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i),
            _ => Err(Error::Sqlite("Cannot convert into i64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<i64> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => i64::try_from(v).map(Some),
        }
    }
}

#[cfg(any(feature = "cast_ints", feature = "cast_ints_unchecked"))]
#[allow(clippy::cast_sign_loss)]
// Both `cast_ints` features exist to downcast the DB's `i64` into a smaller integer on purpose:
// the unchecked one wraps exactly like `as`, the checked one clamps to the target's bounds.
// Clippy cannot see that clamp, so the sign loss it warns about is exactly what these features
// ask for. The allow sits on the module because the casts come out of a macro, where an allow on
// the generated function does not reach them.
mod cast_ints {
    impl TryFrom<super::ValueOwned> for u64 {
        type Error = crate::Error;

        #[inline]
        fn try_from(value: super::ValueOwned) -> Result<Self, Self::Error> {
            let i = i64::try_from(value)?;
            // `cast_ints` clamps: anything below zero lands on 0.
            Ok(i.max(0).cast_unsigned())
        }
    }

    impl TryFrom<super::ValueOwned> for Option<u64> {
        type Error = crate::Error;

        #[inline]
        fn try_from(value: super::ValueOwned) -> Result<Option<u64>, Self::Error> {
            let i: Option<i64> = value.try_into()?;
            Ok(i.map(|i| i.cast_unsigned()))
        }
    }

    macro_rules! downcast_int(
        ($t:ty) => (
            impl TryFrom<super::ValueOwned> for $t {
                type Error = crate::Error;

                #[inline]
                fn try_from(value: super::ValueOwned) -> Result<Self, Self::Error> {
                    let i = i64::try_from(value)?;

                    #[cfg(feature = "cast_ints_unchecked")]
                    return Ok(i as $t);

                    #[cfg(feature = "cast_ints")]
                    if i < 0 {
                        Ok(::core::cmp::max(<$t>::MIN as i64, i) as $t)
                    } else {
                        Ok(::core::cmp::min(<$t>::MAX as i64, i) as $t)
                    }
                }
            }
        )
    );

    downcast_int!(i32);
    downcast_int!(i16);
    downcast_int!(i8);

    downcast_int!(u32);
    downcast_int!(u16);
    downcast_int!(u8);

    macro_rules! downcast_int_opt(
        ($t:ty) => (
            impl TryFrom<super::ValueOwned> for Option<$t> {
                type Error = crate::Error;

                #[inline]
                fn try_from(value: super::ValueOwned) -> Result<Option<$t>, Self::Error> {
                    let i: Option<i64> = value.try_into()?;
                    Ok(i.map(|i| i as $t))
                }
            }
        )
    );

    downcast_int_opt!(i32);
    downcast_int_opt!(i16);
    downcast_int_opt!(i8);

    downcast_int_opt!(u32);
    downcast_int_opt!(u16);
    downcast_int_opt!(u8);
}

impl TryFrom<ValueOwned> for f64 {
    type Error = Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Real(r) => Ok(r),
            _ => Err(Error::Sqlite("Cannot convert into f64".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<f64> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => f64::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for bool {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Integer(i) => Ok(i == 1),
            // SQLite stores a boolean as an exact 1.0, and only that: a margin of error would
            // make near-values true, which is exactly what a stored flag must never do.
            ValueOwned::Real(i) => Ok(i.total_cmp(&1.0) == Ordering::Equal),
            ValueOwned::Text(s) => Ok(s.as_str() == "true"),
            _ => Err(Error::Sqlite("Cannot convert into bool".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<bool> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => v.try_into().map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for String {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => Ok(s),
            _ => Err(Error::Sqlite("Cannot convert into String".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<String> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => String::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for Vec<u8> {
    type Error = Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => Ok(b),
            _ => Err(Error::Sqlite("Cannot convert into Vec<u8>".into())),
        }
    }
}

impl TryFrom<ValueOwned> for Option<Vec<u8>> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => Vec::try_from(v).map(Some),
        }
    }
}

impl<const N: usize> TryFrom<ValueOwned> for [u8; N] {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => Ok(b
                .try_into()
                .map_err(|_| Error::Sqlite("Cannot convert into [u8]: incorrect length".into()))?),
            _ => Err(Error::Sqlite("Cannot convert into [u8]".into())),
        }
    }
}

impl<const N: usize> TryFrom<ValueOwned> for Option<[u8; N]> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Null => Ok(None),
            v => <[u8; N]>::try_from(v).map(Some),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveDate {
    type Error = crate::Error;

    /// "YYYY-MM-DD" => ISO 8601 calendar date without timezone.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        match Self::parse_from_str(s, "%F") {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveTime {
    type Error = crate::Error;

    /// "HH:MM"/"HH:MM:SS"/"HH:MM:SS.SSS" => ISO 8601 time without timezone.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        let fmt = match s.len() {
            5 => "%H:%M",
            8 => "%T",
            _ => "%T%.f",
        };

        match Self::parse_from_str(s, fmt) {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for NaiveDateTime {
    type Error = crate::Error;

    /// "YYYY-MM-DD HH:MM:SS"/"YYYY-MM-DD HH:MM:SS.SSS" => ISO 8601 combined date
    /// and time without timezone. ("YYYY-MM-DDTHH:MM:SS"/"YYYY-MM-DDTHH:MM:SS.SSS"
    /// also supported)
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        let fmt = if s.len() >= 11 && s.as_bytes()[10] == b'T' {
            "%FT%T%.f"
        } else {
            "%F %T%.f"
        };

        match Self::parse_from_str(s, fmt) {
            Ok(slf) => Ok(slf),
            Err(err) => Err(Error::Sqlite(err.to_string().into())),
        }
    }
}

impl TryFrom<ValueOwned> for DateTime<Utc> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Utc>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        {
            // Try to parse value as rfc3339 first.
            let s = value.try_as_str()?;

            let fmt = if s.len() >= 11 && s.as_bytes()[10] == b'T' {
                "%FT%T%.f%#z"
            } else {
                "%F %T%.f%#z"
            };

            if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
                return Ok(dt.with_timezone(&Utc));
            }
        }

        // Couldn't parse as rfc3339 - fall back to NaiveDateTime.
        NaiveDateTime::try_from(value).map(|dt| Utc.from_utc_datetime(&dt))
    }
}

impl TryFrom<ValueOwned> for DateTime<Local> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Local>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let utc_dt = DateTime::<Utc>::try_from(value)?;
        Ok(utc_dt.with_timezone(&Local))
    }
}

impl TryFrom<ValueOwned> for DateTime<FixedOffset> {
    type Error = crate::Error;

    /// RFC3339 ("YYYY-MM-DD HH:MM:SS.SSS[+-]HH:MM") into `DateTime<Local>`.
    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let s = value.try_as_str()?;
        Self::parse_from_rfc3339(s)
            .or_else(|_| Self::parse_from_str(s, "%F %T%.f%:z"))
            .map_err(|err| Error::Sqlite(err.to_string().into()))
    }
}

impl TryFrom<ValueOwned> for serde_json::Value {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        let slf = match value {
            ValueOwned::Null => serde_json::Value::Null,
            ValueOwned::Integer(i) => serde_json::Value::from(i),
            ValueOwned::Real(r) => serde_json::Value::from(r),
            ValueOwned::Text(s) => serde_json::from_str(s.as_str())
                .map_err(|err| Error::Sqlite(err.to_string().into()))?,
            ValueOwned::Blob(b) => {
                serde_json::from_slice(&b).map_err(|err| Error::Sqlite(err.to_string().into()))?
            }
        };
        Ok(slf)
    }
}

impl TryFrom<ValueOwned> for Option<url::Url> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => {
                if s.is_empty() {
                    return Ok(None);
                }
                match url::Url::parse(&s) {
                    Ok(url) => Ok(Some(url)),
                    Err(_) => Err(Error::Sqlite(
                        "Cannot parse URL from given input: {s}".into(),
                    )),
                }
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse URL from TEXT column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for url::Url {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Text(s) => match url::Url::parse(&s) {
                Ok(url) => Ok(url),
                Err(_) => Err(Error::Sqlite(
                    "Cannot parse URL from given input: {s}".into(),
                )),
            },
            _ => Err(Error::Sqlite(
                "Cannot only parse URL from TEXT column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for Option<uuid::Uuid> {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => {
                if b.is_empty() {
                    return Ok(None);
                }
                let bytes = <[u8; 16]>::try_from(b.as_slice()).map_err(|_| {
                    Error::Sqlite("Invalid data in BLOB to be able to convert to UUID".into())
                })?;
                Ok(Some(uuid::Uuid::from_u128(u128::from_be_bytes(bytes))))
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse UUID from BLOB column".into(),
            )),
        }
    }
}

impl TryFrom<ValueOwned> for uuid::Uuid {
    type Error = crate::Error;

    #[inline]
    fn try_from(value: ValueOwned) -> Result<Self, Self::Error> {
        match value {
            ValueOwned::Blob(b) => {
                let bytes = <[u8; 16]>::try_from(b.as_slice()).map_err(|_| {
                    Error::Sqlite("Invalid data in BLOB to be able to convert to UUID".into())
                })?;
                Ok(uuid::Uuid::from_u128(u128::from_be_bytes(bytes)))
            }
            _ => Err(Error::Sqlite(
                "Cannot only parse UUID from BLOB column".into(),
            )),
        }
    }
}

#[cfg(all(test, feature = "macros"))]
mod try_from_row_tests {
    use super::*;
    use crate::macros::TryFromRow;

    #[derive(Debug, PartialEq, TryFromRow)]
    struct Inner {
        a: String,
    }

    #[derive(Debug, PartialEq, TryFromRow)]
    struct Outer {
        name: String,
        #[column(from_i32)]
        count: i32,
        #[column(parse)]
        big: u64,
        #[column(skip)]
        skipped: i64,
        #[column(flatten)]
        inner: Inner,
    }

    fn make_row(cols: &[(&str, ValueOwned)]) -> Row<'static> {
        let names: Vec<String> = cols.iter().map(|(name, _)| (*name).to_string()).collect();
        let values: Vec<Option<ValueOwned>> =
            cols.iter().map(|(_, value)| Some(value.clone())).collect();
        let cols = Arc::new(ColIdx::from_names(&names));
        Row::Owned(RowOwned::new(RowValues { values }, cols))
    }

    // Pin the target type so `TryFrom` resolves unambiguously (both `Inner` and `Outer`
    // implement it).
    fn try_outer(row: &mut Row<'_>) -> Result<Outer, Error> {
        std::convert::TryFrom::try_from(row)
    }

    #[test]
    fn try_from_row_maps_all_attribute_kinds() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(42)),
            ("big", ValueOwned::Text(u64::MAX.to_string())),
            ("skipped", ValueOwned::Integer(999)),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let outer = try_outer(&mut row).unwrap();
        assert_eq!(outer.name, "hi");
        assert_eq!(outer.count, 42);
        assert_eq!(outer.big, u64::MAX);
        // `skip` is never read from the row, so it must stay at its default.
        assert_eq!(outer.skipped, 0);
        assert_eq!(outer.inner.a, "inner");
    }

    #[test]
    fn try_from_row_i32_overflow_is_an_error_not_a_panic() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(i64::MAX)),
            ("big", ValueOwned::Text("1".into())),
            ("skipped", ValueOwned::Null),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let err = try_outer(&mut row).unwrap_err();
        assert!(
            matches!(err, Error::Sqlite(_)),
            "expected Sqlite error: {err:?}"
        );
    }

    #[test]
    fn try_from_row_parse_failure_is_an_error() {
        let mut row = make_row(&[
            ("name", ValueOwned::Text("hi".into())),
            ("count", ValueOwned::Integer(1)),
            ("big", ValueOwned::Text("not-a-number".into())),
            ("skipped", ValueOwned::Null),
            ("a", ValueOwned::Text("inner".into())),
        ]);
        let err = try_outer(&mut row).unwrap_err();
        assert!(
            matches!(err, Error::Sqlite(_)),
            "expected Sqlite error: {err:?}"
        );
    }
}

#[cfg(test)]
mod owned_row_tests {
    use super::*;

    fn owned(values: Vec<Option<ValueOwned>>, names: &[&str]) -> (Row<'static>, Arc<ColIdx>) {
        let cols = Arc::new(ColIdx::from_names(
            &names
                .iter()
                .map(|n| (*n).to_string())
                .collect::<Vec<String>>(),
        ));
        (
            Row::Owned(RowOwned::new(RowValues { values }, cols.clone())),
            cols,
        )
    }

    #[test]
    fn a_column_can_only_be_read_once() {
        let (mut row, _) = owned(
            vec![
                Some(ValueOwned::Integer(1)),
                Some(ValueOwned::Integer(2)),
                Some(ValueOwned::Integer(3)),
            ],
            &["a", "b", "c"],
        );
        assert_eq!(row.try_get::<i64>("a").unwrap(), 1);
        assert!(
            matches!(row.try_get::<i64>("a"), Err(Error::QueryParams(_))),
            "a second read of the same column must be an error"
        );
        // a taken slot never touches the other columns of the row
        assert_eq!(row.try_get::<i64>("c").unwrap(), 3);
        assert_eq!(row.try_get::<i64>("b").unwrap(), 2);
    }

    #[test]
    #[should_panic(expected = "not found")]
    fn get_after_a_taken_slot_panics() {
        let (mut row, _) = owned(vec![Some(ValueOwned::Integer(1))], &["a"]);
        assert_eq!(row.get::<i64>("a"), 1);
        let _ = row.get::<i64>("a");
    }

    #[test]
    fn a_name_that_is_not_on_the_row_is_an_error() {
        let (mut row, _) = owned(vec![Some(ValueOwned::Integer(1))], &["a"]);
        assert!(
            matches!(row.try_get::<i64>("missing"), Err(Error::QueryParams(_))),
            "an unknown column must be an error"
        );
    }

    #[test]
    fn duplicate_names_map_to_the_first_column() {
        // rusqlite's own scan returns the first match, the index has to do the same.
        let cols = ColIdx::from_names(&["id".to_string(), "ts".to_string(), "id".to_string()]);
        assert_eq!(cols.index("id"), Some(0));
        assert_eq!(cols.index("ts"), Some(1));
        assert_eq!(cols.index("other"), None);
    }
}
#[cfg(test)]
mod registry_tests {
    use super::*;

    /// A mapped type with both a name-based and a hoisted mapping, written by hand so the registry
    /// is exercised without the derive.
    struct Plain {
        pub id: i64,
        pub name: String,
    }

    impl From<&mut Row<'_>> for Plain {
        fn from(row: &mut Row<'_>) -> Self {
            Self {
                id: row.get("id"),
                name: row.get("name"),
            }
        }
    }

    impl FromRowIndexed for Plain {
        const FIELD_COUNT: usize = 2;

        fn field_maps() -> FieldMaps<'static> {
            const NAMES: &FieldNames = &FieldNames::new(&[("id", 0u16), ("name", 1u16)]);
            vec![(NAMES, 0u16)]
        }

        fn from_row_indices(row: &mut Row<'_>, idx: &[ColIndex]) -> Self {
            Self {
                id: row.get_idx(idx[0]),
                name: row.get_idx(idx[1]),
            }
        }
    }

    /// A type that only the test below registers.  The registry is one map for the whole process,
    /// so an assertion that a type is not in it yet needs a type no other test touches.
    struct Late {
        pub id: i64,
    }

    impl From<&mut Row<'_>> for Late {
        fn from(row: &mut Row<'_>) -> Self {
            Self { id: row.get("id") }
        }
    }

    impl FromRowIndexed for Late {
        const FIELD_COUNT: usize = 1;

        fn field_maps() -> FieldMaps<'static> {
            const NAMES: &FieldNames = &FieldNames::new(&[("id", 0u16)]);
            vec![(NAMES, 0u16)]
        }

        fn from_row_indices(row: &mut Row<'_>, idx: &[ColIndex]) -> Self {
            Self {
                id: row.get_idx(idx[0]),
            }
        }
    }

    /// A type that only has the name-based mapping.
    struct NameOnly {
        pub id: i64,
    }

    impl From<&mut Row<'_>> for NameOnly {
        fn from(row: &mut Row<'_>) -> Self {
            Self { id: row.get("id") }
        }
    }

    /// A type of the non-panicking family, hand-written like [`Plain`] is for the panicking one.
    #[derive(Debug)]
    struct TryPlain {
        pub id: i64,
        pub name: String,
    }

    impl TryFrom<&mut Row<'_>> for TryPlain {
        type Error = Error;

        fn try_from(row: &mut Row<'_>) -> Result<Self, Self::Error> {
            Ok(Self {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
            })
        }
    }

    impl TryFromRowIndexed for TryPlain {
        const FIELD_COUNT: usize = 2;

        fn field_maps() -> FieldMaps<'static> {
            const NAMES: &FieldNames = &FieldNames::new(&[("id", 0u16), ("name", 1u16)]);
            vec![(NAMES, 0u16)]
        }

        fn try_from_row_indices(row: &mut Row<'_>, idx: &[ColIndex]) -> Result<Self, Error> {
            Ok(Self {
                id: row.try_get_idx(idx[0])?,
                name: row.try_get_idx(idx[1])?,
            })
        }
    }

    fn conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (id INTEGER, name TEXT)", [])
            .unwrap();
        conn.execute("INSERT INTO t VALUES (7, 'seven')", [])
            .unwrap();
        conn
    }

    #[test]
    fn an_unregistered_type_is_mapped_by_its_names() {
        // Nothing registers `NameOnly`, so a query of it runs the body of its `From` impl, reading
        // the names that body passes to `row.get` - on a borrowed row and on the owned row a
        // mapped query builds.
        assert!(Registry::lookup::<NameOnly>().is_none());

        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT id, name FROM t WHERE id = 7")
            .unwrap();
        let mapper = Mapper::<NameOnly>::new();
        assert!(matches!(mapper, Mapper::Named));
        let setup = mapper.setup(&stmt).unwrap();
        assert!(matches!(setup, Setup::Named(_)));

        {
            let mut rows = stmt.raw_query();
            let row = rows.next().unwrap().unwrap();
            let mut row = Row::Borrowed(row, setup.row_cols());
            assert_eq!(mapper.map(&mut row, &setup).id, 7);
        }

        // The very row a `query_map` of this type maps: an owned row on the same index, built,
        // mapped, and dropped by the mapper.  No caller of this path ever holds a row, so the
        // index carries no header, and the read by name is the whole job it has to do.
        let cols = setup.row_cols_owned();
        assert!(
            cols.names().is_empty(),
            "the mapping path builds no header for its rows"
        );
        let columns = ColumnInfo::mapping_cols_from_stmt(stmt.columns());
        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let values = RowValues::from_row_column(row, &columns);
        drop(rows);
        let mut owned = Row::Owned(RowOwned::new(values, cols));
        assert_eq!(
            NameOnly::from(&mut owned).id,
            7,
            "an owned row on a header-less index reads its columns by name"
        );
    }

    /// Registration is what moves a type onto the hoisted path, and the type of this test is
    /// registered by no other test, so the assertion about the registry before the registration
    /// cannot depend on the order the tests run in.
    #[test]
    fn a_type_is_mapped_by_its_names_until_it_registers() {
        assert!(
            Registry::lookup::<Late>().is_none(),
            "no other test registers this type"
        );

        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT name, id FROM t WHERE id = 7")
            .unwrap();
        let mapper = Mapper::<Late>::new();
        assert!(
            matches!(mapper, Mapper::Named),
            "the type is not registered yet"
        );
        let setup = mapper.setup(&stmt).unwrap();
        assert!(matches!(setup, Setup::Named(_)));

        register_hoisted::<Late>();
        let mapper = Mapper::<Late>::new();
        assert!(matches!(mapper, Mapper::Hoisted(_)));
        let setup = mapper.setup(&stmt).unwrap();
        let Setup::Indexed(hoisted) = &setup else {
            panic!("a registered type resolves to its field indices");
        };
        assert_eq!(hoisted.idx[0].pos(), 1);

        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let mut row = Row::Borrowed(row, setup.row_cols());
        assert_eq!(mapper.map(&mut row, &setup).id, 7);
    }

    #[test]
    fn a_registered_type_is_mapped_through_resolved_indices() {
        register_hoisted::<Plain>();
        assert!(matches!(Mapper::<Plain>::new(), Mapper::Hoisted(_)));

        let conn = conn();
        // The columns come in the opposite order of the fields: resolution is by name.
        let mut stmt = conn
            .prepare_cached("SELECT name, id FROM t WHERE id = 7")
            .unwrap();
        let mapper = Mapper::<Plain>::new();
        let setup = mapper.setup(&stmt).unwrap();
        let Setup::Indexed(hoisted) = &setup else {
            panic!("a registered type resolves to its field indices");
        };
        assert_eq!((hoisted.idx[0].pos(), hoisted.idx[1].pos()), (1, 0));

        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let mut row = Row::Borrowed(row, setup.row_cols());
        let mapped = mapper.map(&mut row, &setup);
        assert_eq!(mapped.id, 7);
        assert_eq!(mapped.name, "seven");
    }

    /// The case-insensitive half of the resolution has to answer the same column on both paths.
    /// The borrowed path asks rusqlite, which scans the columns in order and takes the first name
    /// that matches; the owned path reads the map of a result set that travelled through the
    /// network layer, and that map iterates in hash order.  `name` and `NAME` are two keys of it,
    /// and the field of this test matches neither of them exactly.
    #[test]
    fn a_case_only_match_resolves_to_the_same_column_on_both_paths() {
        const NAMES: &FieldNames = &FieldNames::new(&[("NaMe", 0u16)]);
        let maps = vec![(NAMES, 0u16)];
        let names = ["id", "name", "NAME"];
        let sql = "SELECT id, name, id AS NAME FROM t WHERE id = 7";

        let conn = conn();
        let stmt = conn.prepare_cached(sql).unwrap();
        let borrowed = ColIdx::resolve_indexed(&maps, &stmt).unwrap();

        let cols = ColIdx::from_names(
            &names
                .iter()
                .map(|n| (*n).to_string())
                .collect::<Vec<String>>(),
        );
        let owned = cols.resolve_indexed_owned(&maps).unwrap();

        assert_eq!(
            borrowed[0].pos(),
            1,
            "the TEXT column is the first name match"
        );
        assert_eq!(
            owned[0].pos(),
            borrowed[0].pos(),
            "the same query resolves the same column on both paths"
        );

        // Both paths then read that TEXT column, not the integer the alias points at.
        let mut stmt = conn.prepare_cached(sql).unwrap();
        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let empty = ColIdx::empty();
        let mut row = Row::Borrowed(row, &empty);
        assert_eq!(row.get_idx::<String>(borrowed[0]), "seven");

        let mut row = Row::Owned(RowOwned::new(
            RowValues {
                values: names
                    .iter()
                    .map(|n| match *n {
                        "name" => Some(ValueOwned::Text("seven".to_string())),
                        _ => Some(ValueOwned::Integer(7)),
                    })
                    .collect::<Vec<Option<ValueOwned>>>(),
            },
            Arc::new(cols),
        ));
        assert_eq!(row.get_idx::<String>(owned[0]), "seven");
    }

    #[test]
    fn a_missing_column_names_the_field_it_belongs_to() {
        let conn = conn();
        let stmt = conn
            .prepare_cached("SELECT id FROM t WHERE id = 7")
            .unwrap();

        // The panicking family panics with this name, the non-panicking one answers
        // `Error::QueryParams` built from it.
        let name = ColIdx::resolve_indexed(&Plain::field_maps(), &stmt).unwrap_err();
        assert_eq!(name, "name");
    }

    /// A missing column is a verdict about the row that reads it, not about the query: the setup of
    /// a result set that has no rows carries the name, and nothing reports it while no row is
    /// mapped, which is what an empty result set has always answered with.
    #[test]
    fn an_empty_result_set_with_a_missing_column_still_answers_empty() {
        register_hoisted::<Plain>();
        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT id FROM t WHERE id = 0")
            .unwrap();
        let mapper = Mapper::<Plain>::new();
        let setup = mapper.setup(&stmt).unwrap();
        let Setup::Indexed(hoisted) = &setup else {
            panic!("a registered type resolves to its field indices");
        };
        assert_eq!(
            hoisted.missing.as_deref(),
            Some("name"),
            "the setup carries the field no column matched"
        );

        let mut rows = stmt.raw_query();
        assert!(
            rows.next().unwrap().is_none(),
            "this result set has no rows"
        );
    }

    #[test]
    #[should_panic(expected = "column 'name' is not in the result set")]
    fn a_row_reading_a_missing_column_panics_naming_that_field() {
        register_hoisted::<Plain>();
        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT id FROM t WHERE id = 7")
            .unwrap();
        let mapper = Mapper::<Plain>::new();
        let setup = mapper.setup(&stmt).unwrap();

        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let mut row = Row::Borrowed(row, setup.row_cols());
        mapper.map(&mut row, &setup);
    }

    /// The non-panicking family answers the same missing column as an [`Error::QueryParams`]
    /// naming it, and only for a row that reads it.
    #[test]
    fn a_try_row_reading_a_missing_column_reports_the_field_it_cannot_read() {
        register_hoisted_try::<TryPlain>();
        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT id FROM t WHERE id = 7")
            .unwrap();
        let mapper = MapperTry::<TryPlain>::new();
        let setup = mapper.setup(&stmt).unwrap();

        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let mut row = Row::Borrowed(row, setup.row_cols());
        let err = mapper.map(&mut row, &setup).unwrap_err();
        assert!(
            matches!(&err, Error::QueryParams(msg) if msg == "column 'name' is not in the result set"),
            "{err}"
        );
    }

    #[test]
    fn a_try_setup_of_an_empty_result_set_reports_nothing() {
        register_hoisted_try::<TryPlain>();
        let conn = conn();
        let stmt = conn
            .prepare_cached("SELECT id FROM t WHERE id = 0")
            .unwrap();
        let mapper = MapperTry::<TryPlain>::new();
        // Setting up a query that returns no rows cannot fail, whatever its fields are named.
        assert!(mapper.setup(&stmt).is_ok());
    }

    /// The happy path of the non-panicking family: every field has its column, and the row maps.
    #[test]
    fn a_try_row_maps_every_field_it_has() {
        register_hoisted_try::<TryPlain>();
        let conn = conn();
        let mut stmt = conn
            .prepare_cached("SELECT name, id FROM t WHERE id = 7")
            .unwrap();
        let mapper = MapperTry::<TryPlain>::new();
        let setup = mapper.setup(&stmt).unwrap();

        let mut rows = stmt.raw_query();
        let row = rows.next().unwrap().unwrap();
        let mut row = Row::Borrowed(row, setup.row_cols());
        let mapped = mapper.map(&mut row, &setup).unwrap();
        assert_eq!(mapped.id, 7);
        assert_eq!(mapped.name, "seven");
    }
}
#[cfg(test)]
mod shape_tests {
    use super::*;

    #[test]
    fn the_shape_of_columns_ignores_their_order_and_case() {
        assert_eq!(sql_shape(&["id", "ts"]), sql_shape(&["ts", "id"]));
        assert_eq!(sql_shape(&["ID", "ts"]), sql_shape(&["id", "TS"]));
    }

    #[test]
    fn a_different_set_of_columns_is_a_different_shape() {
        assert_ne!(sql_shape(&["id", "ts"]), sql_shape(&["id"]));
        assert_ne!(sql_shape(&["id", "ts"]), sql_shape(&["id", "ts2"]));
        // The count of the names is part of the value, so one name is never the shape of two.
        assert_ne!(sql_shape(&["id"]), sql_shape(&["id", "id"]));
    }

    #[test]
    fn a_flattened_type_merges_the_shapes_of_its_parts() {
        let own = sql_shape(&["id"]);
        let inner = sql_shape(&["x", "y"]);
        assert_eq!(
            own.wrapping_add(inner),
            sql_shape(&["id", "x", "y"]),
            "a flattened type is checked against all the columns its parts read"
        );
    }

    const PARENT: u64 = sql_shape(&["id", "name_db", "desc", "vec_wrap"]);
    const PART: u64 = sql_shape(&["sub_id", "sub_name", "url", "uuid"]);
    /// The merge the derive performs, evaluated at compile time: a debug build must not stop here.
    const MERGED: u64 = PARENT.wrapping_add(PART);
    /// The reason it is a `wrapping_add`: these column names overflow a plain `+`, and a debug build
    /// that evaluated one would stop the compile of a `sql_map!()` that has nothing wrong with it.
    const _: () = assert!(PARENT > u64::MAX - PART);

    #[test]
    fn a_merged_shape_is_summed_with_wrapping_addition() {
        assert_eq!(
            MERGED,
            sql_shape(&[
                "id", "name_db", "desc", "vec_wrap", "sub_id", "sub_name", "url", "uuid"
            ]),
            "the merge of the two parts is the shape of all their columns"
        );
    }

    #[test]
    fn the_shape_algorithm_is_pinned() {
        // The value a `sql_map!()` comparison is built from: changing the algorithm here changes
        // every shape, so it is pinned to a literal.
        assert_eq!(sql_shape(&["id", "ts"]), 0x8925_040f_6ac7_0230);
    }
}
