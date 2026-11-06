//@check-pass
#![warn(clippy::module_cycles)]

// ui_test builds a file that contains `#[test]` with `--test`, which adds a `main`
// function that refers to every test. It must not make `tests` depend on the crate root in both directions.

fn helper() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn uses_parent() {
        assert_eq!(super::helper(), 1);
    }
}
