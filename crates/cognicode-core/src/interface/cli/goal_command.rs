//! La línea de órdenes de los goals: el árbol, el parseo y la ejecución.
//!
//! Los goals **no** son variantes de [`super::commands::CliCommand`]: el
//! registro los construye, y un enum los duplicaría —el nombre del goal, su
//! descripción y sus argumentos vivirían en dos sitios—. Por eso el parseo no
//! es `Cli::parse()` sino [`parse_cli`], que mira primero si lo que se
//! escribió es un goal.
//!
//! Lo que hay aquí es la proyección del goal a la terminal: el comando clap
//! lo construye el registro ([`crate::interface::goals::clap_command`]), y lo
//! que se ejecuta es la misma implementación que ejecuta el MCP.

use clap::{CommandFactory, FromArgMatches};

use crate::interface::cli::commands::{Cli, CommandExecutor, Format};
use crate::interface::cli::result::CommandOutput;
use crate::interface::goals::GoalSpec;

/// Lo que el parseo de la línea de órdenes encontró.
#[derive(Debug)]
pub enum ParsedCli {
    /// Un comando de siempre.
    Command(Cli),
    /// Un goal del registro, con los argumentos que su comando leyó.
    Goal {
        /// El goal que se pidió.
        goal: &'static GoalSpec,
        /// Los argumentos que leyó el comando clap del goal.
        arguments: clap::ArgMatches,
        /// El formato que pidió el llamante en la raíz.
        format: Format,
        /// Si el llamante pidió más detalle.
        verbose: bool,
    },
}

/// El árbol de comandos con los goals del registro encima.
///
/// El árbol se construye con los goals dentro, así que `cognicode --help`
/// los lista y `cognicode <goal> --help` explica el goal.
pub fn cli_command() -> clap::Command {
    Cli::command().subcommands(crate::interface::goals::clap_commands())
}

/// Parsea la línea de órdenes, goals incluidos.
///
/// Un goal se resuelve antes que el enum: si lo que se escribió es un goal,
/// el enum no lo conoce, y `Cli::from_arg_matches` se quejaría de un
/// subcomando que no existe.
pub fn parse_cli() -> Result<ParsedCli, clap::Error> {
    let matches = cli_command().get_matches();
    if let Some((name, arguments)) = matches.subcommand()
        && let Some(goal) = crate::interface::goals::find(name)
    {
        // El formato lo elige el comando del goal, no la raiz: es la
        // convencion de los comandos que tienen su propia forma de
        // responder (`capabilities`, `doctor`, `find-usages`).
        let format = match arguments.get_one::<String>("format").map(String::as_str) {
            Some("json") => Format::Json,
            _ => Format::Text,
        };
        return Ok(ParsedCli::Goal {
            goal,
            arguments: arguments.clone(),
            format,
            verbose: matches.get_flag("verbose"),
        });
    }
    Ok(ParsedCli::Command(Cli::from_arg_matches(&matches)?))
}

impl CommandExecutor {
    /// Execute a goal from the registry.
    ///
    /// The arguments arrive as the `ArgMatches` of the command clap built
    /// from the declaration, and the core reads them with the same struct the
    /// MCP reads: the flag in the terminal and the field on the wire are the
    /// same argument, and the answer is the same one.
    ///
    /// TASK-82.
    pub async fn execute_goal(
        goal: &GoalSpec,
        arguments: &clap::ArgMatches,
        workspace: &std::path::Path,
        format: Format,
    ) -> Result<CommandOutput, Box<dyn std::error::Error>> {
        let values = crate::interface::goals::arguments_from_matches(goal, arguments);
        let answer = (goal.run)(workspace, &values)
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;

        let text = match format {
            Format::Json => serde_json::to_string_pretty(&answer)?,
            Format::Text => (goal.render)(&answer),
        };
        Ok(CommandOutput::rendered(format!("{text}\n")))
    }
}
