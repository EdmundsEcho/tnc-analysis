use color_eyre::eyre::{eyre, Result};
use std::fmt;
use tracing::{event, Level};

use polars::prelude::*;

use crate::header::Header;
// use crate::to_dummies::CategoryField;
use crate::propensity::{build_dummies, BinaryTarget, Predictors, PropensityCfg};
use crate::to_row_dominant;
use crate::FieldNamesCfg;
use crate::Mask;
use crate::{get_fuzzy_binary_target, get_fuzzy_predictors};

use propensity_score::prelude::*;

// temporary
const OUT_FILE: &str = "./res/logit-data.parquet";

/// Transient type mostly for displaying with proper description. Utilized by [`Matrix::get_logit_columns`].
#[derive(Debug)]
pub struct LogitColumns<'a> {
    inner: Vec<&'a str>,
}
impl<'a> LogitColumns<'a> {
    fn target(&self) -> &str {
        &self.inner[0]
    }
    fn predictors(&self) -> &[&str] {
        &self.inner[1..]
    }
}
impl<'a> From<Vec<&'a str>> for LogitColumns<'a> {
    fn from(vec: Vec<&'a str>) -> Self {
        assert!(!vec.is_empty(), "Vector cannot be empty");
        LogitColumns { inner: vec }
    }
}
impl<'a> From<&[&'a str]> for LogitColumns<'a> {
    fn from(slice: &[&'a str]) -> Self {
        assert!(!slice.is_empty(), "Slice cannot be empty");
        LogitColumns {
            inner: slice.to_vec(),
        }
    }
}
impl<'a> fmt::Display for LogitColumns<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Logit target: {}", self.target())?;
        writeln!(f, "Predictors: {:?}", self.predictors())
    }
}
impl<'a> std::ops::Deref for LogitColumns<'a> {
    type Target = Vec<&'a str>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

///
/// Main struct.  Hosts the data required to move through the different phases of the TNC analysis.
///
#[derive(Debug)]
pub struct Matrix<T> {
    pub inner: T,
}
///
/// Use a separate X, y data structure specified in the linear-optimization package.
///
#[deprecated]
#[derive(Debug)]
pub struct LogitData {}

impl From<DataFrame> for Matrix<DataFrame> {
    fn from(df: DataFrame) -> Self {
        Matrix::<DataFrame>::new(df)
    }
}
impl std::ops::Deref for Matrix<DataFrame> {
    type Target = DataFrame;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Matrix<DataFrame> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Matrix<DataFrame> {
    fn new(inner: DataFrame) -> Self {
        Matrix { inner }
    }
    ///
    /// Converts the predictors in the matrix to a 1D array.  The dummy variables need to be built here
    /// to avoid having to reference them post instantiation.
    ///
    /// Dependency: User knows the original column names, pre-dummie making.
    ///
    pub fn to_row_dominant(
        &self,
        columns: &Vec<String>,
        // mask: Option<&Mask<'a>>,
    ) -> Result<(Vec<f64>, usize)> {
        // select from the dataframe
        /*
        let df = match mask {
            Some(mask) => self.filter(mask)?.select(columns)?,
            None => self.select(columns)?,
        }; */
        event!(Level::DEBUG, "Columns sent to build X? {:?}", &columns);

        let df = self.select(columns)?;
        let df = build_dummies(df, None, None)?;

        to_row_dominant(&df)
    }
    pub fn from_file(path: &str, header: Option<Header>) -> Result<Matrix<DataFrame>> {
        // The header is either provided or sourced from the file
        let has_header = header.is_none();
        let dd: DataFrame = CsvReader::from_path(path)?
            .has_header(has_header)
            .finish()?
            .lazy()
            .collect()?;
        Ok(dd.into())
    }
    ///
    /// Appends a propensity field to the Matrix. Requires a configuration.
    ///
    /// ```rust
    /// pub struct PropensityCfg<'a> {
    ///     pub target: BinaryTarget<'a>,
    ///     pub predictors: Predictors<'a>,
    ///     pub bin_count: Option<u32>,
    ///     pub name: Option<&'a str>,
    /// }
    /// ```
    ///
    pub fn with_propensity(&mut self, cfg: PropensityCfg) -> Result<()> {
        event!(Level::DEBUG, "📋 logit cfg:\n{:?}", &cfg,);

        //
        // build the data needed to run the logit
        // to_row_dominant will append intercept
        //
        // TODO: MAKE SURE NO NULLS
        // let (x, row_count) = self.to_row_dominant(&cfg.predictors, cfg.mask.as_ref())?;
        let (x, row_count) = self.to_row_dominant(&cfg.predictors /* None */)?;
        event!(Level::INFO, "✅ X as 1D with row count:{}", row_count);
        event!(Level::DEBUG, "{:#?}", self.show_meta()?);
        // build y
        let y: &Series = self.column(&cfg.target.as_str())?;
        let y = y
            .cast(&DataType::Float64)
            .expect("Cast to Float64 failed")
            .f64()?
            .into_iter()
            .map(|v| -> Result<f64> {
                v.ok_or_else(|| eyre!("Null value error")).map_err(|e| {
                    eyre!("Failed to cast {} to f64 binary target: {}", &cfg.target, e)
                })
            })
            .collect::<Result<Vec<_>>>()?;

        event!(
            Level::INFO,
            "✅ Y {}, as Vec<_> with len: {}",
            &cfg.target,
            y.len()
        );
        debug_assert!(
            y.len() == row_count,
            "The y and X logit inputs have different row counts"
        );

        // .map(|s| Ok(s.f64()?.into_iter()))
        let objective = Objective::from_vecs(x, y, row_count)?;

        let cfg = CfgBuilder::new().max_iters(100).logging(false).build();

        let findings = logit::run(&objective, cfg)?;

        event!(Level::INFO, "\n📋 logit findings\n{}", findings.report()?);

        // create a prediction and append to the matrix
        self.with_column(Series::new(
            "propensity",
            Vec::from(
                findings.predict(false), // binary = false, show_sample
            ),
        ))?;

        Ok(())
    }
    pub fn write_to_file_csv<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<()> {
        let mut file = std::fs::File::create(path)?;
        CsvWriter::new(&mut file).finish(self)?;
        Ok(())
    }
    pub fn header(&self) -> Header<'_> {
        Header::new(self.get_column_names())
    }
    /// Make this part of the initialization sequence
    pub fn with_include_tag(&mut self) -> (usize, usize) {
        let include_col = Series::new("include", self.column("subject_idx").unwrap().is_not_null());
        self.with_column(include_col).unwrap();

        let null_count: usize = self.column("subject_idx").unwrap().null_count();
        let mask = self.column("include").unwrap().bool().unwrap();
        let included = self.filter(mask).unwrap().height();
        let excluded = self.height() - self.filter(mask).unwrap().height();

        // let excluded_count = ec.height();
        debug_assert!(null_count == excluded, "The excluded column is flawed");

        (included, excluded)
    }
    /// predictors
    /// Use "q_" & "derived" in the Matrix.  Override with cfg.
    /// Used to specify the Logit model
    pub fn predictors(&self, cfg: FieldNamesCfg) -> Predictors<'_> {
        get_fuzzy_predictors(self.get_column_names(), cfg).into()
    }
    pub fn binary_target(&self, cfg: FieldNamesCfg) -> BinaryTarget<'_> {
        get_fuzzy_binary_target(self.get_column_names(), cfg).into()
    }
    /// write so that target-binary is the first arrow
    pub fn write_to_file<P: AsRef<std::path::Path>>(&mut self, path: Option<P>) -> Result<()> {
        let mut file = match path {
            None => std::fs::File::create(OUT_FILE)?,
            Some(value) => std::fs::File::create(value)?,
        };
        ParquetWriter::new(&mut file).finish(self)?;
        Ok(())
    }
    pub fn show_meta(&self) -> Result<String> {
        let report = format!(
            r#"
-----------------------------------
shape: {:#?}
schema: {:#?}
-----------------------------------
"#,
            self.shape(),
            self.schema(),
        );
        Ok(report)
    }
    pub fn show_fields(&self) -> Result<String> {
        Ok(format!("Fields\n{:#?}", self.fields()))
    }
}
// -------------------------------------------------------------------------------------
// transient type for printing
pub struct ModelDef<'a> {
    dependent: &'a str,
    predictors: Vec<&'a str>,
}
impl<'a> From<(&'a str, Vec<&'a str>)> for ModelDef<'a> {
    fn from((dependent, predictors): (&'a str, Vec<&'a str>)) -> Self {
        ModelDef {
            dependent,
            predictors,
        }
    }
}
impl<'a> fmt::Display for ModelDef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ~ {}", self.dependent, self.predictors.join(" + "))
    }
}
// -------------------------------------------------------------------------------------
// debug - show type
//
/*
fn print_type_of<T>(_: &T) {
    println!("{}", std::any::type_name::<T>())
}

fn print_type_name<T>(_: T) {
    println!("type: {:?}", std::any::type_name::<T>());
}

fn print_ref_type_name<T: ?Sized>(_: &T) {
    println!("ref type: {:?}", std::any::type_name::<T>());
}
*/
// -------------------------------------------------------------------------------------
#[derive(Debug)]
pub struct LogitFindings {
    pub intercept: f64,
    pub coefficients: Vec<f64>,
}
