// Shared test code. Each test crate that says `mod support;` compiles all of it and
// uses only part, so unused items are not warnings here.
#![allow(dead_code)]

pub mod http;
