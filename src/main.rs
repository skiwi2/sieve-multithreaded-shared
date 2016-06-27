extern crate bit_vector;
extern crate crossbeam;
extern crate num;
extern crate stopwatch;

use std::collections::VecDeque;
use std::cmp;
use std::env;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::mpsc;

use bit_vector::{BitVector,BitSliceMut};

use num::Bounded;

use stopwatch::Stopwatch;

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
    let stopwatch = Stopwatch::start_new();
    let result = sieve.find_primes();
    println!("Time elapsed finding primes: {:.3}s", stopwatch.elapsed_ms() as f64 / 1000f64);
    //println!("Primes: {:?}", result.primes());
    println!("Found number of primes: {}", result.number_of_primes());
    println!("Total time elapsed: {:.3}s", stopwatch.elapsed_ms() as f64 / 1000f64);
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
        bit_vector.set(0, false);
        bit_vector.set(1, false);

        {
            let indices = self.calculate_indices();
            let mut prime_slices = self.split_into_prime_slices(&mut bit_vector, &indices);
            let first_prime_slice = prime_slices.pop_front().unwrap();
            //TODO refactor this horrible VecDeque popping, should instead use references, but how to get multiple mutable references to slice contents?

            let mut transmitters = vec![];
            let mut handles = vec![];

            crossbeam::scope(|scope| {
                for _ in 1..self.threads {
                    let (tx, rx) = mpsc::channel();
                    transmitters.push(tx);

                    let mut threaded_sieve_task = ThreadedSieveTask::new(prime_slices.pop_front().unwrap());

                    let handle = scope.spawn(move || {
                        for prime in rx.iter() {
                            threaded_sieve_task.strike_out_multiples(prime);
                        }
                    });
                    handles.push(handle);
                }

                let sqrt_prime = (self.max_prime as f64).sqrt().ceil() as usize;
                let main_sieve_task = MainSieveTask::new(first_prime_slice);
                for prime in main_sieve_task.generate_primes().take_while(|prime| *prime <= sqrt_prime) {
                    for tx in &transmitters {
                        tx.send(prime).unwrap();
                    }
                }

                drop(transmitters);
            });
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

    fn split_into_prime_slices<'a>(&self, bit_vector: &'a mut BitVector<SieveStorage>, indices: &[usize]) -> VecDeque<PrimeSlice<'a>> {
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

        let mut prime_slices = VecDeque::with_capacity(indices.len());
        let mut rolling_index = 0;
        for bit_slice in bit_slices {
            let bit_slice_capacity = bit_slice.capacity();
            prime_slices.push_back(PrimeSlice::new(bit_slice, rolling_index));
            rolling_index += bit_slice_capacity;
        }

        prime_slices
    }
}

#[derive(Debug)]
struct PrimeSlice<'a> {
    bit_slice: BitSliceMut<'a, SieveStorage>,
    start_number: usize
}

impl<'a> PrimeSlice<'a> {
    fn new(bit_slice: BitSliceMut<'a, SieveStorage>, start_number: usize) -> PrimeSlice<'a> {
        PrimeSlice {
            bit_slice: bit_slice,
            start_number: start_number
        }
    }

    fn set_is_prime(&mut self, number: usize, value: bool) {
        self.bit_slice.set(number - self.start_number, value);
    }

    fn is_prime(&self, number: usize) -> bool {
        self.bit_slice[number - self.start_number]
    }

    fn first_number(&self) -> usize {
        self.start_number
    }

    fn last_number(&self) -> usize {
        self.start_number + self.bit_slice.capacity() - 1
    }

    fn first_number_in_range_with_divisor(&self, divisor: usize) -> Option<usize> {
        if divisor > self.last_number() {
            return None;
        }
        if self.first_number() % divisor == 0 {
            return Some(self.first_number());
        }
        Some(self.first_number() + (divisor - (self.first_number() % divisor)))
    }
}

#[derive(Debug)]
struct MainSieveTask<'a> {
    prime_slice: PrimeSlice<'a>
}

impl<'a> MainSieveTask<'a> {
    fn new(prime_slice: PrimeSlice<'a>) -> MainSieveTask<'a> {
        MainSieveTask {
            prime_slice: prime_slice
        }
    }

    fn generate_primes(self) -> GenIter<'a> {
        GenIter {
            prime_slice: self.prime_slice,
            start_number: 0
        }
    }
}

struct GenIter<'a> {
    prime_slice: PrimeSlice<'a>,
    start_number: usize
}

impl<'a> Iterator for GenIter<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        let mut num = self.start_number;

        while !self.prime_slice.is_prime(num) {
            num += 1;
            if num > self.prime_slice.last_number() {
                return None;
            }
        }
        self.start_number = num + 1;    //do not mark num as prime, but do step over it for the next iteration

        let mut i = num * num;
        while i <= self.prime_slice.last_number() {
            self.prime_slice.set_is_prime(i, false);
            i += num;
        }

        Some(num)
    }
}

#[derive(Debug)]
struct ThreadedSieveTask<'a> {
    prime_slice: PrimeSlice<'a>
}

impl<'a> ThreadedSieveTask<'a> {
    fn new(prime_slice: PrimeSlice<'a>) -> ThreadedSieveTask<'a> {
        ThreadedSieveTask {
            prime_slice: prime_slice
        }
    }

    fn strike_out_multiples(&mut self, number: usize) {
        match self.prime_slice.first_number_in_range_with_divisor(number) {
            None => return,
            Some(mut val) => {
                while val <= self.prime_slice.last_number() {
                    self.prime_slice.set_is_prime(val, false);
                    val += number;
                } 
            }
        }
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