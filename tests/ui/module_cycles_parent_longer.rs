#![warn(clippy::module_cycles)]
#![allow(dead_code)]

mod longer_cycle {
    mod parent {
        pub struct Value;

        pub fn run() {
            child::run();
            //~^ module_cycles
        }

        mod child {
            pub fn run() {
                let _ = super::Value;
                super::super::other::run();
                //~^ module_cycles
            }
        }
    }

    mod other {
        pub fn run() {
            let _ = super::parent::Value;
            //~^ module_cycles
        }
    }
}

mod parent_outside_sibling_cycle {
    mod parent {
        pub struct Value;

        pub fn run() {
            child::run();
        }

        pub mod child {
            pub fn run() {
                let _ = super::Value;
                super::super::other::run();
                //~^ module_cycles
            }
        }
    }

    mod other {
        pub fn run() {
            super::parent::child::run();
            //~^ module_cycles
        }
    }
}

mod grandchild {
    pub struct Value;

    fn run() {
        child::grandchild::run();
        //~^ module_cycles
    }

    mod child {
        pub mod grandchild {
            pub fn run() {
                let _ = super::super::Value;
                //~^ module_cycles
            }
        }
    }
}

fn main() {}
