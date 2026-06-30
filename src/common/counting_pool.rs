use std::fmt;
use std::ops::Deref;

use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use sqlx::{
    postgres::{PgQueryResult, PgRow, PgStatement, PgTypeInfo},
    Describe, Execute, Executor, PgPool, Postgres,
};

use crate::common::call_counter;

/// A thin wrapper around `PgPool` that increments the per-request DB call
/// counter on every query execution. When called outside an HTTP request scope
/// (e.g. from the worker or scheduler) the counter increment is a silent no-op.
#[derive(Clone)]
pub struct CountingPool(PgPool);

impl CountingPool {
    pub fn new(pool: PgPool) -> Self {
        Self(pool)
    }

    /// Begin a transaction, counting it as a DB call.
    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, Postgres>, sqlx::Error> {
        call_counter::inc_db();
        self.0.begin().await
    }
}

impl From<PgPool> for CountingPool {
    fn from(pool: PgPool) -> Self {
        Self(pool)
    }
}

impl fmt::Debug for CountingPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CountingPool").finish_non_exhaustive()
    }
}

// Deref lets callers pass `&CountingPool` where `&PgPool` is needed (e.g. pool
// options, type-info queries) without copying boilerplate — but note that Rust's
// method resolution will prefer the inherent `Executor` impl below over the
// deref'd `PgPool` impl, so queries DO go through our counter.
impl Deref for CountingPool {
    type Target = PgPool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'c> Executor<'c> for &'c CountingPool {
    type Database = Postgres;

    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<PgQueryResult, PgRow>, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Postgres>,
    {
        call_counter::inc_db();
        (&self.0).fetch_many(query)
    }

    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<PgRow>, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Postgres>,
    {
        call_counter::inc_db();
        (&self.0).fetch_optional(query)
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [PgTypeInfo],
    ) -> BoxFuture<'e, Result<PgStatement<'q>, sqlx::Error>>
    where
        'c: 'e,
    {
        (&self.0).prepare_with(sql, parameters)
    }

    #[doc(hidden)]
    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<Postgres>, sqlx::Error>>
    where
        'c: 'e,
    {
        (&self.0).describe(sql)
    }
}
