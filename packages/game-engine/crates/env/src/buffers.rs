//! One contiguous Rust allocation per NumPy tensor, reused across episodes.
use mtg_observe::Row;
macro_rules! tensors {
    ($($name:ident: $ty:ty [$($dim:literal),*]),*) => {
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
    ($x:expr,$a:literal) => {
        $x
    };
    ($x:expr,$a:literal,$b:literal) => {
        $x.as_flattened()
    };
}
tensors! {
    globals: i32 [48],
    players: i32 [4, 24],
    ent_int: i32 [256, 10],
    ent_num: i32 [256, 24],
    ent_mask: u8 [256],
    cand: i32 [128, 6],
    ptr_mask: u8 [131],
    num_mask: u8 [64],
    head: u8 [],
    seat: u8 [],
    reward: i32 [4],
    done: u8 [],
    info: i32 [8]
}
