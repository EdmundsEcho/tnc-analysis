pub(crate) mod config;
pub(crate) mod header;
pub(crate) mod matrix;
pub(crate) mod propensity;
pub(crate) mod tnc_analysis_cfg;

pub mod prelude {
    pub use crate::config::FieldNamesCfg;
    pub use crate::matrix::Matrix;
    pub use crate::propensity::PropensityCfg;
    pub use crate::read_config;
    pub use crate::tnc_analysis_cfg::Config;
}

use nom::branch::alt;
use nom::bytes::complete::{tag, take_until1};
use tracing::{event, Level};

use crate::config::FieldNamesCfg;

use polars::prelude::{BooleanType, ChunkedArray, DataFrame, DataType, NamedFrom, Series};

///!
///! Host functional style methods and trivial type definitions.
///!
type FieldName<'a> = &'a str;
// pub(crate) type Mask<'a> = &'a ChunkedArray<BooleanType>;
pub(crate) type Mask<'a> = &'a ChunkedArray<BooleanType>;

///
/// These fuzzy lookups depend on how the tnc app names fieldnames. Coordination is facilitated
/// use a configuration for the App.
///
/// Note: need to convert to bytes to avoid a separate memory allocation?
///
fn get_fuzzy_binary_target<'a>(
    fields: Vec<FieldName<'a>>,
    cfg: FieldNamesCfg,
) -> Vec<FieldName<'a>> {
    fields
        .iter()
        .filter(|field| filter_fuzzy_binary_target(cfg.clone())(field.as_bytes()))
        .copied()
        .collect()
}
///
/// These fuzzy lookups depend on how the tnc app names fieldnames. Coordination is facilitated
/// use a configuration for the App.
///
fn get_fuzzy_predictors<'a>(fields: Vec<FieldName<'a>>, cfg: FieldNamesCfg) -> Vec<FieldName<'a>> {
    fields
        .iter()
        .filter(|field| filter_fuzzy_predictors(cfg.clone())(field.as_bytes()))
        .copied()
        .collect()
}
/// Depends on const values that find quality and derived fields
///
/// 🚧 todo: remove dependency on const.  Use configuration instead.
///
fn filter_fuzzy_predictors<'a>(cfg: FieldNamesCfg) -> impl FnMut(&[u8]) -> bool + 'a {
    move |try_this| {
        alt((
            tag::<_, _, nom::error::VerboseError<&[u8]>>(cfg.quality_field_tag.as_str()),
            take_until1::<_, _, nom::error::VerboseError<&[u8]>>(cfg.derived_field_tag.as_str()),
        ))(try_this)
        .map(|_| true)
        .is_ok()
    }
}
/// Depends on const values that find quality and derived fields
fn filter_fuzzy_binary_target<'a>(cfg: FieldNamesCfg) -> impl FnMut(&[u8]) -> bool + 'a {
    move |try_this| {
        alt((
            tag::<_, _, nom::error::VerboseError<&[u8]>>(cfg.binary_target_field_tag.as_str()),
            take_until1::<_, _, nom::error::VerboseError<&[u8]>>(
                cfg.binary_target_field_tag.as_str(),
            ),
        ))(try_this)
        .map(|_| true)
        .is_ok()
    }
}
///
/// convert a pre-processed data frame that is safe to transpose, and store
/// into a single `Vec<N>`.  Used to build X for the propensity score.
///
/// dependencies & "side-effect?"
/// * bias or intercept slot will be added to the last column
///
pub fn to_row_dominant(df: &DataFrame) -> Result<(Vec<f64>, usize)> {
    let (rows, cols) = df.shape();
    event!(
        Level::DEBUG,
        "🦀 to_row_dominant rows: {rows}, cols: {cols}"
    );
    // create a longer-lived reference to Vec<Series>
    // iterate over the dataframe to cast each column to Float64
    let mut col_iters: Vec<Series> = df
        .iter()
        .map(|s| s.cast(&DataType::Float64).expect("Cast to Float64 failed"))
        .collect();
    col_iters.push(Series::new("bias_slot", vec![1.0; rows]));

    // iterate over each Series to create an iterator with Item: Option<f64>
    let col_iters = col_iters
        .iter()
        .map(|s| Ok(s.f64()?.into_iter()))
        .collect::<Result<Vec<_>>>()?;

    // build the 1D arrays
    let cols = col_iters.len();

    // build the sized buffer for the 1D array
    let mut data: Vec<f64> = vec![0.0; rows * cols];

    // use the number of columns to set the step size
    // shift each iteration by idx columns
    for (idx, c_iter) in col_iters.into_iter().enumerate() {
        event!(Level::DEBUG, " SKIP  👉 idx: {idx} of {cols}");
        // consume a column, iter[c]
        data.iter_mut()
            .skip(idx)
            .step_by(cols)
            .zip(c_iter)
            .for_each(|(d, c_value)| -> () {
                match c_value {
                    None => panic!("c_value: None"),
                    Some(c) => *d = c,
                }
            });
    }
    assert!(data[cols - 1] == 1.0, "first value is not as expected");

    /*
    // set vec with capacity
    let mut data2: Vec<f64> = Vec::with_capacity(rows * cols);

    // iterate over each Series to create an iterator with Item: Option<f64>
    let mut col_iters = iters
        .iter()
        .map(|s| Ok(s.f64()?.into_iter()))
        .collect::<Result<Vec<_>>>()?;

    // for each row
    for _ in 0..rows {
        // move each column iter down one using next
        for iter in &mut col_iters {
            let value = iter
                .next()
                // .map_or(Ok(None), |r| Ok(r.map(Some)))
                .ok_or_else(|| "each iter should have as many items as rows")
                .map_err(|e| eyre!("{}", e))?;
            data2.push(value.unwrap() as f64);
        }
    }
    assert!(data2[cols - 1] == 1.0, "first value is not as expected");

    let comparison = zip(&data, &data2).into_iter().enumerate().fold(
        vec![(0, 0.0, 0.0)],
        |mut acc, (idx, (d1, d2))| match d1 == d2 {
            false => {
                acc.push((idx, *d1, *d2));
                acc
            }
            true => acc,
        },
    );

    event!(Level::INFO, " ❌ NO EQ: {:?}", comparison);
    assert!(data2 == data, "The two data are one in the same");
    */

    Ok((data, rows))
}
// -------------------------------------------------------------------------------------------------
// Configuration
use crate::tnc_analysis_cfg::Config;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use serde::Deserialize;
use std::fs::File;
use std::path::Path;

/*
lazy_static::lazy_static! {
    // default value that can be changed using set_app_config
    pub static ref APP_CONFIG: RwLock<Config<FieldNamesCfg>> = RwLock::new(read_config(APP_CFG).expect("Failed to read the default app config"));
}

*/
pub fn read_config<T>(path: &str) -> Result<Config<T>>
where
    for<'de> T: Deserialize<'de>,
{
    let path = Path::new(path);
    let cfg: File = File::open(path).wrap_err_with(|| format!("Failed {}", path.display()))?;
    let cfg: T = serde_json::from_reader(cfg)
        .wrap_err_with(|| format!("Failed to parse: {}", path.display()))?;
    Ok(Config::new(cfg))
}

/*
pub fn set_app_config(cfg: Config<FieldNamesCfg>) {
    let mut write_lock = FIELD_NAME_CFG.write().unwrap();
    *write_lock = cfg;
} */
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    const FIELDS: [&str; 10] = [
        "subject_idx",
        "q_innetwork",
        "q_specialty",
        "q_state",
        "MeaType::m_reach.time::28_35",
        "MeaType::m_unitcount.product::A.time::0_23",
        "MeaType::m_unitcount.product::C.time::0_23",
        "MeaType::m_unitcount.product::A.time::14",
        "MeaType::m_unitcount.product::A.time::15",
        "MeaType::m_unitcount.product::C.time::0_23.derivedField::decile",
    ];
    #[test]
    fn test_get_fuzzy_predictors() {
        let result = get_fuzzy_predictors(FIELDS.to_vec(), None);
        assert!(result.len() == 4);
    }
    #[test]
    fn test_get_fuzzy_binary_target() {
        let result = get_fuzzy_binary_target(FIELDS.to_vec(), None);
        assert!(result.len() == 1);
    }
}
