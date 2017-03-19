#![allow(dead_code)]

#[macro_use]
extern crate lazy_static;
extern crate regex;

mod errors;
mod inputline;
mod scenario;


fn main() {
    let s = "hello";
    println!("{:?}", &s[1..s.len()-1]);
}
