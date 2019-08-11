extern crate reqwest;
extern crate chrono;

use reqwest::{header, Client, Error, Response};
use chrono::prelude::*;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

use std::fs::File;
use std::str::FromStr;
use std::path::PathBuf;
use zip::ZipWriter;
use zip::write::FileOptions;
use std::process::exit;
use std::error::Error as StdError;
use clap::ArgMatches;

extern crate zip;
extern crate clap;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BASE_URI: &str = "https://flow.polar.com";

#[derive(Deserialize, Debug)]
struct CalendarEvent {
    #[serde(rename = "type")]
    record_type: String,
    #[serde(default)]
    timestamp: u64,
    url: String,
    #[serde(default, rename = "listItemId")]
    list_item_id: u64,
    #[serde(default)]
    datetime: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    calories: u32,
    #[serde(default)]
    distance: f32,
}

enum ExportFormat {
    TCX,
    GPX,
    CSV,
}

impl ExportFormat {
    fn as_str(&self) -> &'static str {
        match &self {
            ExportFormat::TCX => "tcx",
            ExportFormat::GPX => "gpx",
            ExportFormat::CSV => "csv",
        }
    }
}

impl FromStr for ExportFormat {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "tcx" => Ok(ExportFormat::TCX),
            "gpx" => Ok(ExportFormat::GPX),
            "csv" => Ok(ExportFormat::CSV),
            _ => Err("unknown format")
        }
    }
}

#[derive(Debug, Clone)]
struct ExporterError {
    cause: String
}

impl StdError for ExporterError {
    fn description(&self) -> &str { self.cause.as_str() }
}

impl std::fmt::Display for ExporterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.cause)
    }
}

trait Exporter {
    fn export(&mut self, client: &Client, event: &CalendarEvent, fmt: &ExportFormat);
}

struct FileExporter { directory: String }

struct ZipExporter { zip_writer: ZipWriter<File> }

fn load_session<F: FnMut(&mut Response)>(client: &Client, evt: &CalendarEvent, fmt: &ExportFormat, f: &mut F) {
    let mut res = client.get(format!("{}/api/export/training/{}/{}",
                                     BASE_URI, fmt.as_str(), evt.list_item_id).as_str())
        .send().unwrap();
    f(&mut res);
}

fn session_file_name(event: &CalendarEvent, fmt: &ExportFormat) -> String {
    let date = DateTime::parse_from_rfc3339(event.datetime.as_str()).unwrap();
    date.format(format!("%Y-%m-%d-%H_%M_%S.{}", fmt.as_str()).as_str()).to_string()
}

impl Exporter for FileExporter {
    fn export(&mut self, client: &Client, event: &CalendarEvent, fmt: &ExportFormat) {
        fn export_file(client: &Client, evt: &CalendarEvent, fmt: &ExportFormat, file_name: &str) {
            load_session(client, evt, fmt, &mut |res| {
                let mut file = File::create(file_name).unwrap();
                res.copy_to(&mut file).unwrap();
            });
        }
        let mut path = PathBuf::from(&self.directory);
        path.push(session_file_name(event, fmt));
        export_file(client, event,
                    fmt,
                    path.to_str().unwrap());
    }
}

impl Exporter for ZipExporter {
    fn export(&mut self, client: &Client, event: &CalendarEvent, fmt: &ExportFormat) {
        fn export_file(client: &Client, evt: &CalendarEvent, fmt: &ExportFormat, file_name: &str, w: &mut ZipWriter<File>) {
            load_session(client, evt, fmt, &mut |res| {
                w.start_file(file_name, FileOptions::default()).unwrap();
                res.copy_to(w).unwrap();
            });
        }
        export_file(client, event,
                    fmt,
                    &session_file_name(event, fmt),
                    &mut self.zip_writer,
        );
    }
}

/// Setup rest client, must provide `user-agent`
fn setup_client() -> Result<Client, Error> {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT,
                   header::HeaderValue::from_str(format!("rust / {}", VERSION).as_str()).unwrap());

    reqwest::ClientBuilder::new()
        .cookie_store(true)
        .default_headers(headers)
        .build()
}

/// Login to the polar flow API gateway
fn login(client: &Client, email: &str, password: &str) -> Result<Response, ExporterError> {
    let result = client.post((BASE_URI.to_owned() + "/" + "login").as_str())
        .form(&[("returnUrl", "/"), ("email", email), ("password", password)]).send();
    match result {
        Err(e) => Err(ExporterError { cause: e.description().to_owned() }),
        Ok(r) => if r.status() == 200 { Ok(r) } else { Err(ExporterError { cause: "invalid email or password".to_owned() }) }
    }
}

/// Gets training sessions lists
fn session_list(client: &Client, start: &DateTime<Local>, end: &DateTime<Local>) -> Vec<CalendarEvent> {
    const DATE_FMT: &str = "%-d.%-m.%Y";

    let uri = format!("{}/training/getCalendarEvents?start={}&end={}",
                      BASE_URI, start.format(DATE_FMT), end.format(DATE_FMT));
    let mut result = client.get(uri.as_str())
        .header(header::ACCEPT, "application/json")
        .send().unwrap();
    let calendar_events: Vec<CalendarEvent> = result.json().unwrap();
    calendar_events
}

fn main() {
    fn validate_date(v: String) -> Result<(), String> {
        match Local.datetime_from_str((v + " 00:00").as_str(), "%d.%m.%Y %H:%M") {
            Err(_) => Err("invalid date format".to_string()),
            _ => Ok(())
        }
    }

    let cmd_matches = clap::App::new("Polar flow session exporter")
        .version(VERSION)
        .about("Exports training sessions in various formats")
        .arg(clap::Arg::with_name("email").short("u")
            .takes_value(true)
            .required(true)
            .value_name("EMAIL")
            .help("Polar flow registration email"))
        .arg(clap::Arg::with_name("password").short("p")
            .takes_value(true)
            .required(true)
            .value_name("PASSWORD")
            .help("Polar flow registration password"))
        .arg(clap::Arg::with_name("format").short("f")
            .takes_value(true)
            .required(true)
            .value_name("EXPORT-FORMAT")
            .default_value("tcx")
            .possible_value("tcx")
            .possible_value("gpx")
            .possible_value("csv")
            .help("Training sessions export format"))
        .arg(clap::Arg::with_name("v")
            .short("v")
            .multiple(true)
            .help("Sets the level of verbosity"))
        .arg(clap::Arg::with_name("start-date").short("s")
            .takes_value(true)
            .required(true)
            .value_name("DATE")
            .default_value("01.01.1970")
            .validator(validate_date)
            .help("Start date for export, format DD.MM.YYYY"))
        .arg(clap::Arg::with_name("end-date").short("e")
            .takes_value(true)
            .required(true)
            .value_name("DATE")
            .default_value("31.12.2039")
            .validator(validate_date)
            .help("End date for export, format DD.MM.YYYY"))
        .subcommand(clap::SubCommand::with_name("zip")
            .about("exports all sessions into zip archive")
            .arg(clap::Arg::with_name("output-file").short("o")
                .takes_value(true)
                .help("output archive name")
                .value_name("ZIP-ARCHIVE")
                .required(true)))
        .subcommand(clap::SubCommand::with_name("files")
            .about("exports all sessions into directory")
            .arg(clap::Arg::with_name("output-directory").short("d")
                .takes_value(true)
                .help("output directory name")
                .value_name("OUTPUT-DIRECTORY")
                .required(false)))
        .get_matches();

    fn parse_input_date(opts: &ArgMatches, option: &str) -> DateTime<Local> {
        Local.datetime_from_str((opts.value_of(option).unwrap().to_owned() + " 00:00").as_str(),
                                "%d.%m.%Y %H:%M").unwrap()
    }

    let client = &setup_client().unwrap();
    login(client, cmd_matches.value_of("email").unwrap(),
          cmd_matches.value_of("password").unwrap())
        .unwrap_or_else(|e| {
            eprintln!("Unable to login to polar flow, {}", e);
            exit(1)
        });

    let mut exporter: Box<Exporter>;

    match cmd_matches.subcommand() {
        ("zip", Some(args)) => {
            exporter = Box::new(ZipExporter {
                zip_writer: ZipWriter::new(
                    File::create(args.value_of("output-file").unwrap()).unwrap())
            }
            );
        }
        ("files", Some(args)) => {
            exporter = Box::new(FileExporter {
                directory: args.value_of("output-directory")
                    .unwrap_or("").to_string()
            });
        }
        _ => {
            eprintln!("Invalid mode, {}", cmd_matches.usage());
            exit(1);
        }
    }

    let start_date = parse_input_date(&cmd_matches, "start-date");
    let end_date = parse_input_date(&cmd_matches, "end-date");

    if cmd_matches.occurrences_of("v") > 0 {
        println!("Export of the sessions from {} to {} started, exporter version: {}",
                 start_date.format("%-e-%b-%Y"), end_date.format("%-e-%b-%Y"), VERSION);
    }

    session_list(client, &start_date, &end_date)
        .into_iter().filter(|x| x.record_type.as_str() == "EXERCISE")
        .for_each(|x| {
            if cmd_matches.occurrences_of("v") > 0 {
                let hours = x.duration / 3600000;
                let minutes = x.duration % 3600000 / 60000;
                let seconds = x.duration % 3600000 % 60000 / 1000;
                println!("exporting session from {}, duration: {:02}:{:02}:{:02}, calories: {} kcal, distance: {:.2} km",
                         x.datetime,
                         hours, minutes, seconds,
                         x.calories,
                         (x.distance as f64) / 1000.0f64)
            }
            if cmd_matches.occurrences_of("v") > 1 {
                println!("{:#?}", x);
            }
            exporter.export(client, &x,
                            &ExportFormat::from_str(cmd_matches.value_of("format").unwrap()).unwrap());
        });
}
