//! Row decoding helpers shared by the modules.

use std::str::FromStr;

use sqlx::Row;
use sqlx::postgres::PgRow;

use crate::error::TaskError;

pub(crate) fn parse_col<T>(row: &PgRow, column: &str) -> Result<T, TaskError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let text: String = row.try_get(column)?;
    text.parse::<T>()
        .map_err(|e| TaskError::Corrupt(format!("{column}: {e}")))
}
