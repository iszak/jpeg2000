#![allow(dead_code)]

use clap::Parser;
use std::error;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, Seek};
use std::path::Path;
use std::str::FromStr;

use jp2::decode_jp2;
use jpc::decode_j2c;
use jpxml::{encode_jpxml, Representation};

#[derive(Debug)]
enum JP2000Error {
    UnsupportedExtension { extension: String },
}

impl error::Error for JP2000Error {}
impl fmt::Display for JP2000Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::UnsupportedExtension { extension } => {
                write!(f, "unsupported extension {}", extension)
            }
        }
    }
}

#[derive(Parser)]
struct Opts {
    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    Decode(JPXML),
    JPXML(JPXML),
}

#[derive(Parser)]
struct Decode {
    input: String,
}

#[derive(Parser)]
struct JPXML {
    input: String,

    #[clap(short, long, default_value = "skeleton")]
    representation: String,
}

fn run() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opts: Opts = Opts::parse();

    match opts.subcommand {
        SubCommand::Decode(c) => {
            let path = Path::new(&c.input);
            let extension = match path.extension().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };

            let file = File::open(path)?;

            match extension {
                "jp2" => {
                    let mut reader = BufReader::new(file);

                    let jp2 = decode_jp2(&mut reader)?;
                    for contiguous_codestreams_box in jp2.contiguous_codestreams_boxes() {
                        reader.seek(io::SeekFrom::Start(contiguous_codestreams_box.offset))?;
                        decode_j2c(&mut reader)?;
                    }
                }
                _ => {
                    return Err(JP2000Error::UnsupportedExtension {
                        extension: extension.to_owned(),
                    }
                    .into())
                }
            }
        }
        SubCommand::JPXML(c) => {
            let path = Path::new(&c.input);
            let filename = match path.file_name().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };
            let extension = match path.extension().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };

            let file = File::open(path)?;

            match extension {
                "jp2" => {
                    let mut writer = io::stdout();

                    encode_jpxml(
                        &mut writer,
                        &file,
                        Representation::from_str(&c.representation)?,
                        filename,
                    )?;
                }
                _ => {
                    return Err(JP2000Error::UnsupportedExtension {
                        extension: extension.to_owned(),
                    }
                    .into())
                }
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match run() {
        Err(e) => {
            return Err(e.to_string().into());
        }
        Ok(_) => Ok(()),
    }
}
