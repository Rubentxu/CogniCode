//! La línea de órdenes de los goals: el árbol, el parseo y la ejecución.
//!
//! Los goals **no** son variantes de [`crate::cli::commands::CliCommand`]: el
//! registro los construye, y un enum los duplicaría —el nombre del goal, su
//! descripción y sus argumentos vivirían en dos sitios—. Por eso el parseo no
//! es `Cli::parse()` sino [`parse_cli`], que mira primero si lo que se
//! escribió es un goal.
//!
//! Lo que hay aquí es la proyección del goal a la terminal: el comando clap
//! lo construye el registro ([`crate::cli::goals_clap::clap_command`]), y lo
//! que se ejecuta es la misma implementación que ejecuta el MCP.

use clap::{CommandFactory, FromArgMatches};

use crate::cli::commands::{Cli, CommandExecutor, Format};
use crate::cli::result::CommandOutput;
use cognicode_core::interface::goals::GoalSpec;

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
    Cli::command().subcommands(crate::cli::goals_clap::clap_commands())
}

/// Parsea la línea de órdenes, goals incluidos.
///
/// Un goal se resuelve antes que el enum: si lo que se escribió es un goal,
/// el enum no lo conoce, y `Cli::from_arg_matches` se quejaría de un
/// subcomando que no existe.
pub fn parse_cli() -> Result<ParsedCli, clap::Error> {
    let matches = cli_command().get_matches();
    if let Some((name, arguments)) = matches.subcommand()
        && let Some(goal) = cognicode_core::interface::goals::find(name)
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

    // `cognicode` sin subcomando es el goal `status`: la raíz del producto.
    // Es el goal que responde «qué ofrece este build y qué se ve del
    // workspace», y esa es la primera pregunta de quien llega.
    if matches.subcommand_name().is_none() {
        let format = match matches.get_one::<Format>("format") {
            Some(Format::Json) => Format::Json,
            _ => Format::Text,
        };
        let arguments =
            crate::cli::goals_clap::clap_command(&cognicode_core::interface::goals::STATUS)
                .get_matches_from(["status"]);
        return Ok(ParsedCli::Goal {
            goal: &cognicode_core::interface::goals::STATUS,
            arguments,
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
        let values = crate::cli::goals_clap::arguments_from_matches(goal, arguments);
        // La terminal no tiene una caché inyectada: usa la del entorno, la
        // misma que usa el resto del producto.
        let cache_root = cognicode_core::application::graph_cache::default_cache_root();
        let context = cognicode_core::interface::goals::GoalEnvironment::new(
            workspace,
            cache_root.as_deref(),
        );
        let answer = (goal.run)(&context, &values)
            .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;

        let text = match format {
            // El mismo sobre que el MCP publica: el dueño del sobre es el
            // mismo en las dos superficies, así que la terminal y el cable no
            // pueden decir cosas distintas de la misma respuesta.
            Format::Json => {
                let context = cognicode_core::interface::hateoas::HandleContext {
                    workspace: workspace.to_string_lossy().to_string(),
                    snapshot: "none".to_string(),
                };
                let envelope = cognicode_core::interface::hateoas::envelope_for(
                    goal.name,
                    &values.as_object().cloned().unwrap_or_default(),
                    &answer,
                    &context,
                );
                serde_json::to_string_pretty(&envelope)?
            }
            Format::Text => (goal.render)(&answer),
        };
        Ok(CommandOutput::rendered(format!("{text}\n")))
    }
}
