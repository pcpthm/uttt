use derive_more::{BitAnd, BitAndAssign, BitOr, Not};
use once_cell::sync::Lazy;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, BitAnd, BitOr, BitAndAssign, Not, PartialEq, Eq, Hash)]
pub struct Mask81(u128);

impl Mask81 {
    pub const ALL: Mask81 = Mask81((1u128 << 81) - 1);

    #[inline]
    fn count_ones(&self) -> u32 {
        self.0.count_ones()
    }
}

const FIELD_ALL: u16 = (1u16 << 9) - 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct State {
    player_placed: Mask81,
    opponent_placed: Mask81,
    next_valid: Mask81,
    available_fields: Mask81,
    meta_player_placed: u16,
    meta_opponent_placed: u16,
}

fn symmetry_perm(sym: u8, i: usize) -> usize {
    let mut y = i / 3;
    let mut x = i % 3;
    if sym & 1 != 0 {
        y = 2 - y;
    }
    if sym & 2 != 0 {
        x = 2 - x;
    }
    if sym & 4 != 0 {
        std::mem::swap(&mut y, &mut x);
    }
    y * 3 + x
}

fn symmetry_perm2(sym: u8, i: usize) -> usize {
    symmetry_perm(sym, i / 9) * 9 + symmetry_perm(sym, i % 9)
}

fn symmetry_perm_field(sym: u8, field: u16) -> u16 {
    (0..9)
        .map(|i| (field >> symmetry_perm(sym, i) & 1) << i)
        .sum()
}

fn symmetry_perm_mask(sym: u8, mask: Mask81) -> Mask81 {
    Mask81(
        (0..81)
            .map(|i| (mask.0 >> symmetry_perm2(sym, i) & 1) << i)
            .sum(),
    )
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

    // note: slow
    pub fn minimize_by_symmetry(&self) -> State {
        let mut board = [0u8; 81];
        board.iter_mut().enumerate().for_each(|(i, r)| {
            *r = ((self.player_placed.0 >> i & 1) as u8) << 2
                | ((self.opponent_placed.0 >> i & 1) as u8) << 1
                | ((self.next_valid.0 >> i & 1) as u8)
        });
        let mut min_board = [0xffu8; 81];
        let mut min_sym = 0;
        for sym in 0..8 {
            let mut permuted = [0u8; 81];
            for i in 0..81 {
                permuted[i] = board[symmetry_perm2(sym, i)];
            }
            if &permuted[..] < &min_board[..] {
                min_board = permuted;
                min_sym = sym;
            }
        }
        State {
            player_placed: symmetry_perm_mask(min_sym, self.player_placed),
            opponent_placed: symmetry_perm_mask(min_sym, self.opponent_placed),
            next_valid: symmetry_perm_mask(min_sym, self.next_valid),
            available_fields: symmetry_perm_mask(min_sym, self.available_fields),
            meta_player_placed: symmetry_perm_field(min_sym, self.meta_player_placed),
            meta_opponent_placed: symmetry_perm_field(min_sym, self.meta_opponent_placed),
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

    fn recurse(&self, state: State, depth: u32, max_depth: u32) -> u64 {
        debug_assert!(depth < max_depth);
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
        if max_depth - depth == 1 {
            self.for_each_next_states(state, |next_state| {
                total += next_state.next_valid.count_ones() as u64;
            });
        } else if max_depth - depth >= 7 {
            let mut next_states: HashMap<State, u64> = HashMap::new();
            self.for_each_next_states(state, |next_state| {
                *next_states
                    .entry(next_state.minimize_by_symmetry())
                    .or_default() += 1;
            });
            total += next_states
                .into_par_iter()
                .map(|(next_state, mul)| mul * self.recurse(next_state, depth + 1, max_depth))
                .sum::<u64>();
        } else {
            self.for_each_next_states(state, |next_state| {
                total += self.recurse(next_state, depth + 1, max_depth);
            });
        };

        total
    }

    pub fn count_moves(&self, max_depth: u32) -> u64 {
        if max_depth == 0 {
            0
        } else {
            self.recurse(State::initial(), 0, max_depth)
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
