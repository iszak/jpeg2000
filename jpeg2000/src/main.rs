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
use jpc::decode_jpc;
use jpxml::{encode_jp2, encode_jpc, Representation};

#[derive(Debug)]
enum JP2000Error {
    DecodingContainer { error: String },
    DecodingCodestream { error: String },
    UnsupportedExtension { extension: String },
}

impl error::Error for JP2000Error {}
impl fmt::Display for JP2000Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::DecodingContainer { error } => {
                write!(f, "error decoding jp2 container {}", error.to_string())
            }
            Self::DecodingCodestream { error } => {
                write!(f, "error decoding jpc codestream {}", error.to_string())
            }
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
    /// Decode .jp2 container or .jpc codestream file (noop)
    Decode(Decode),

    /// Encode .jp2 container or .jpc codestream file to JPXML document (stdout)
    JPXML(JPXML),
}

#[derive(Parser)]
struct Decode {
    /// Path to .jp2 file
    path: String,
}

#[derive(Parser)]
struct JPXML {
    /// Path to .jp2 file
    path: String,

    /// Level of representation of JPXML document generated from an image file
    /// format and/or codestreams.
    ///
    /// skeleton - does not contains text nodes.
    ///
    /// fat-skeleton - contains image properties excluding codestream chunk
    /// data.
    ///
    /// fat - contains whole image data on text nodes.
    #[clap(short, long, default_value = "skeleton")]
    representation: String,
}

fn run() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let opts: Opts = Opts::parse();

    match opts.subcommand {
        SubCommand::Decode(c) => {
            let path = Path::new(&c.path);
            let extension = match path.extension().and_then(OsStr::to_str) {
                Some(value) => value,
                None => "",
            };

            let file = File::open(path)?;

            match extension {
                "jp2" => {
                    let mut reader = BufReader::new(file);

                    let jp2 = match decode_jp2(&mut reader) {
                        Ok(jp2) => jp2,
                        Err(error) => {
                            return Err(JP2000Error::DecodingContainer {
                                error: error.to_string(),
                            }
                            .into())
                        }
                    };
                    for contiguous_codestreams_box in jp2.contiguous_codestreams_boxes() {
                        reader.seek(io::SeekFrom::Start(contiguous_codestreams_box.offset))?;
                        match decode_jpc(&mut reader) {
                            Err(error) => {
                                return Err(JP2000Error::DecodingCodestream {
                                    error: error.to_string(),
                                }
                                .into())
                            }
                            Ok(_) => {}
                        };
                    }
                }
                "jpc" | "j2c" => {
                    let mut reader = BufReader::new(file);
                    match decode_jpc(&mut reader) {
                        Err(error) => {
                            return Err(JP2000Error::DecodingCodestream {
                                error: error.to_string(),
                            }
                            .into())
                        }
                        Ok(_) => {}
                    };
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
            let path = Path::new(&c.path);
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

                    encode_jp2(
                        &mut writer,
                        &file,
                        Representation::from_str(&c.representation)?,
                        filename,
                    )?;
                }
                "jpc" => {
                    let mut writer = io::stdout();

                    encode_jpc(
                        &mut writer,
                        &file,
                        Representation::from_str(&c.representation)?,
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
