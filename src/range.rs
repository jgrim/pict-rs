use crate::{
    error::{Error, UploadError},
    store::Store,
};
use actix_web::{
    http::header::{ByteRangeSpec, ContentRange, ContentRangeSpec, Range},
    web::Bytes,
};
use futures_util::stream::{once, Stream};
use std::future::ready;

pub(crate) fn chop_bytes(
    byte_range: &ByteRangeSpec,
    bytes: Bytes,
    length: u64,
) -> Result<impl Stream<Item = Result<Bytes, Error>>, Error> {
    if let Some((start, end)) = byte_range.to_satisfiable_range(length) {
        // END IS INCLUSIVE
        let end = end as usize + 1;
        return Ok(once(ready(Ok(bytes.slice(start as usize..end)))));
    }

    Err(UploadError::Range.into())
}

pub(crate) async fn chop_store<S: Store>(
    byte_range: &ByteRangeSpec,
    store: &S,
    identifier: &S::Identifier,
    length: u64,
) -> Result<impl Stream<Item = std::io::Result<Bytes>>, Error>
where
    Error: From<S::Error>,
{
    if let Some((start, end)) = byte_range.to_satisfiable_range(length) {
        // END IS INCLUSIVE
        let end = end + 1;
        return Ok(store
            .to_stream(identifier, Some(start), Some(end.saturating_sub(start)))
            .await?);
    }

    Err(UploadError::Range.into())
}

pub(crate) fn single_bytes_range(range: &Range) -> Option<&ByteRangeSpec> {
    if let Range::Bytes(ranges) = range {
        if ranges.len() == 1 {
            return ranges.get(0);
        }
    }

    None
}

pub(crate) fn to_content_range(
    byte_range: &ByteRangeSpec,
    instance_length: u64,
) -> Option<ContentRange> {
    byte_range
        .to_satisfiable_range(instance_length)
        .map(|range| {
            ContentRange(ContentRangeSpec::Bytes {
                range: Some(range),
                instance_length: Some(instance_length),
            })
        })
}
