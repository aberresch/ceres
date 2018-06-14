use clap::{App, Arg, ArgMatches, SubCommand};
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};

use config::CeresConfig as Config;
use modules::{Result as ModuleResult, Error as ModuleError, ErrorKind as ModuleErrorKind, Module};
use output::OutputType;
use output::infrastructure::{JsonOutputAspListResult, OutputAspListResult, PlainOutputAspListResult, TableOutputAspListResult};
use run_config::RunConfig;

pub const NAME: &str = "list";

pub struct SubModule;

impl Module for SubModule {
    fn build_sub_cli() -> App<'static, 'static> {
        SubCommand::with_name(NAME)
            .about("list available infrastructure ASPs")
            .arg(
                Arg::with_name("base-dir")
                    .long("base-dir")
                    .takes_value(true)
                    .help("Overwrites base dir from ceres configuration file"),
            )
            .arg(
                Arg::with_name("output")
                    .long("output")
                    .short("o")
                    .takes_value(true)
                    .default_value("human")
                    .possible_values(&["human", "json", "plain"])
                    .help("Selects output format"),
            )
    }

    fn call(cli_args: Option<&ArgMatches>, run_config: &RunConfig, config: &Config) -> ModuleResult<()> {
        let args = cli_args.unwrap(); // Safe unwrap
        do_call(args, run_config, config)
            .map_err(|e| ModuleError::with_chain(e, ModuleErrorKind::ModuleFailed(NAME.to_owned())))
    }
}

error_chain! {
    errors {
        FailedToLoadProfile {
            description("Failed to load profile")
            display("Failed to load profile")
        }
        NoLocalBaseDir {
            description("No local base directory configured for this profile")
            display("No local base directory configured for this profile")
        }
        FailedToFindAsps {
            description("Failed to find asps")
            display("Failed to find asps")
        }
        FailedParseAspFromPath(path: String) {
            description("Failed to parse ASP from path")
            display("Failed to parse ASP from path '{}'", path)
        }
        FailedToParseOutputType {
            description("Failed to parse output type")
            display("Failed to parse output type")
        }
        FailedOutput {
            description("Failed to output")
            display("Failed to output")
        }
    }
}

#[allow(unstable_name_collision)] // flatten from itertools
fn do_call(args: &ArgMatches, run_config: &RunConfig, config: &Config) -> Result<()> {
    let profile = match run_config.active_profile.as_ref() {
        "default" => config.get_default_profile(),
        s => config.get_profile(s),
    }.chain_err(|| ErrorKind::FailedToLoadProfile)?;

    let local_base_dir = if let Some(base_dir) = args.value_of("base-dir") {
        base_dir
    } else {
        profile.local_base_dir.as_ref()
        .ok_or(Error::from_kind(ErrorKind::NoLocalBaseDir))?
    };

    let asps: Result<Vec<_>> = find_asps(local_base_dir)?
        .iter()
        .flat_map(|x| x.strip_prefix(local_base_dir))
        .map(|x| Asp::from_path(x))
        .collect();
    let asps = asps?;

    info!("Outputting asp list");
    output_list(args, run_config, config, &asps)?;

    Ok(())
}

fn find_asps<P: AsRef<Path>>(base_dir: P) -> Result<Vec<PathBuf>> {
    let walker = WalkBuilder::new(base_dir).build();

    let asps = walker
        .filter(|x| // Does the path point to a Makefile?
            x.is_ok() &&
            x.as_ref().unwrap().path().ends_with("Makefile")
        )
        .filter(|x| // Does the path to the Makefile contain "ansible-setup-package/resources"?
            x.as_ref().unwrap().path().parent().is_some() && // Safe see above
            x.as_ref().unwrap().path().parent().unwrap().to_string_lossy().contains("ansible-setup-package/resources")
        )
        .map(|x|
             x
             .map(|d| d.path().parent().unwrap().to_path_buf()) // Safe
             .map_err(|e| Error::with_chain(e, ErrorKind::FailedToFindAsps)))
        .filter(|x| { // Does the parent directory contain a file "project.cfg"
            if let Ok(x) = x {
                let mut p = x.clone();
                p.pop(); // Will be true since at least two parents are guaranteed; see above.
                p.push("project.cfg");
                p.exists() && p.is_file()
            } else {
                false
            }
        })
        .collect();

    asps
}

#[derive(Debug, Serialize)]
pub struct Asp {
    pub project: String,
    pub resource: String,
}

impl Asp {
    /// This function assumes that it gets a relative path starting with the project directory
    ///
    /// Example: "logimon/ansible-setup-package/resources/elk_elasticsearch/" instead of "/Users/lukas/Documents/src/ceres/tests/base_dir/logimon/ansible-setup-package/resources/elk_elasticsearch"
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path: &Path = path.as_ref();

        let components: Vec<_> = path.components().collect();
        match components.as_slice() {
            [Component::Normal(project), _, _, Component::Normal(resource)] =>
                Ok( Asp {
                    project: project.to_string_lossy().to_string(),
                    resource: resource.to_string_lossy().to_string(),
                } ),
            _ => Err(Error::from_kind(ErrorKind::FailedParseAspFromPath(path.to_string_lossy().to_string()))),
        }
    }
}

fn output_list(
    args: &ArgMatches,
    _: &RunConfig,
    _: &Config,
    asps: &[Asp],
) -> Result<()> {
    let output_type = args.value_of("output").unwrap() // Safe
        .parse::<OutputType>()
        .chain_err(|| ErrorKind::FailedToParseOutputType)?;
    let mut stdout = ::std::io::stdout();

    match output_type {
        OutputType::Human => {
            let output = TableOutputAspListResult;

            output
                .output(&mut stdout, asps)
                .chain_err(|| ErrorKind::FailedOutput)
        },
        OutputType::Json => {
            let output = JsonOutputAspListResult;

            output
                .output(&mut stdout, asps)
                .chain_err(|| ErrorKind::FailedOutput)
        },
        OutputType::Plain => {
            let output = PlainOutputAspListResult;

            output
                .output(&mut stdout, asps)
                .chain_err(|| ErrorKind::FailedOutput)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectral::prelude::*;

    #[test]
    fn find_asps_okay() {
        let path = Path::new("tests/base_dir");

        let asp_paths = find_asps(path);

        assert_that(&asp_paths).is_ok().has_length(22);
    }

    #[test]
    fn asp_from_path_okay() {
        let path = Path::new("logimon/ansible-setup-package/resources/elk_elasticsearch");

        let asp_res = Asp::from_path(path);
        assert_that(&asp_res.is_ok()).is_true();

        let asp = asp_res.unwrap();
        assert_that(&asp.project).is_equal_to("logimon".to_owned());
        assert_that(&asp.resource).is_equal_to("elk_elasticsearch".to_owned());
    }
}