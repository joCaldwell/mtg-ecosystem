//! One contiguous Rust allocation per NumPy tensor, reused across episodes.
use mtg_observe::{MAX_ENTITIES, Row};
macro_rules! tensors {
    ($($name:ident: $ty:ty [$($dim:tt),*]),*) => {
        #[derive(PartialEq, Eq)]
        pub struct Storage {
            pub len: usize,
            $(pub $name: Vec<$ty>),*
        }

        pub struct RowMut<'a> {
            $(pub $name: &'a mut [$ty]),*
        }

        impl Storage {
            pub fn new(len: usize) -> Self {
                Self {len, $($name: vec![0; len $(* $dim)*]),*}
            }

            pub fn partition(&mut self, workers: usize) -> Vec<Vec<RowMut<'_>>> {
                $(let mut $name = self.$name.chunks_exact_mut(1 $(* $dim)*);)*
                let mut groups: Vec<Vec<_>> = (0..workers).map(|_| Vec::new()).collect();
                for index in 0..self.len {
                    groups[index % workers].push(RowMut {
                        $($name: $name.next().expect("one slice per environment")) ,*
                    });
                }
                groups
            }
        }

        impl RowMut<'_> {
            pub fn write(&mut self, row: &Row) {
                $(self.$name.copy_from_slice(flat!(&row.$name $(,$dim)*));)*
            }
        }
    }
}
macro_rules! flat {
    ($x:expr) => {
        std::slice::from_ref($x)
    };
    ($x:expr,$a:tt) => {
        $x
    };
    ($x:expr,$a:tt,$b:tt) => {
        $x.as_flattened()
    };
}
tensors! {
    globals: i32 [48],
    players: i32 [4, 24],
    ent_int: i32 [MAX_ENTITIES, 10],
    ent_num: i32 [MAX_ENTITIES, 24],
    ent_mask: u8 [MAX_ENTITIES],
    cand: i32 [128, 6],
    ptr_mask: u8 [131],
    num_mask: u8 [64],
    head: u8 [],
    seat: u8 [],
    reward: i32 [4],
    done: u8 [],
    info: i32 [8]
}
