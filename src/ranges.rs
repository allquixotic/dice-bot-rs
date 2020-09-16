#![allow(clippy::len_zero)]

use lazy_static::lazy_static;
use randomize::*;
use std::borrow::Cow;
extern crate regex;
use regex::*;

pub const D4: RandRangeU32 = RandRangeU32::new(1, 4);
pub const D6: RandRangeU32 = RandRangeU32::new(1, 6);
pub const D8: RandRangeU32 = RandRangeU32::new(1, 8);
pub const D10: RandRangeU32 = RandRangeU32::new(1, 10);
pub const D12: RandRangeU32 = RandRangeU32::new(1, 12);
pub const D20: RandRangeU32 = RandRangeU32::new(1, 20);

trait ExplodingRange {
  fn explode(&self, gen: &mut PCG32) -> u32;
}

impl ExplodingRange for RandRangeU32 {
  fn explode(&self, gen: &mut PCG32) -> u32 {
    let mut times = 0;
    loop {
      let roll = self.sample(gen);
      if roll == self.high() {
        times += 1;
        continue;
      } else {
        return self.high() * times + roll;
      }
    }
  }
}
