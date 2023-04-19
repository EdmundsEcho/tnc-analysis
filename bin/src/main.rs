use color_eyre::eyre::Result;
use colored::*;

use tracing::{event, Level};
use tracing_subscriber;

use std::time::Instant;

use tnc_analysis_lib::prelude::*;

const FILENAME: &str = "/Users/edmund/Downloads/matrix.csv";
const OUT_FILE_CSV: &str = "/Users/edmund/Downloads/matrix.logit.csv";
const FIELD_NAMES_CFG: &str = "./res/app-cfg.json";
// const PROPENSITY_CFG: &str = "./res/propensity-cfg.json";

fn main() -> Result<()> {
    // performance and debugging metrics
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let start = Instant::now();

    // let cfg: Config<PropensityScore> = read_cfg(PROPENSITY_CFG)?;
    let field_names_cfg: Config<FieldNamesCfg> = read_config(FIELD_NAMES_CFG)?;

    // Future option: set a global static using lazy
    // set_app_config(read_cfg(FIELD_NAMES_CFG)?);

    let matrix = Matrix::from_file(FILENAME, None)?;

    // print the first few lines
    let summary = matrix.describe(None)?;
    println!("{}", &summary);
    println!("{}", &matrix.head(Some(5)));

    // initialize
    let mut matrix = matrix;
    let (included, excluded) = matrix.with_include_tag();

    event!(
        Level::INFO,
        "Included: {} Excluded: {}",
        included.to_string().green(),
        excluded.to_string().red()
    );

    event!(Level::DEBUG, "{}", matrix.show_fields()?);

    // Configure the propensity score computation
    let cfg = PropensityCfg::builder(
        matrix.binary_target(field_names_cfg.clone()),
        matrix.predictors(field_names_cfg.clone()),
    )
    .with_name("prop_score")
    .bin_count(5)
    .build();
    event!(Level::DEBUG, "{:#?}", &cfg);
    matrix.with_propensity(cfg)?;

    let view = matrix.select(["subject_idx", "propensity"])?;
    event!(Level::INFO, "{}", &view.head(Some(5)));

    event!(Level::INFO, "Meta with propensity: {}", matrix.show_meta()?);

    // save to file
    matrix.write_to_file_csv(OUT_FILE_CSV)?;
    event!(Level::INFO, "✅ Wrote to file: {}", OUT_FILE_CSV);

    let duration = start.elapsed();
    event!(Level::INFO, "Time elapsed: {:?}", duration);

    Ok(())
}
