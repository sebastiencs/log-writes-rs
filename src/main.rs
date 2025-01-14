#![feature(cstring_from_vec_with_nul)]

use std::{fs::OpenOptions, process::Command};
use crate::log_writes::{LogWriteEntry, Log};
use clap::{App, Arg};
use anyhow::Result;
use std::result::Result::Ok;

mod log_writes;
mod reader;
mod io;
mod util;

fn should_stop(entry : &LogWriteEntry, stop_flags : u64, mark : &str) -> i32 {
    let flags = entry.flags;
    let check_mark: i64 = (stop_flags & log_writes::LOG_MARK_FLAG) as i64;
    if (flags & stop_flags) > 0 {
        if check_mark <= 0 {
            return 1
        }
        if (flags & log_writes::LOG_MARK_FLAG) > 0 && entry.cmd == mark {
            return 1
        }
    }

    return 0
}

fn should_run_fsck(fsck_cmd: Option<&str>, flags: u64) -> bool {
    fsck_cmd.is_some() && flags & log_writes::LOG_FUA_FLAG != 0
}

fn run_fsck(log: &Log, fsck_cmd: Option<&str>) {
    let cmd = fsck_cmd.unwrap();

    log.replay_file.sync_all().expect("Failed to sync replay file");

    let ret = Command::new(cmd)
        .status()
        .expect("Failed to run fsck command");

    if !ret.success() {
        panic!("Fsck command failed {}", ret);
    }
}

#[cfg(target_os = "linux")]
fn main() -> Result<()>{
    let matches = App::new("Log Writer").version("1.0")
        .arg(Arg::with_name("log")
            .long("log")
            .value_name("LOG_PATH")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("replay")
            .long("replay")
            .value_name("REPLAY_PATH")
            .takes_value(true)
            .required(true)
        )
        .arg(Arg::with_name("limit")
            .long("limit")
            .value_name("LIMIT")
            .takes_value(true)
            .default_value("0")
        )
        .arg(Arg::with_name("fsck")
            .long("fsck")
            .value_name("FSCK")
            .takes_value(true)
        )
        .arg(Arg::with_name("check")
            .long("check")
            .value_name("CHECK")
            .takes_value(true)
        )
        .arg(Arg::with_name("verbose")
            .long("verbose")
            .short("v")
            .value_name("VERBOSE")
            .takes_value(false)
        )
        .arg( Arg::with_name("start-mark")
            .long("start-mark")
            .value_name("START_MARK")
            .takes_value(true)
        )
        .arg( Arg::with_name("end-mark")
            .long("end-mark")
            .value_name("END_MARK")
            .takes_value(true)
        ).get_matches();

    let log_file_path = matches.value_of("log").expect("Log file not provided");
    let replay_file_path = matches.value_of("replay").expect("Replay file not provided");
    let limit = matches.value_of("limit").expect("Log file not provided");
    let fsck_cmd = matches.value_of("fsck");
    let check = matches.value_of("check");
    let run_limit : u64 = limit.parse()?;
    let start_mark = matches.value_of("start-mark");
    let end_mark = matches.value_of("end-mark").unwrap();
    let mut stop_flags : u64 = 0;
    stop_flags |= log_writes::LOG_MARK_FLAG;
    let mut num_entries : u64 = 0;

    if fsck_cmd.is_some() && check != Some("fua") {
        panic!("parameter --check=fua is only supported");
    }

    let mut log = Log::open(log_file_path, replay_file_path)?;

    while let Some(entry) = log.replay_next_entry(true).unwrap() {
        num_entries += 1;

        if should_run_fsck(fsck_cmd, entry.flags) {
            run_fsck(&log, fsck_cmd);
        }

        if (run_limit > 0 && num_entries == run_limit)  || should_stop(&entry,stop_flags,end_mark.as_ref()) > 0 {
            break
        }
    }

    log.replay_file.sync_all().expect("Failed to sync replay file");

    Ok(())
}
