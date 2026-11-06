//@check-pass
#![deny(clippy::module_cycles)]
#![allow(dead_code)]

fn run() {
    child::run();
}

struct Value;

mod child {
    pub fn run() -> super::Value {
        super::Value
    }
}

mod nested {
    fn run() {
        child::run();
    }

    struct Value;

    mod child {
        pub fn run() -> super::Value {
            super::Value
        }
    }
}

fn main() {}
