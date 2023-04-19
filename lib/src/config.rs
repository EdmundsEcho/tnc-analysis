use serde::Deserialize;

///
/// Specifies how to bridge how the fields are named in the graphql service to the fields required
/// to conduct a Test and Control analysis.
///
/// .json file must follow this structure (or vice-versa)
///
///
/// # Examples
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tnc_analysis_lib::app_cfg::FieldNamesCfg;
///
/// let json = r#"{
///      "quality-field-tag": "q_",
///      "derived-field-tag": "derived",
///      "binary-target-field-tag": "reach"
///   }"#;
/// let model: FieldNamesCfg = serde_json::from_str(&json).unwrap();
/// assert!(model.quality_field_tag == "q_");
/// assert!(model.derived_field_tag == "derived");
/// assert!(model.binary_target_field_tag == "reach");
///
/// ```
#[derive(Clone, Debug, Deserialize)]
pub struct FieldNamesCfg {
    #[serde(rename = "quality-field-tag")]
    pub quality_field_tag: SearchTerm,
    #[serde(rename = "derived-field-tag")]
    pub derived_field_tag: SearchTerm,
    #[serde(rename = "binary-target-field-tag")]
    pub binary_target_field_tag: SearchTerm,
}

type SearchTerm = String;
