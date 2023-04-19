use nom::branch::alt;
use nom::bytes::complete::{tag, take_until1};
use polars::prelude::*;

#[derive(Debug)]
pub struct Header<'a> {
    inner: Vec<&'a str>,
}

impl<'a> Header<'a> {
    pub fn new(inner: Vec<&'a str>) -> Self {
        Header { inner }
    }
}

pub type FieldNameWithIndex<'a> = (&'a str, usize);

impl<'a> From<&'a DataFrame> for Header<'a> {
    fn from(df: &'a DataFrame) -> Self {
        Header::new(df.get_column_names())
    }
}
impl<'a> From<Vec<&'a str>> for Header<'a> {
    fn from(vec: Vec<&'a str>) -> Self {
        Header::new(vec)
    }
}
impl<'a> std::ops::Deref for Header<'a> {
    type Target = Vec<&'a str>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<'a> Header<'a> {
    pub fn get_fuzzy_field(&self, find_this: &str) -> Option<FieldNameWithIndex<'_>> {
        self.get_fuzzy_fields(find_this).first().cloned()
    }
    pub fn get_fuzzy_fields(&self, find_this: &str) -> Vec<FieldNameWithIndex<'_>> {
        self.iter()
            .enumerate()
            .filter(|(_, try_this)| {
                alt((
                    tag::<_, _, nom::error::Error<_>>(find_this),
                    take_until1::<_, _, nom::error::Error<_>>(find_this),
                ))(**try_this)
                .map(|_| true)
                .is_ok()
            })
            .map(|(idx, matching_field)| (*matching_field, idx))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const HEADER: [&str; 10] = [
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
    fn test_get_fuzzy_field_logic() {
        let result = take_until1::<&str, &str, nom::error::Error<&str>>("reach")("thatreachThis");
        assert!(result.is_ok());
    }
    #[test]
    fn test_get_when_middle() {
        let header = Header::new(HEADER.to_vec());
        let result = header.get_fuzzy_field("reach");
        assert!(result.is_some());
    }
    #[test]
    fn test_get_when_start() {
        let header = Header::new(HEADER.to_vec());
        let result = header.get_fuzzy_field("q_");
        assert!(result.is_some());
    }
    #[test]
    fn test_get_quality_logic() {
        let result = tag::<&str, &str, nom::error::Error<&str>>("q_")("q_this");
        assert!(result.is_ok());
    }
    #[test]
    fn test_get_qualities() {
        let header = Header::new(HEADER.to_vec());
        let result = header.get_fuzzy_fields("q_");
        assert_eq!(3, result.len());
    }
    #[test]
    fn test_get_derived() {
        let header = Header::new(HEADER.to_vec());
        let result = header.get_fuzzy_fields("derived");
        assert_eq!(1, result.len());
        assert_eq!(
            Some(9),
            result.first().map(|field_with_idx| field_with_idx.1)
        );
    }
}
