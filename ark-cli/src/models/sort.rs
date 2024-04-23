use clap::Parser;

#[derive(Parser, Debug, clap::ValueEnum, Clone)]
pub enum Sort {
    Asc,
    Desc,
}
