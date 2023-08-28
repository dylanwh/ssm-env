
# ssm-env

ssm-env is a Rust utility for setting environment variables from AWS SSM parameters.  It
is analogous to the standard `env` utility, but instead of literally specifying values
it pulls them from SSM.

In addition to explicitly mapping parameters to environment variables, ssm-env can also
export all parameters in a given path as environment variables.  This is useful for
cases where you have a large number of parameters that you want to export as environment
variables.

Note that (for now) it does not support recursive parameter fetching, so you can't do
that (although it is not a significant change to add it).

## Usage

If you have an SSM parameter named `DATABASE_URL` that you want to feed into your application `my-app`,
assuming the following is run from an EC2 instance with an appropriate IAM role:

```
ssm-env -e DATABASE_URL -- /usr/local/bin/my-app
```

Note that ssm-env makes use of the aws sdk and honors all of the standard AWS
environment variables and credentials.

Most of the time, ssm parameters will not be named the same as the environment variables
and in that case you can append `=PARAMETER_NAME` to the environment variable name to map
it to a different parameter name.  For example, if you have a parameter named
`/my-app/database-url` you can do:

```
ssm-env -e DATABASE_URL=/my-app/database-url -- /usr/local/bin/my-app
```

Technically `database-url` is a valid environment variable name if a bit unconventional.
If you can either name your parameters in the same way as environment variables 
or you don't care about being unconventional, you can use the `-P` option to map a path.

```
ssm-env -P /my-app/ -- /usr/local/bin/my-app
```


## Installation

ssm-env is available as binary releases for Linux, FreeBSD, and MacOS.  You can download them from the [releases page](TODO). 

If you have Rust installed, you can also install it with `cargo build --release` and then copy the binary from `target/release/ssm-env` to wherever you want it.


## Prior Art

There are a few other implementations of this idea in other languages,
and it's such an obvious idea that I'm sure there are more. I find it a bit surprising this functionality isn't built into the aws cli.

I wrote this before I found the following, and I think I prefer my implementation more but perhaps these others will have fewer rough edges or be more suitable for your use case.

- https://github.com/remind101/ssm-env
- https://github.com/jamietsao/aws-ssm-env

## License

MIT