//@check-pass
//@revisions: cycle no_cycle
//@[cycle] compile-flags: --cfg 'feature="cycle"'
#![deny(unfulfilled_lint_expectations, clippy::module_cycles)]
#![allow(dead_code)]

#[cfg_attr(
    feature = "cycle",
    expect(clippy::module_cycles, reason = "The existing cycle is being removed")
)]
mod first {
    pub fn helper() {}

    #[cfg(feature = "cycle")]
    pub fn run() {
        super::second::run();
        super::second::run();
    }
}

#[allow(clippy::module_cycles, reason = "The other end of the existing cycle")]
mod second {
    pub fn run() {
        super::first::helper();
    }
}

fn main() {}
