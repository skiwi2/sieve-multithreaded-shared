extern crate bit_vector;
extern crate num;

use std::cmp;
use std::env;
use std::fmt::Display;
use std::str::FromStr;

use bit_vector::{BitVector,BitSliceMut};

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

type SieveStorage = usize;

fn main() {
    let requested_threads: usize = parse_command_line_argument(1, "threads");
    let max_prime: usize = parse_command_line_argument(2, "max_prime");

    let threads = calculate_actual_threads(requested_threads, max_prime);

    let sieve = Sieve {
        threads: threads,
        max_prime: max_prime
    };
    println!("{:?}", sieve);
    let result = sieve.find_primes();
    println!("Primes: {:?}", result.primes());
    println!("Found number of primes: {}", result.number_of_primes());
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
    fn find_primes(&self) -> SieveResult {
        let mut bit_vector = BitVector::with_capacity(self.max_prime + 1, true);

        {
            let indices = self.calculate_indices();
            let bit_slices = self.split_into_bit_slices(&mut bit_vector, &indices);

            println!("{:?}", bit_slices);
        }

        SieveResult {
            threads: self.threads,
            max_prime: self.max_prime,
            bit_vector: bit_vector
        }
    }

    fn calculate_indices(&self) -> Vec<usize> {
        let storage_size = std::mem::size_of::<SieveStorage>() * 8;
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

    fn split_into_bit_slices<'a>(&self, bit_vector: &'a mut BitVector<SieveStorage>, indices: &[usize]) -> Vec<BitSliceMut<'a, SieveStorage>> {
        let mut bit_slices = vec![];

        //TODO request as_bit_slice and as_bit_slice_mut methods?
        bit_slices.push(bit_vector.split_at_mut(0).1);
        let mut split_indices = 0;
        for index in indices {
            let last_slice = bit_slices.pop().unwrap();
            let (new_slice, remainder) = last_slice.split_at_mut(index - split_indices);
            split_indices = *index;
            bit_slices.push(new_slice);
            bit_slices.push(remainder);
        }

        bit_slices
    }
}

#[derive(Debug)]
struct SieveResult {
    threads: usize,
    max_prime: usize,
    bit_vector: BitVector<SieveStorage>
}

impl SieveResult {
    fn number_of_primes(&self) -> usize {
        self.bit_vector.iter().filter(|x| *x).count()
    }

    fn primes(&self) -> Vec<usize> {
        self.bit_vector.iter().enumerate().filter(|x| x.1).map(|x| x.0).collect()
    }
}