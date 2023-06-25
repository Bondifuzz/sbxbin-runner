mod config;
mod redirection;

use config::Config;

use signal_hook::flag::register;

use subprocess::ExitStatus;
use subprocess::Popen;
use subprocess::PopenConfig;

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[rustfmt::skip]
use redirection::{
    stdin_redirection,
    stdout_redirection,
    stderr_redirection,
};

#[derive(Debug)]
enum ExitReason {
    Finished,
    Timeout,
    Terminated,
    InternalError,
}

fn exit(reason: ExitReason) -> ! {
    match reason {
        ExitReason::Finished => std::process::exit(0),
        ExitReason::Timeout => std::process::exit(138), // SIGUSR1
        ExitReason::Terminated => std::process::exit(130), // SIGTERM
        ExitReason::InternalError => std::process::exit(-1),
    }
}

fn get_config_path() -> String {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: monitor <config.json>");
        exit(ExitReason::InternalError);
    }

    let config_path = args[1].clone();
    eprintln!("Using config file: '{config_path}'");
    config_path
}

fn get_config(path: &str) -> Config {
    config::load_json(path).unwrap_or_else(|e| {
        eprintln!("Failed to load config. Reason - {e}");
        exit(ExitReason::InternalError);
    })
}

fn create_popen_config(config: &Config) -> Result<PopenConfig, String> {

    let mut tmp_envs = HashMap::new();

    for (key, value) in env::vars_os() {
        tmp_envs.insert(key, value);
    }

    for env in config.env.iter() {
        tmp_envs.insert(
            Into::<OsString>::into(env.name.clone()),
            Into::<OsString>::into(env.value.clone()),
        );
    }

    let list_of_tuples_env = tmp_envs.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                v.clone(),
            )
        })
        .collect();

    Ok(PopenConfig {
        stdin: stdin_redirection(&config)?,
        stdout: stdout_redirection(&config)?,
        stderr: stderr_redirection(&config)?,
        env: Some(list_of_tuples_env),
        cwd: Some(config.cwd.clone().into()),
        ..Default::default()
    })
}

fn get_exit_code(exit_status: ExitStatus) -> Option<u32> {
    match exit_status {
        ExitStatus::Exited(code) => Some(code),
        ExitStatus::Signaled(code) => Some(code as u32 + 128),
        _ => None,
    }
}

#[rustfmt::skip]
fn graceful_shutdown(ps: &mut Popen, timeout: Duration) -> Result<u32, String> {
    //
    // Send SIGTERM and hope the process
    // will handle it and exit normally
    //

    if let Err(e) = ps.terminate() {
        return Err(format!(
            "Failed to send SIGTERM to {:?}. Reason - {}",
            ps.pid(), e.to_string()
        ));
    }

    //
    // Give some time to shutdown
    //

    let result = match ps.wait_timeout(timeout) {
        Ok(val) => val,
        Err(e) => {
            return Err(format!(
                "Unhandled error in process.wait(): {}",
                e.to_string()
            ));
        }
    };

    //
    // Process has ignored SIGTERM
    // Send SIGKILL to finish it off
    //

    if let None = result {
        if let Err(e) = ps.kill() {
            return Err(format!(
                "Failed to send SIGKILL to {:?}. Reason - {}",
                ps.pid(), e.to_string()
            ));
        }
    }

    //
    // Process must be finished
    // Just wait a bit and get exit code
    //

    let result = match ps.wait() {
        Ok(val) => val,
        Err(e) => {
            return Err(format!(
                "Unhandled error in process.wait(): {}",
                e.to_string()
            ));
        }
    };

    let exit_code = match get_exit_code(result) {
        Some(val) => val,
        None => {
            return Err(String::from("Failed to get child exit code"));
        }
    };

    Ok(exit_code)
}

fn main() {
    //
    // Run results
    //

    let mut child_exit_code: Option<u32> = None;
    let mut exit_reason = ExitReason::Finished;

    //
    // Get config
    //

    let config_path = get_config_path();
    let config = get_config(&config_path);

    //
    // Register signal handlers
    //

    let signals = [
        signal_hook::consts::SIGINT,  // rustfmt::skip
        signal_hook::consts::SIGTERM, // rustfmt::skip
    ];

    let term = Arc::new(AtomicBool::new(false));

    for signal in signals {
        register(signal, Arc::clone(&term)).unwrap_or_else(|e| {
            eprintln!("Failed to register signal handlers. Reason - {e}");
            exit(ExitReason::InternalError);
        });
    }

    //
    // Setup poll interval, duration...
    //

    let poll_interval = config.poll_interval_ms;
    let mut run_timeout = config.run_timeout_sec * 1000;
    let dur_timeout = Duration::from_millis(poll_interval);
    let dur_shutdown = Duration::from_secs(config.grace_period_sec);

    //
    // Start process with provided cmdline, cwd, env...
    //

    let pconf = create_popen_config(&config).unwrap_or_else(|e| {
        eprintln!("Failed to create popen config. Reason - {}", e);
        exit(ExitReason::InternalError);
    });

    eprintln!("Working directory: '{}'", config.cwd);
    eprintln!("Start process: '{}'", config.command.join(" "));

    let mut ps = Popen::create(&config.command, pconf).unwrap_or_else(|e| {
        eprintln!("Failed to start process. Reason - {}", e.to_string());
        exit(ExitReason::InternalError);
    });

    //
    // Wait for process finish, run timeout, os signals...
    //

    loop {
        let result = match ps.wait_timeout(dur_timeout) {
            Ok(val) => val,
            Err(e) => {
                eprintln!("Unhandled error in process.wait(): {}", e.to_string());
                exit_reason = ExitReason::InternalError;
                break;
            }
        };

        if let Some(exit_status) = result {
            child_exit_code = match get_exit_code(exit_status) {
                Some(val) => Some(val),
                None => {
                    eprintln!("Failed to get child exit code");
                    exit_reason = ExitReason::InternalError;
                    break;
                }
            };

            break;
        }

        //
        // Handle run timeout
        //

        run_timeout -= poll_interval;
        if run_timeout <= 0 {
            eprintln!("Run timeout. Exitting...");
            child_exit_code = match graceful_shutdown(&mut ps, dur_shutdown) {
                Ok(val) => Some(val),
                Err(e) => {
                    eprintln!("Graceful shutdown failed. Reason - {e}");
                    exit_reason = ExitReason::InternalError;
                    break;
                }
            };

            exit_reason = ExitReason::Timeout;
            break;
        }

        //
        // Handle OS signals
        //

        if term.load(Ordering::Relaxed) {
            eprintln!("Caught SIGTERM. Exitting...");
            child_exit_code = match graceful_shutdown(&mut ps, dur_shutdown) {
                Ok(val) => Some(val),
                Err(e) => {
                    eprintln!("Graceful shutdown failed. Reason - {e}");
                    exit_reason = ExitReason::InternalError;
                    break;
                }
            };

            exit_reason = ExitReason::Terminated;
            break;
        }
    }

    eprintln!("Exit. Reason: {exit_reason:?}");
    eprintln!("Child exit code: {child_exit_code:?}");

    if let Some(code) = child_exit_code {
        println!("{}", code);
    }

    exit(exit_reason);
}
