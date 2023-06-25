use crate::config::Config;

use std::fs::{File, OpenOptions};
use subprocess::Redirection;

fn file_read() -> OpenOptions {
    let mut opts = File::options();
    opts.read(true);
    opts
}

fn file_write() -> OpenOptions {
    let mut opts = File::options();
    opts.truncate(true);
    opts.write(true);
    opts.create(true);
    opts
}

fn stream_redirection(stream: &Option<String>, file: OpenOptions) -> Result<Redirection, String> {
    let path = stream.clone().unwrap_or("/dev/null".to_string());
    
    match file.open(path.clone()) {
        Ok(fd) => Ok(Redirection::File(fd)),
        Err(_) => Err(format!("Failed to open file for write. Path: {path}")),
    }
}

pub fn stdin_redirection(config: &Config) -> Result<Redirection, String> {
    Ok(stream_redirection(&config.streams.stdin, file_read())?)
}

pub fn stdout_redirection(config: &Config) -> Result<Redirection, String> {
    Ok(stream_redirection(&config.streams.stdout, file_write())?)
}

pub fn stderr_redirection(config: &Config) -> Result<Redirection, String> {
    if config.streams.stdout != config.streams.stderr {
        Ok(stream_redirection(&config.streams.stderr, file_write())?)
    } else {
        Ok(Redirection::Merge)
    }
}
