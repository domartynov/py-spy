use clap::{App, Arg};
use remoteprocess::Pid;

/// Options on how to collect samples from a python process
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    /// Whether or not we should stop the python process when taking samples.
    /// Setting this to false will reduce the performance impact on the target
    /// python process, but can lead to incorrect results like partial stack
    /// traces being returned or a higher sampling error rate
    pub non_blocking: bool,

    /// Whether or not to profile native extensions. Note: this option can not be
    /// used with the nonblocking option, as we have to pause the process to collect
    /// the native stack traces
    pub native: bool,

    // The following config options only apply when using py-spy as an application
    #[doc(hidden)]
    pub command: String,
    #[doc(hidden)]
    pub pid: Option<Pid>,
    #[doc(hidden)]
    pub python_program: Option<Vec<String>>,
    #[doc(hidden)]
    pub sampling_rate: u64,
    #[doc(hidden)]
    pub filename: Option<String>,
    #[doc(hidden)]
    pub format: Option<FileFormat>,
    #[doc(hidden)]
    pub show_line_numbers: bool,
    #[doc(hidden)]
    pub duration: RecordDuration,
    #[doc(hidden)]
    pub include_idle: bool,
    #[doc(hidden)]
    pub include_thread_ids: bool,
    #[doc(hidden)]
    pub gil_only: bool,
    #[doc(hidden)]
    pub hide_progess: bool,
    #[doc(hidden)]
    pub dump_json: bool,
}

arg_enum!{
    #[derive(Debug, Clone, Eq, PartialEq)]
    #[allow(non_camel_case_types)]
    pub enum FileFormat {
        flamegraph,
        raw,
        speedscope
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RecordDuration {
    Unlimited,
    Seconds(u64)
}

impl Default for Config {
    /// Initializes a new Config object with default parameters
    #[allow(dead_code)]
    fn default() -> Config {
        Config{pid: None, python_program: None, filename: None, format: None,
               command: String::from("top"),
               non_blocking: false, show_line_numbers: false, sampling_rate: 100,
               duration: RecordDuration::Unlimited, native: false,
               gil_only: false, include_idle: false, include_thread_ids: false,
               hide_progess: false, dump_json: false}
    }
}

impl Config {
    /// Uses clap to set config options from commandline arguments
    pub fn from_commandline() -> Config {
        let args: Vec<String> = std::env::args().collect();
        Config::from_args(&args).unwrap_or_else( |e| e.exit() )
    }

    pub fn from_args(args: &[String]) -> clap::Result<Config> {
        // pid/native/nonblocking/rate/pythonprogram arguments can be
        // used across various subcommand - define once here
        let pid = Arg::with_name("pid")
                    .short("p")
                    .long("pid")
                    .value_name("pid")
                    .help("PID of a running python program to spy on")
                    .takes_value(true)
                    .required_unless("python_program");
        #[cfg(unwind)]
        let native = Arg::with_name("native")
                    .short("n")
                    .long("native")
                    .help("Collect stack traces from native extensions written in Cython, C or C++");

        #[cfg(not(target_os="freebsd"))]
        let nonblocking = Arg::with_name("nonblocking")
                    .long("nonblocking")
                    .help("Don't pause the python process when collecting samples. Setting this option will reduce \
                          the perfomance impact of sampling, but may lead to inaccurate results");

        let rate = Arg::with_name("rate")
                    .short("r")
                    .long("rate")
                    .value_name("rate")
                    .help("The number of samples to collect per second")
                    .default_value("100")
                    .takes_value(true);
        let program = Arg::with_name("python_program")
                    .help("commandline of a python program to run")
                    .multiple(true);

        let record = clap::SubCommand::with_name("record")
            .about("Records stack trace information to a flamegraph, speedscope or raw file")
            .arg(program.clone())
            .arg(pid.clone())
            .arg(Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("filename")
                .help("Output filename")
                .takes_value(true)
                .required(true))
            .arg(Arg::with_name("format")
                .short("f")
                .long("format")
                .value_name("format")
                .help("Output file format")
                .takes_value(true)
                .possible_values(&FileFormat::variants())
                .case_insensitive(true)
                .default_value("flamegraph"))
            .arg(Arg::with_name("duration")
                .short("d")
                .long("duration")
                .value_name("duration")
                .help("The number of seconds to sample for")
                .default_value("unlimited")
                .takes_value(true))
            .arg(rate.clone())
            .arg(Arg::with_name("function")
                .short("F")
                .long("function")
                .help("Aggregate samples by function name instead of by line number"))
            .arg(Arg::with_name("gil")
                .short("g")
                .long("gil")
                .help("Only include traces that are holding on to the GIL"))
            .arg(Arg::with_name("threads")
                .short("t")
                .long("threads")
                .help("Show thread ids in the output"))
            .arg(Arg::with_name("idle")
                .short("i")
                .long("idle")
                .help("Include stack traces for idle threads"))
            .arg(Arg::with_name("hideprogress")
                .long("hideprogress")
                .hidden(true)
                .help("Hides progress bar (useful for showing error output on record)"));

        let top = clap::SubCommand::with_name("top")
            .about("Displays a top like view of functions consuming CPU")
            .arg(program.clone())
            .arg(pid.clone())
            .arg(rate.clone());

        let dump = clap::SubCommand::with_name("dump")
            .about("Dumps stack traces for a target program to stdout")
            .arg(pid.clone().required(true))
            .arg(Arg::with_name("json")
                .short("j")
                .long("json")
                .help("Format output as JSON"));

        // add native unwinding if appropiate
        #[cfg(unwind)]
        let record = record.arg(native.clone());
        #[cfg(unwind)]
        let top = top.arg(native.clone());
        #[cfg(unwind)]
        let dump = dump.arg(native.clone());

        // Nonblocking isn't an option for freebsd, remove
        #[cfg(not(target_os="freebsd"))]
        let record = record.arg(nonblocking.clone());
        #[cfg(not(target_os="freebsd"))]
        let top = top.arg(nonblocking.clone());
        #[cfg(not(target_os="freebsd"))]
        let dump = dump.arg(nonblocking.clone());

        let matches = App::new(crate_name!())
            .version(crate_version!())
            .about(crate_description!())
            .setting(clap::AppSettings::InferSubcommands)
            .setting(clap::AppSettings::SubcommandRequiredElseHelp)
            .global_setting(clap::AppSettings::DeriveDisplayOrder)
            .global_setting(clap::AppSettings::UnifiedHelpMessage)
            .subcommand(record)
            .subcommand(top)
            .subcommand(dump)
            .get_matches_from_safe(args)?;
        info!("Command line args: {:?}", matches);

        let mut config = Config::default();

        let (subcommand, matches) = matches.subcommand();
        let matches = matches.unwrap();

        match subcommand {
            "record" => {
                config.sampling_rate = value_t!(matches, "rate", u64)?;
                config.duration = match matches.value_of("duration") {
                    Some("unlimited") | None => RecordDuration::Unlimited,
                    Some(seconds) => RecordDuration::Seconds(seconds.parse().expect("invalid duration"))
                };
                config.format = Some(value_t!(matches.value_of("format"), FileFormat).unwrap_or_else(|e| e.exit()));
                config.filename = matches.value_of("output").map(|f| f.to_owned());
            },
            "top" => {
                config.sampling_rate = value_t!(matches, "rate", u64)?;
            }
            _ => {}
        }
        config.command = subcommand.to_owned();

        // options that can be shared between subcommands
        config.pid = matches.value_of("pid").map(|p| p.parse().expect("invalid pid"));
        config.python_program = matches.values_of("python_program").map(|vals| {
            vals.map(|v| v.to_owned()).collect()
        });
        config.show_line_numbers = matches.occurrences_of("function") == 0;
        config.include_idle = matches.occurrences_of("idle") > 0;
        config.gil_only = matches.occurrences_of("gil") > 0;
        config.include_thread_ids = matches.occurrences_of("threads") > 0;

        config.non_blocking = matches.occurrences_of("nonblocking") > 0;
        config.native = matches.occurrences_of("native") > 0;
        config.hide_progess = matches.occurrences_of("hideprogress") > 0;
        config.dump_json = matches.occurrences_of("json") > 0;

        // disable native profiling if invalidly asked for
        if config.native && config.non_blocking {
            error!("Can't get native stack traces with the --nonblocking option. Disabling native.");
            config.native = false;
        }

        #[cfg(target_os="freebsd")]
        {
           if config.pid.is_some() {
               if std::env::var("PYSPY_ALLOW_FREEBSD_ATTACH").is_err() {
                    eprintln!("On FreeBSD, running py-spy can cause an exception in the profiled process if the process \
                        is calling 'socket.connect'.");
                    eprintln!("While this is fixed in recent versions of python, you need to acknowledge the risk here by \
                        setting an environment variable PYSPY_ALLOW_FREEBSD_ATTACH to run this command.");
                    eprintln!("\nSee https://github.com/benfred/py-spy/issues/147 for more information");
                    std::process::exit(-1);
               }
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn get_config(cmd: &str) -> clap::Result<Config> {
        #[cfg(target_os="freebsd")]
        std::env::set_var("PYSPY_ALLOW_FREEBSD_ATTACH", "1");
        let args: Vec<String> = cmd.split_whitespace().map(|x| x.to_owned()).collect();
        Config::from_args(&args)
    }

    #[test]
    fn test_parse_record_args() {
        // basic use case
        let config = get_config("py-spy record --pid 1234 --output foo").unwrap();
        assert_eq!(config.pid, Some(1234));
        assert_eq!(config.filename, Some(String::from("foo")));
        assert_eq!(config.format, Some(FileFormat::flamegraph));
        assert_eq!(config.command, String::from("record"));

        // same command using short versions of everything
        let short_config = get_config("py-spy r -p 1234 -o foo").unwrap();
        assert_eq!(config, short_config);

        // missing the --pid argument should fail
        assert_eq!(get_config("py-spy record -o foo").unwrap_err().kind,
                   clap::ErrorKind::MissingRequiredArgument);

        // but should work when passed a python program
        let program_config = get_config("py-spy r -o foo -- python test.py").unwrap();
        assert_eq!(program_config.python_program, Some(vec![String::from("python"), String::from("test.py")]));
        assert_eq!(program_config.pid, None);

        // passing an invalid file format should fail
        assert_eq!(get_config("py-spy r -p 1234 -o foo -f unknown").unwrap_err().kind,
                   clap::ErrorKind::InvalidValue);

        // test out overriding these params by setting flags
        assert_eq!(config.include_idle, false);
        assert_eq!(config.gil_only, false);
        assert_eq!(config.include_thread_ids, false);

        let config_flags = get_config("py-spy r -p 1234 -o foo --idle --gil --threads").unwrap();
        assert_eq!(config_flags.include_idle, true);
        assert_eq!(config_flags.gil_only, true);
        assert_eq!(config_flags.include_thread_ids, true);
    }

    #[test]
    fn test_parse_dump_args() {
        // basic use case
        let config = get_config("py-spy dump --pid 1234").unwrap();
        assert_eq!(config.pid, Some(1234));
        assert_eq!(config.command, String::from("dump"));

        // short version
        let short_config = get_config("py-spy d -p 1234").unwrap();
        assert_eq!(config, short_config);

        // missing the --pid argument should fail
        assert_eq!(get_config("py-spy dump").unwrap_err().kind,
                   clap::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_parse_top_args() {
        // basic use case
        let config = get_config("py-spy top --pid 1234").unwrap();
        assert_eq!(config.pid, Some(1234));
        assert_eq!(config.command, String::from("top"));

        // short version
        let short_config = get_config("py-spy t -p 1234").unwrap();
        assert_eq!(config, short_config);
    }

    #[test]
    fn test_parse_args() {
        assert_eq!(get_config("py-spy dude").unwrap_err().kind,
                   clap::ErrorKind::UnrecognizedSubcommand);
    }
}
