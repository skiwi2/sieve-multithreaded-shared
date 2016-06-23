extern crate num;

use std::env;
use std::fmt::Display;
use std::str::FromStr;

use num::Bounded;

macro_rules! err_exit {
    ($($arg:tt)*) => (
        {
            use std::io::prelude::*;
            use std::process;
            if let Err(e) = write!(&mut ::std::io::stderr(), "{}\n", format_args!($($arg)*)) {
                panic!("Failed to write to stderr.\
                    \nOriginal error output: {}\
                    \nSecondary error writing to stderr: {}", format!($($arg)*), e);
            }
            process::exit(1)
        }
    )
}

fn main() {
    let threads: usize = parse_command_line_argument(1, "threads");
    let max_prime: usize = parse_command_line_argument(2, "max_prime");

    let sieve = Sieve {
        threads: threads,
        max_prime: max_prime
    };
    println!("{:?}", sieve);
}

fn parse_command_line_argument<T: FromStr + Bounded + Display>(position: usize, name: &str) -> T {
    match env::args().nth(position) {
        None => err_exit!("Usage: sieve_multithreaded_shared <threads> <max_prime>"),
        Some(val) => match val.parse::<T>() {
            Err(_) => err_exit!("Expected <{}>: {} to be >= 0 and <= {}", name, val, T::max_value()),
            Ok(val) => val
        }
    }
}

#[derive(Debug)]
struct Sieve {
    threads: usize,
    max_prime: usize
}