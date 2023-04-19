use polars::prelude::ToDummies;
use polars::prelude::*;
use polars::series::Series;

use tracing::{event, Level};

// type DummyType = u8;
// type DummyCa = UInt8Chunked;
type DummyType = i32;
type DummyCa = Int32Chunked;

pub struct CategoryField {
    inner: Series,
}
impl std::ops::Deref for CategoryField {
    type Target = Series;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl From<Series> for CategoryField {
    fn from(series: Series) -> Self {
        CategoryField { inner: series }
    }
}
impl ToDummies for CategoryField {
    fn to_dummies(&self, separator: Option<&str>) -> PolarsResult<DataFrame> {
        let sep = separator.unwrap_or("_");
        let col_name = self.name();
        // generate an index that points to where each group value lives
        // in the series.
        let groups = self.group_tuples(true, false)?;

        let levels: Series = unsafe { self.agg_first(&groups) };

        // safety: groups are in bounds
        let columns: Vec<Series> = levels
            .iter()
            .zip(groups.iter())
            .skip(1) // skip the first to generate k - 1 dummies
            .map(|(av, group)| {
                // strings are formatted with extra \" \" in polars, so we
                // extract the string
                let name = if let Some(s) = av.get_str() {
                    event!(Level::DEBUG, "👉 group str: {:?}", &s);
                    format!("{col_name}{sep}{s}")
                } else {
                    // other types don't have this formatting issue
                    event!(Level::DEBUG, "👉 group: {:?}", &av);
                    format!("{col_name}{sep}{av}")
                };

                let ca = match group {
                    GroupsIndicator::Idx((_, group)) => {
                        dummies_helper_idx(group, self.len(), &name)
                    }
                    GroupsIndicator::Slice([offset, len]) => {
                        dummies_helper_slice(offset, len, self.len(), &name)
                    }
                };
                ca.into_series()
            })
            .collect();

        event!(
            Level::INFO,
            "👉 counts levels: {} dummies: {}",
            &groups.len(),
            &columns.len()
        );
        Ok(DataFrame::new_no_checks(sort_columns(columns)))
    }
}

fn dummies_helper_idx(groups: &[IdxSize], len: usize, name: &str) -> DummyCa {
    let mut av = vec![0 as DummyType; len];

    for &idx in groups {
        let elem = unsafe { av.get_unchecked_mut(idx as usize) };
        *elem = 1;
    }

    ChunkedArray::from_vec(name, av)
}

fn dummies_helper_slice(
    group_offset: IdxSize,
    group_len: IdxSize,
    len: usize,
    name: &str,
) -> DummyCa {
    let mut av = vec![0 as DummyType; len];

    for idx in group_offset..(group_offset + group_len) {
        let elem = unsafe { av.get_unchecked_mut(idx as usize) };
        *elem = 1;
    }

    ChunkedArray::from_vec(name, av)
}

fn sort_columns(mut columns: Vec<Series>) -> Vec<Series> {
    columns.sort_by(|a, b| a.name().partial_cmp(b.name()).unwrap());
    columns
}
