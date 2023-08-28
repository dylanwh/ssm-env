#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::unwrap_used,
    clippy::expect_used
)]

use std::{collections::HashMap, process::ExitCode, str::FromStr};

use aws_sdk_ssm::{types::Parameter, Client};
use clap::{command, Parser};
use eyre::Result;
use tokio::process::Command;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Decrypt SecureStrings
    #[arg(long)]
    no_decrypt: bool,

    /// Ignore (clear) existing environment variables.
    #[arg(long, short)]
    ignore: bool,

    /// Export an aws ssm parameter to an environment variable. The parameter name can
    /// be specified if it differs from the environment variable.
    #[arg(long, short = 'e', value_name = "ENV[=PARAM]")]
    export: Vec<Export>,

    /// Export one level of a path of aws ssm parameters to environment variables. All
    /// parameters under the prefix will be exported as environment variables.
    #[arg(long, short = 'P', value_name = "PATH")]
    export_path: Vec<String>,

    /// The command to run after setting the environment variables from the ssm parameters.
    utility: String,

    /// The arguments to pass to the utility.
    arguments: Vec<String>,
}

#[derive(Clone, Debug)]
struct Export {
    env: String,
    param: Option<String>,
}

impl Args {
    fn parameter_names(&self) -> Vec<String> {
        self.export
            .iter()
            .map(|e| e.param.clone().unwrap_or_else(|| e.env.clone()))
            .collect::<Vec<_>>()
    }

    fn export_names(&self) -> HashMap<String, String> {
        self.export
            .iter()
            .filter_map(|e| match e {
                Export {
                    env,
                    param: Some(param),
                } => Some((param.clone(), env.clone())),
                Export { param: None, .. } => None,
            })
            .collect::<HashMap<_, _>>()
    }
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    env_logger::init();

    let args = Args::parse();
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);
    let names = args.parameter_names();
    let mut params: Vec<(String, String)> = Vec::new();
    if !names.is_empty() {
        let exports = args.export_names();
        let p = client
            .get_parameters()
            .set_names(Some(names))
            .set_with_decryption(Some(!args.no_decrypt))
            .send()
            .await?
            .parameters
            .into_iter()
            .flatten()
            .filter_map(|p| filter_export(p, &exports))
            .collect::<Vec<_>>();

        params.extend(p);
    }
    let paths = args.export_path;
    for path in paths {
        let p = client
            .get_parameters_by_path()
            .set_path(Some(path.clone()))
            .set_with_decryption(Some(!args.no_decrypt))
            .send()
            .await?
            .parameters
            .into_iter()
            .flatten()
            .filter_map(|param| filter_export_path(param, &path))
            .collect::<Vec<_>>();

        params.extend(p);
    }

    let mut cmd = Command::new(args.utility);
    if args.ignore {
        cmd.env_clear();
    }
    cmd.args(args.arguments);
    cmd.envs(params);

    let code = cmd.spawn()?.wait().await?.code().unwrap_or(1);
    Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)))
}

fn filter_export(param: Parameter, exports: &HashMap<String, String>) -> Option<(String, String)> {
    if let Parameter {
        name: Some(name),
        value: Some(value),
        ..
    } = param
    {
        let name = exports.get(&name).unwrap_or(&name);
        Some((name.clone(), value))
    } else {
        None
    }
}

fn filter_export_path(param: Parameter, path: &str) -> Option<(String, String)> {
    if let Parameter {
        name: Some(name),
        value: Some(value),
        ..
    } = param
    {
        let prefix = if path.ends_with('/') {
            path.to_owned()
        } else {
            format!("{path}/")
        };
        let name = name.strip_prefix(&prefix).unwrap_or(&name);
        Some((name.to_owned(), value))
    } else {
        None
    }
}
impl FromStr for Export {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once('=') {
            Some((env, param)) => Ok(Self {
                env: env.to_owned(),
                param: Some(param.to_owned()),
            }),
            None => Ok(Self {
                env: s.to_owned(),
                param: None,
            }),
        }
    }
}
