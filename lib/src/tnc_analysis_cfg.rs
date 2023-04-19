use serde::{Deserialize, Serialize};

/// Wrapper for a wide range of configurations.
///
#[derive(Debug, Serialize, Deserialize)]
pub struct Config<T> {
    inner: T,
}
impl<T> Config<T>
where
    for<'de> T: Deserialize<'de>,
{
    pub fn new(value: T) -> Self {
        Config { inner: value }
    }
}
impl<T> std::ops::Deref for Config<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> std::ops::DerefMut for Config<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
///
/// Configuration for the Propensity Score, a derived field. This needs to be coordinated
/// with the [`crate::propensity::PropensityCfg`].
///
/// # Example
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tnc_analysis_lib::tnc_analysis_cfg::PropensityScore;
///
/// let json = r#"{
///      "binary-target-field-tag": "reach",
///      "predictors": [
///        "q_specialty",
///        "Meatype::decile"
///      ],
///      "bins": {
///        "count": 5,
///        "ranges": [
///          { "start": 0.0, "stop": 0.2 },
///          { "start": 0.2, "stop": 0.4 },
///          { "start": 0.4, "stop": 0.6 },
///          { "start": 0.6, "stop": 0.8 },
///          { "start": 0.8, "stop": 1.0 }
///        ],
///        "generator": { "type": "EqualRange" }
///      }
///   }"#;
/// let model: PropensityScore = serde_json::from_str(&json).unwrap();
/// assert!(model.binary_target_field_tag == "reach");
/// assert!(model.bins.count == 5 as usize);
///
/// let json = r#"{
///      "type": "propensity-score",
///      "binary-target-field-tag": "reach",
///      "predictors": [],
///      "bins": {
///        "count": 5,
///        "ranges": [
///          { "start": 0.0, "stop": 0.2 },
///          { "start": 0.2, "stop": 0.4 },
///          { "start": 0.4, "stop": 0.6 },
///          { "start": 0.6, "stop": 0.8 },
///          { "start": 0.8, "stop": 1.0 }
///        ],
///        "generator": { "type": "EqualRange" }
///      }
///   }"#;
/// let model: PropensityScore = serde_json::from_str(&json).unwrap();
/// assert!(model.binary_target_field_tag == "reach");
/// assert!(model.predictors.is_empty());
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct PropensityScore {
    #[serde(rename = "binary-target-field-tag")]
    pub binary_target_field_tag: SearchTerm,
    pub predictors: Vec<SearchTerm>,
    pub bins: Bins,
}
type SearchTerm = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct Bins {
    pub count: usize,
    pub ranges: Vec<Range>,
    pub generator: BinGenerators,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BinGenerators {
    EqualRange,
    EqualCount,
    Custom,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Range {
    pub start: f64,
    pub stop: f64,
}
