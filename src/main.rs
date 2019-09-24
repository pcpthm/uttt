use derive_more::{BitAnd, BitAndAssign, BitOr, Not};
use once_cell::sync::Lazy;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

#[derive(Debug, Clone, Copy, Default, BitAnd, BitOr, BitAndAssign, Not, PartialEq, Eq)]
pub struct Mask81(u128);

impl Mask81 {
    pub const ALL: Mask81 = Mask81((1u128 << 81) - 1);

    #[inline]
    fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }
}

const FIELD_ALL: u16 = (1u16 << 9) - 1;

#[derive(Debug, Clone)]
pub struct State {
    player_placed: Mask81,
    opponent_placed: Mask81,
    next_valid: Mask81,
    available_fields: Mask81,
    meta_player_placed: u16,
    meta_opponent_placed: u16,
}

impl State {
    pub fn initial() -> State {
        State {
            player_placed: Mask81::default(),
            opponent_placed: Mask81::default(),
            next_valid: Mask81::ALL,
            available_fields: Mask81::ALL,
            meta_player_placed: 0,
            meta_opponent_placed: 0,
        }
    }
}

struct Constants {
    pub info: [u8; 1 << 9],
    pub fields: [Mask81; 9],
}

static CONSTANTS: Lazy<Constants> = Lazy::new(|| {
    pub const WIN: [u16; 8] = [0o421, 0o124, 0o700, 0o070, 0o007, 0o111, 0o222, 0o444];
    let mut info = [0; 1 << 9];
    for mask in 0..(1 << 9) {
        info[mask as usize] = if WIN.iter().any(|&x| mask & x == x) {
            1
        } else {
            0
        };
    }
    let mut fields = [Mask81::default(); 9];
    for i in 0..9 {
        fields[i] = Mask81(((1u128 << 9) - 1) << (i * 9));
    }
    Constants { info, fields }
});

pub struct MoveCounter {
    constants: &'static Constants,
}

impl MoveCounter {
    pub fn new() -> Self {
        MoveCounter {
            constants: &CONSTANTS,
        }
    }

    #[inline]
    fn for_each_next_states(&self, state: State, mut callback: impl FnMut(State)) {
        let mut iter = state.next_valid.0;
        while iter != 0 {
            let pos = iter.trailing_zeros();

            debug_assert!((state.player_placed.0 >> pos) & 1 == 0);
            debug_assert!((state.opponent_placed.0 >> pos) & 1 == 0);

            let field = pos / 9;
            let field_mask = self.constants.fields[field as usize];
            let pos_in_field = pos % 9;
            let next_field_mask = self.constants.fields[pos_in_field as usize];

            let next_placed = state.player_placed | Mask81(1u128 << pos);

            let extracted = (state.player_placed.0 >> (field * 9) & ((1u128 << 9) - 1)) as u16;
            let new_field = extracted | 1u16 << pos_in_field;
            let new_win = self.constants.info[new_field as usize] != 0;
            let new_end = new_win || new_field == FIELD_ALL;

            let mut next_available_fields = state.available_fields;
            let mut meta_next_placed = state.meta_player_placed;
            let mut game_over = false;

            if new_end {
                next_available_fields &= !field_mask;

                if new_win {
                    meta_next_placed |= 1 << field;
                }

                game_over = self.constants.info[meta_next_placed as usize] != 0
                    || next_available_fields == Mask81::default();
            }

            let mut next_valid = if (next_available_fields & next_field_mask) == Mask81::default() {
                next_available_fields
            } else {
                next_field_mask
            };
            next_valid &= !next_placed;
            next_valid &= !state.opponent_placed;

            if !game_over {
                callback(State {
                    player_placed: state.opponent_placed,
                    opponent_placed: next_placed,
                    next_valid,
                    available_fields: next_available_fields,
                    meta_player_placed: state.meta_opponent_placed,
                    meta_opponent_placed: meta_next_placed,
                });
            }

            iter &= iter - 1;
        }
    }

    fn recurse(&self, state: State, depth: u32) -> u64 {
        debug_assert!(depth > 0);
        debug_assert!((state.player_placed & state.opponent_placed) == Mask81::default());
        debug_assert!(
            (state.next_valid
                & (state.player_placed | state.opponent_placed | !state.available_fields))
                == Mask81::default()
        );
        debug_assert!((0..9).all(|i| {
            let mask = (state.available_fields.0 >> (i * 9) & ((1u128 << 9) - 1)) as u16;
            assert!(mask == 0 || mask == FIELD_ALL);
            let meta = (state.meta_player_placed | state.meta_opponent_placed) >> i & 1;
            let placed = ((state.player_placed | state.opponent_placed).0 >> (i * 9)
                & ((1u128 << 9) - 1)) as u16;
            assert_eq!(mask == 0, meta != 0 || placed == FIELD_ALL);
            true
        }));

        let mut total = state.next_valid.count_ones().into();
        if depth >= 7 {
            let mut next_states = Vec::new();
            self.for_each_next_states(state, |next_state| {
                next_states.push(next_state);
            });
            total += next_states
                .into_par_iter()
                .map(|next_state| self.recurse(next_state, depth - 1))
                .sum::<u64>();
        } else if depth == 1 {
            self.for_each_next_states(state, |next_state| {
                total += next_state.next_valid.count_ones() as u64;
            });
        } else {
            self.for_each_next_states(state, |next_state| {
                total += self.recurse(next_state, depth - 1);
            });
        };

        total
    }

    pub fn count_moves(&self, depth: u32) -> u64 {
        if depth == 0 {
            0
        } else {
            // Take account of the symmetry for the first move
            let mut initial_states = Vec::with_capacity(9 * 3);
            let mut index = 0;
            self.for_each_next_states(State::initial(), |next_state| {
                if index < 9 * 2 {
                    initial_states.push((next_state, 4));
                } else if 9 * 4 <= index && index < 9 * 5 {
                    initial_states.push((next_state, 1));
                }
                index += 1;
            });
            initial_states
                .into_par_iter()
                .map(|(state, mul)| {
                    mul * (1 + if depth == 1 {
                        state.next_valid.count_ones() as u64
                    } else {
                        self.recurse(state, depth - 1)
                    })
                })
                .sum()
        }
    }
}

fn main() {
    use std::time::Instant;
    let counter = MoveCounter::new();
    for depth in 1..=9 {
        println!("depth = {}", depth);
        let instant = Instant::now();
        let result = counter.count_moves(depth);
        println!(
            "result = {}, time = {}ms",
            result,
            instant.elapsed().as_millis()
        );
    }
}
