#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests;
