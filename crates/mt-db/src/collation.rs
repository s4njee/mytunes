use icu_collator::Collator;
use icu_collator::options::{CollatorOptions, Strength};
use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;

use crate::DbError;

pub(crate) fn register(db: &Connection) -> Result<(), DbError> {
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Secondary);
    let collator = Collator::try_new(Default::default(), options)
        .map_err(|error| DbError::Collation(error.to_string()))?;
    db.create_collation("mytunes_unicode_v1", move |left, right| {
        collator.compare(left, right)
    })?;
    db.create_scalar_function(
        "mytunes_artist_v1",
        1,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |context| {
            let name: Option<String> = context.get(0)?;
            Ok(name.map(|name| match name.split_at_checked(4) {
                Some((article, rest))
                    if article.eq_ignore_ascii_case("the ") && !rest.is_empty() =>
                {
                    rest.to_owned()
                }
                _ => name,
            }))
        },
    )?;
    Ok(())
}
