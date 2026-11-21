//! [`text_enum!`]: stores a closed set of cases as the text the schema's `CHECK` lists, so the
//! database stays readable and a case can't be confused with a number.

/// Implements `as_str`, `ToSql` and `FromSql` for a field-less enum, mapping each case to its
/// text. Text the enum doesn't know reads as an error, not a default.
macro_rules! text_enum {
    ($name:ident { $($case:ident => $text:literal),+ $(,)? }) => {
        impl $name {
            /// How the database stores it.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$case => $text),+
                }
            }
        }

        impl rusqlite::types::ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
                Ok(self.as_str().into())
            }
        }

        impl rusqlite::types::FromSql for $name {
            fn column_result(
                value: rusqlite::types::ValueRef<'_>,
            ) -> rusqlite::types::FromSqlResult<Self> {
                match value.as_str()? {
                    $($text => Ok(Self::$case),)+
                    other => Err(rusqlite::types::FromSqlError::Other(
                        format!("unknown {} `{other}`", stringify!($name)).into(),
                    )),
                }
            }
        }
    };
}

pub(crate) use text_enum;
