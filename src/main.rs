use std::{collections::HashMap, process::ExitCode, str::FromStr};

use aws_sdk_ssm::{types::Parameter, Client};
use clap::{command, Parser};
use eyre::{Context, Result};
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

    /// Explicitly specify parameters to fetch and optionally rename the environment variable to set.
    #[arg(long = "param", short, value_name = "NAME[:ENV]")]
    params: Vec<Param>,

    /// The command to run after setting the environment variables from the ssm parameters.
    utility: String,

    /// The arguments to pass to the utility.
    arguments: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);
    let names = match args.params[..] {
        [] => get_parameter_names(&client).await?,
        _ => args.params.names(),
    };
    let params = client
        .get_parameters()
        .set_names(Some(names))
        .set_with_decryption(Some(!args.no_decrypt))
        .send()
        .await?
        .parameters
        .into_iter()
        .flatten()
        .filter_map(|p| {
            if let Parameter {
                name: Some(name),
                value: Some(value),
                ..
            } = p
            {
                Some((name, value))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let rename = args.params.pairs().into_iter().collect::<HashMap<_, _>>();
    let mut cmd = Command::new(args.utility);
    if args.ignore {
        cmd.env_clear();
    }
    for arg in args.arguments {
        cmd.arg(arg);
    }
    for (name, value) in params {
        let name = rename.get(&name).unwrap_or(&name);
        cmd.env(name, value);
    }

    let code = cmd.spawn()?.wait().await?.code().unwrap_or(1);
    Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)))
}

/// Gets the names of all the parameters in the given AWS Systems Manager parameter store.
/// Note that if the role doesn't have permission to list all the parameters, the -p (--param) option
/// can be used to specify the parameters to fetch.
/// TODO: support filtering by path.
async fn get_parameter_names(client: &Client) -> Result<Vec<String>> {
    let params = client
        .describe_parameters()
        .send()
        .await
        .context("Failed to get parameter names")?
        .parameters
        .into_iter()
        .flatten()
        .filter_map(|p| p.name)
        .collect();
    Ok(params)
}

#[derive(Clone, Debug)]
struct Param {
    name: String,
    alias: Option<String>,
}

trait ParamNames {
    /// As we need only the names of parameters to call get_parameters, this function is used to
    /// convert a Vec<Param> to a Vec<String> containing only the names.
    fn names(&self) -> Vec<String>;

    /// This function is used to convert a Vec<Param> to a Vec<(String, String)>, which can
    /// be used to create a HashMap.
    fn pairs(&self) -> Vec<(String, String)>;
}

impl ParamNames for Vec<Param> {
    fn names(&self) -> Vec<String> {
        self.iter().map(|p| p.name.clone()).collect()
    }

    fn pairs(&self) -> Vec<(String, String)> {
        self.iter()
            .filter_map(|p| match p {
                Param {
                    name,
                    alias: Some(alias),
                } => Some((name.clone(), alias.clone())),
                _ => None,
            })
            .collect()
    }
}

// This function is used to convert a string to a Param.  The string is
// expected to be in the format "name:alias" where "name" is the parameter
// name and "alias" is an optional alias for the parameter.
impl FromStr for Param {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((name, alias)) => Ok(Self {
                name: name.to_owned(),
                alias: Some(alias.to_owned()),
            }),
            None => Ok(Self {
                name: s.to_owned(),
                alias: None,
            }),
        }
    }
}
