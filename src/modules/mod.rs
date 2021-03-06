use clap::{App, ArgMatches};
use config::CeresConfig as Config;
use run_config::RunConfig;

pub trait Module {
    fn build_sub_cli() -> App<'static, 'static>;
    fn call(cli_args: Option<&ArgMatches>, run_config: &RunConfig, config: &Config) -> Result<()>;
}

main_module!(
    consul,
    health,
    infrastructure,
    instances,
    ops,
    statuspages,
    stories
);

error_chain! {
    errors {
        NoSuchCommand(command: String) {
            description("no such command")
            display("no such command '{}'", command)
        }

        NoCommandSpecified {
            description("no command specified")
            display("no command specified")
        }

        NoSubcommandSpecified(module_name: String) {
            description("no sub command specified")
            display("no sub command for module {} specified", module_name)
        }

        ModuleFailed(module_name: String) {
            description("module failed")
            display("executing module {} failed", module_name)
        }

        ConfigMissingInProfile(config: String) {
            description("There is a configuration missing in this profile")
            display("there is no {} configuration in this profile", config)
        }
    }
}
