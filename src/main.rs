extern crate num;

use std::cmp;
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
    let requested_threads: usize = parse_command_line_argument(1, "threads");
    let max_prime: usize = parse_command_line_argument(2, "max_prime");

    let threads = calculate_actual_threads(requested_threads, max_prime);

    let sieve = Sieve {
        threads: threads,
        max_prime: max_prime
    };
    println!("{:?}", sieve);
    println!("{:?}", sieve.calculate_indices(std::mem::size_of::<usize>() * 8));
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

fn calculate_actual_threads(requested_threads: usize, max_prime: usize) -> usize {
    let sqrt_prime = (max_prime as f64).sqrt().ceil() as usize;
    cmp::min(sqrt_prime, requested_threads)
}

#[derive(Debug)]
struct Sieve {
    threads: usize,
    max_prime: usize
}

impl Sieve {
    fn calculate_indices(&self, storage_size: usize) -> Vec<usize> {
        let numbers_per_segment = (self.max_prime + 1) / self.threads;

        let mut indices = vec![];        
        let mut current_index = self.max_prime + 1;
        loop {
            if indices.len() == self.threads - 1 {
                break;
            }

            current_index = match current_index.checked_sub(numbers_per_segment) {
                None => break,
                Some(val) => val
            };
            indices.insert(0, current_index);
        }

        for index in &mut indices {
            *index = ((*index / storage_size) + 1) * storage_size;
        }

        indices
    }
}