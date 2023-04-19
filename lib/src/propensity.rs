use crate::Mask;
use color_eyre::eyre::Result;
use polars::prelude::*;
// use polars::prelude::DataFrameOps::columns_to_dummies;
use tracing::{event, Level};

///
/// Host how to engage the logit optimization lib.  Returns a Series.
/// Use this to run the propensity method.
///
/// 🔑 Values must not borrow from elsewhere.  Use the builder -> cfg to
///    copy values as needed.
///
#[derive(Debug)]
pub struct PropensityCfg {
    pub target: String,
    pub predictors: Vec<String>,
    // pub mask: Option<Mask<'a>>,
    pub bin_count: Option<u32>,
    pub name: Option<String>,
}
// Some(self.column("include").unwrap().bool().unwrap()),

///
/// Phased build of parameters required to run and append the propensity score.
///
pub struct PropensityCfgBuilder<'a> {
    pub target: BinaryTarget<'a>,
    pub predictors: Predictors<'a>,
    mask: Option<Mask<'a>>,
    bin_count: Option<u32>,
    name: Option<&'a str>,
}

impl<'a> PropensityCfgBuilder<'a> {
    pub fn new(target: BinaryTarget<'a>, predictors: Predictors<'a>) -> Self {
        PropensityCfgBuilder {
            target,
            predictors,
            mask: None,
            bin_count: None,
            name: Some("propensity"),
        }
    }

    pub fn bin_count(mut self, bin_count: u32) -> Self {
        self.bin_count = Some(bin_count);
        self
    }

    pub fn with_name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    ///
    /// Break all dependencies on Matrix, the struct this config is being used to mutate.
    /// NEXT HERE
    ///
    pub fn build(self) -> PropensityCfg {
        let predictors: Vec<String> = self.predictors.iter().map(|p| p.to_string()).collect();
        let name = match self.name {
            None => None,
            Some(n) => Some(n.to_string()),
        };

        PropensityCfg {
            target: self.target.to_string(),
            predictors,
            // mask: self.mask,
            bin_count: self.bin_count,
            name,
        }
    }
}

///
/// Entry point for building the PropensityCfg.  See exit: .build().
///
impl<'a> PropensityCfg {
    pub fn builder(
        target: BinaryTarget<'a>,
        predictors: Predictors<'a>,
    ) -> PropensityCfgBuilder<'a> {
        PropensityCfgBuilder::new(target, predictors)
    }
}
///
/// Owned versions for use in the final configuration
/// Required b/c configuration cannot borrow from matrix that is being mutated.
///
#[derive(Debug)]
pub struct BinaryTargetOwned {
    inner: FieldNameOwned,
}
#[derive(Debug)]
pub struct PredictorsOwned {
    inner: Vec<FieldNameOwned>,
}
type FieldNameOwned = String;
///
/// Wrappers to String values
pub struct BinaryTarget<'a> {
    inner: FieldName<'a>,
}
impl<'a> std::convert::From<FieldName<'a>> for BinaryTarget<'a> {
    fn from(fieldname: FieldName<'a>) -> Self {
        BinaryTarget { inner: fieldname }
    }
}
impl<'a> std::ops::Deref for BinaryTarget<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<'a> std::convert::AsRef<str> for BinaryTarget<'a> {
    fn as_ref(&self) -> &str {
        &self.inner
    }
}
impl<'a> From<Vec<FieldName<'a>>> for BinaryTarget<'a> {
    fn from(vec: Vec<FieldName<'a>>) -> Self {
        debug_assert!(
            vec.len() == 1,
            "The attempt to construct BinaryTarget failed"
        );
        BinaryTarget {
            inner: vec.first().unwrap(),
        }
    }
}
impl<'a> std::fmt::Debug for BinaryTarget<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Binary target: {}", self.inner)?;
        Ok(())
    }
}
/// Must be implemented because used by polars to lookup the column name.
impl<'a> std::fmt::Display for BinaryTarget<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner.fmt(f)
    }
}
#[derive(Debug)]
pub struct Predictors<'a> {
    inner: Vec<FieldName<'a>>,
}

impl<'a> std::convert::Into<Vec<&'a str>> for Predictors<'a> {
    fn into(self) -> Vec<&'a str> {
        self.inner
    }
}
impl<'a> std::convert::AsRef<Vec<&'a str>> for Predictors<'a> {
    fn as_ref(&self) -> &Vec<&'a str> {
        &self.inner
    }
}
impl<'a> std::ops::Deref for Predictors<'a> {
    type Target = Vec<FieldName<'a>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<'a> From<Vec<FieldName<'a>>> for Predictors<'a> {
    fn from(vec: Vec<FieldName<'a>>) -> Self {
        Predictors { inner: vec }
    }
}
/// read-only iterator
impl<'a> Iterator for &'a Predictors<'a> {
    type Item = &'a FieldName<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.iter().next()
    }
}
// 🚧 Not sure if this makes sense
impl<'a> From<&Vec<FieldName<'a>>> for Predictors<'a> {
    fn from(vec: &Vec<FieldName<'a>>) -> Self {
        Predictors {
            inner: vec.to_vec(),
        }
    }
}
impl<'a> std::fmt::Display for Predictors<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Predictors:")?;
        for field_name in &self.inner {
            writeln!(f, "\t{}", field_name)?;
        }
        Ok(())
    }
}

///
pub type FieldName<'a> = &'a str;

///
/// Enables building dummies outside of the format matrix construct.  Useful for temporary builds.
/// Introduces dependency on nalgebra for the propensity module.
///
/// Default is to convert all utf8 type columns of data to dummy columns.
///
pub fn build_dummies(
    df: DataFrame,
    columns: Option<Predictors<'_>>,
    separator: Option<&str>,
) -> Result<DataFrame> {
    event!(Level::INFO, "🧮 Building dummies for X");
    // for each field of type DataType::Utf8 build out the dummy
    let hold_fields: Vec<String>;

    let fields: Vec<&str> = match columns {
        None => {
            hold_fields = df
                .fields()
                .iter()
                .filter(|f| match f.data_type() {
                    polars::datatypes::DataType::Utf8 => true,
                    _ => false,
                })
                .map(|f| f.name().to_string())
                .collect();

            let fields: Vec<&str> = hold_fields.iter().map(|f| f.as_str()).collect();
            fields
        }
        Some(cs) => cs.into(),
    };

    event!(Level::DEBUG, "dummies for fields:\n{:#?}", &fields);
    let df = df.columns_to_dummies(fields, separator)?;

    Ok(df)
}
