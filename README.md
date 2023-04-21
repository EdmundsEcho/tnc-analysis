# tnc-analysis
Test and control analysis in rust

Proof of concept/early draft of how to use polars to pre-process data for consumption by a statistical analysis.

## Overview

`csv -> polars -> subset columns -> X: 1D array, y 1D array -> polars plus column for prediction -> csv`
