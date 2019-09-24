# Ultimate Tick Tac Toe

See: <https://www.reddit.com/r/rust/comments/d85gyh/why_is_this_slow/> and <https://old.reddit.com/r/rust/comments/d8kew3/why_is_this_slow_update/>.

Not optimized at all.

## Key optimization

I made that last (depth = 0) moves can be counted just by the popcount instruction.
9x9 board is now an `u128` value.

## More optimization oppotunities

* More efficient representation than using consective 81 bits on an `u128`. For example, I used 32-bit-aligned bits for each 9x3 stack to sped up my sudoku solver <https://github.com/pcpthm/sudoku> (warning: overly optimized and not readable).
* Faster counting of recursion leaves (depth=1), somehow.
* Memorization
* Because depth is so small, we actually don't need to keep meta game information. i.e. Game won't end with such a small depth.
* Micro optimizations such as using `get_unchecked`.

## Branch: Parallel & symmetry exploit version

<https://github.com/pcpthm/uttt/tree/more>

* Symmetry is used for first few moves.
* Embarrassingly-parallel.

## Result on my machine (4 physical cores, 8 logical cores)

```text
depth = 1
result = 801, time = 0ms
depth = 2
result = 7137, time = 0ms
depth = 3
result = 62217, time = 0ms
depth = 4
result = 535473, time = 0ms
depth = 5
result = 4556433, time = 5ms
depth = 6
result = 38338977, time = 42ms
depth = 7
result = 319406385, time = 16ms
depth = 8
result = 2636425377, time = 100ms
depth = 9
result = 21620184705, time = 753ms
depth = 10
result = 176498814001, time = 5960ms
```
