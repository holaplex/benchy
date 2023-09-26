use std::path::PathBuf;

use structopt::StructOpt;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "benchy", about = "A CLI to benchmark Hub minting speed")]
pub struct Opt {
    #[structopt(flatten)]
    pub global: GlobalOptions,

    #[structopt(flatten)]
    pub cmd: Cli,
}

#[derive(Debug, StructOpt, Clone)]
pub struct GlobalOptions {
    #[structopt(
        long,
        global = true,
        help = "config path",
        default_value = "./config.json",
        env = "CONFIG_PATH",
        parse(from_os_str)
    )]
    pub config: PathBuf,
    #[structopt(
        long,
        global = true,
        help = "CSV report output path",
        default_value = "./output.csv",
        env = "OUTPUT_PATH",
        parse(from_os_str)
    )]
    pub output: PathBuf,
}

#[derive(StructOpt, Debug, Default, Clone)]
pub struct Cli {
    /// Number of concurrent requests
    #[structopt(short, long, default_value = "1")]
    pub parallelism: usize,

    /// Number of iterations to run
    #[structopt(short, long, default_value = "1")]
    pub iterations: usize,

    /// Wait Delay in seconds between each iteration
    #[structopt(short, long, default_value = "1")]
    pub delay: u64,

    /// Wait Delay in seconds between each iteration
    #[structopt(short, long)]
    pub retry: bool,
}
