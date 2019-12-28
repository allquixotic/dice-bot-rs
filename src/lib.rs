#![allow(clippy::len_zero)]

use lazy_static::lazy_static;
use lokacore::*;
use randomize::*;
use std::sync::{Mutex, MutexGuard};
use std::borrow::Cow;
extern crate regex;
use regex::*;

pub mod earthdawn;
pub mod eote;
pub mod shadowrun;

pub const D4: RandRangeU32 = RandRangeU32::new(1, 4);
pub const D6: RandRangeU32 = RandRangeU32::new(1, 6);
pub const D8: RandRangeU32 = RandRangeU32::new(1, 8);
pub const D10: RandRangeU32 = RandRangeU32::new(1, 10);
pub const D12: RandRangeU32 = RandRangeU32::new(1, 12);
pub const D20: RandRangeU32 = RandRangeU32::new(1, 20);

lazy_static! {
  static ref GLOBAL_GEN: Mutex<PCG32> = Mutex::new(PCG32::default());
}

pub fn global_gen() -> MutexGuard<'static, PCG32> {
  GLOBAL_GEN
    .lock()
    .unwrap_or_else(|poison| poison.into_inner())
}
pub fn just_seed_the_global_gen() {
  let gen: &mut PCG32 = &mut global_gen();
  let mut arr: [u64; 2] = [0, 0];
  match getrandom::getrandom(bytes_of_mut(&mut arr)) {
    Ok(_) => *gen = PCG32::seed(arr[0], arr[1]),
    Err(_) => *gen = PCG32::default(),
  }
}

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

pub fn basic_sum_str(s: &str) -> Option<i32> {
  let mut ss : Cow<str> = Cow::from(s.clone());
  if ss.contains("/") {
      return None;
  }
  let mut total = 0;
  let mut current = 0;
  lazy_static! {
    static ref RE1: Regex = Regex::new(r"(\-{2,})").unwrap();
  }
  if RE1.is_match(&ss) {
  let cap = RE1.captures(s).unwrap();
  if cap[1].len() % 2 == 0 {
    ss = RE1.replace_all(s, "");
  }
  else {
    ss = RE1.replace_all(s, "-");
  }
  }
  let mut current_is_negative = ss.chars().nth(0).unwrap() == '-';
  for ch in ss.chars() {
    match ch {
      '0'..='9' => {
        current *= 10;
        current += ch.to_digit(10).unwrap() as i32;
      }
      '+' => {
        total += if current_is_negative {
          -current
        } else {
          current
        };
        current = 0;
        current_is_negative = false;
      }
      '-' => {
        total += if current_is_negative {
          -current
        } else {
          current
        };
        current = 0;
        current_is_negative = true;
      }
      _ => return None,
    };
  }
  total += if current_is_negative {
    -current
  } else {
    current
  };
  Some(total)
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn basic_sum_str_test_nums() {
    assert_eq!(basic_sum_str("1"), Some(1));
    assert_eq!(basic_sum_str("12"), Some(12));
    assert_eq!(basic_sum_str("-2"), Some(-2));
  }

  #[test]
  fn basic_sum_str_test_equations() {
    assert_eq!(basic_sum_str("-2+7"), Some(5));
    assert_eq!(basic_sum_str("8-2"), Some(6));
    assert_eq!(basic_sum_str("4+5"), Some(9));
  }

  #[test]
  fn basic_sum_str_test_too_many_operands() {
    assert_eq!(basic_sum_str("--23"), Some(23));
    assert_eq!(basic_sum_str("++54"), Some(54));
    assert_eq!(basic_sum_str("-------123"), Some(-123));
  }

  #[test]
  fn basic_sum_str_test_not_an_expression() {
    assert_eq!(basic_sum_str("abc"), None);
    assert_eq!(basic_sum_str("😝"), None);
  }

  #[test]
  fn basic_sum_str_no_mult_or_div() {
      assert_eq!(basic_sum_str("3/2"), None);
      assert_eq!(basic_sum_str("45*3.145"), None);
  }

}