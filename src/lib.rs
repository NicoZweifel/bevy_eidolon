pub mod components;
pub mod draw;
pub mod material;
pub mod node;
pub mod pipeline;
pub mod plugin;
pub mod prepare;
pub mod resources;
pub mod systems;

pub use plugin::*;

pub mod prelude {
    pub use super::plugin::*;
    pub use super::{components::*, material::*};
}

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
